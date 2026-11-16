//! Exceptions of a series as rows (master + `recurrence_id`, `calendar_override_field`) and the cuts that make a partial
//! delete undoable (`calendar_cut`, `calendar_cut_rule`).
//!
//! Pattern: Transaction script. An exception is an own appointment row with its own envelope; the fields it sets are
//! listed, the rest is inherited. Rows are changed one by one, never rewritten as a whole (R4: no history loss).

use fs_calendar_testdata::draft::{
    Appointment, AppointmentPatch, LocalDateTime, ObjectId, Override, OverrideChange, PersonRef,
    Result,
};
use fs_calendar_testdata::instances::when_at;
use fs_calendar_testdata::patch::apply;
use fs_calendar_testdata::zone::naive;

use crate::calendar_tables::{APPOINTMENT, OVERRIDE};
use crate::clock::new_id;
use crate::codec::{get_id, get_text, id_value, int, int_to_local, invalid, local_to_int, text};
use crate::envelope::insert;
use crate::read::{Rd, fill, head, select};
use crate::store::Conn;
use crate::write::write_row;

/// The fields of a patch, as stored words.
fn fields_of(p: &AppointmentPatch) -> Vec<&'static str> {
    [
        ("title", p.title.is_some()),
        ("description", p.description.is_some()),
        ("locale", p.locale.is_some()),
        ("when", p.when.is_some()),
        ("status", p.status.is_some()),
        ("free_busy", p.free_busy.is_some()),
        ("privacy", p.privacy.is_some()),
        ("priority", p.priority.is_some()),
        ("color", p.color.is_some()),
        ("keywords", p.keywords.is_some()),
        ("categories", p.categories.is_some()),
        ("participants", p.participants.is_some()),
        ("alerts", p.alerts.is_some()),
        ("use_default_alerts", p.use_default_alerts.is_some()),
    ]
    .into_iter()
    .filter_map(|(n, set)| set.then_some(n))
    .collect()
}

/// The id of the exception row of `(master, rid)`, hidden or not.
pub async fn exception_id(
    c: &Conn<'_>,
    master: ObjectId,
    rid: LocalDateTime,
) -> Result<Option<ObjectId>> {
    let rows = c
        .query(
            "SELECT id FROM calendar_appointment WHERE master = ? AND recurrence_id = ?",
            vec![id_value(master), int(local_to_int(rid)?)],
        )
        .await?;
    rows.first().map(|r| get_id(r, 0)).transpose()
}

/// Create or replace the exception of one occurrence (an `Excluded` one or a changed one).
///
/// # Errors
/// `Invalid`, `UnknownZone` or the engine's error.
pub async fn put_exception(
    c: &Conn<'_>,
    master: &Appointment,
    o: &Override,
    actor: &PersonRef,
    now: i64,
) -> Result<()> {
    let at = naive(o.recurrence_id)
        .ok_or_else(|| invalid("scope.recurrence_id", "no such date or time"))?;
    let existing = exception_id(c, master.id, o.recurrence_id).await?;
    let eid = existing.unwrap_or_else(|| new_id(now));
    let mut m = master.clone();
    m.id = eid;
    m.when = when_at(&master.when, at);
    let fields = match &o.change {
        OverrideChange::Changed(p) => {
            apply(&mut m, p);
            fields_of(p)
        }
        OverrideChange::Excluded => Vec::new(),
    };
    (m.recurrence, m.excluded_recurrence, m.overrides, m.related) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    if existing.is_none() {
        insert(c, eid, "EventException", &m.title, actor, now).await?;
    }
    write_row(
        c,
        &m,
        Some((
            master.id,
            o.recurrence_id,
            o.change == OverrideChange::Excluded,
        )),
    )
    .await?;
    c.exec(
        "DELETE FROM calendar_override_field WHERE appointment = ?",
        vec![id_value(eid)],
    )
    .await?;
    for f in fields {
        c.insert(&OVERRIDE, vec![id_value(eid), text(f)]).await?;
    }
    Ok(())
}

fn patch_of(e: &Appointment, fields: &[String]) -> AppointmentPatch {
    let has = |f: &str| fields.iter().any(|x| x == f);
    AppointmentPatch {
        title: has("title").then(|| e.title.clone()),
        description: has("description").then(|| (e.description.clone(), e.description_type)),
        locale: has("locale").then(|| e.locale.clone()),
        when: has("when").then(|| e.when.clone()),
        status: has("status").then_some(e.status),
        free_busy: has("free_busy").then_some(e.free_busy),
        privacy: has("privacy").then_some(e.privacy),
        priority: has("priority").then_some(e.priority),
        color: has("color").then(|| e.color.clone()),
        keywords: has("keywords").then(|| e.keywords.clone()),
        categories: has("categories").then(|| e.categories.clone()),
        participants: has("participants").then(|| e.participants.clone()),
        alerts: has("alerts").then(|| e.alerts.clone()),
        use_default_alerts: has("use_default_alerts").then_some(e.use_default_alerts),
        ..AppointmentPatch::default()
    }
}

/// The overrides of a master (hidden exceptions are not part of it), by occurrence.
///
/// # Errors
/// `Unavailable` for a row that does not read back.
pub async fn read_overrides(c: &Conn<'_>, master: ObjectId) -> Result<Vec<Override>> {
    let rows = c
        .query(
            &format!(
                "{} WHERE t.master = ? AND t.hidden_by IS NULL ORDER BY t.recurrence_id",
                select()
            ),
            vec![id_value(master)],
        )
        .await?;
    let mut out = Vec::new();
    for row in &rows {
        let r = Rd {
            t: &APPOINTMENT,
            row,
        };
        let rid = int_to_local(
            r.opt_int("recurrence_id")?
                .ok_or_else(|| crate::codec::damaged("recurrence_id"))?,
        )?;
        if r.flag("excluded")? {
            out.push(Override {
                recurrence_id: rid,
                change: OverrideChange::Excluded,
            });
            continue;
        }
        let mut e = head(row)?.appt;
        fill(c, &mut e).await?;
        let f = c
            .query(
                "SELECT field FROM calendar_override_field WHERE appointment = ?",
                vec![id_value(e.id)],
            )
            .await?;
        let names: Vec<String> = f.iter().map(|x| get_text(x, 0)).collect::<Result<_>>()?;
        out.push(Override {
            recurrence_id: rid,
            change: OverrideChange::Changed(patch_of(&e, &names)),
        });
    }
    Ok(out)
}

/// Remove the exception of one occurrence (envelope and rows).
///
/// # Errors
/// The engine's error.
pub async fn remove_exception(c: &Conn<'_>, master: ObjectId, rid: LocalDateTime) -> Result<()> {
    if let Some(e) = exception_id(c, master, rid).await? {
        c.exec("DELETE FROM objects WHERE id = ?", vec![id_value(e)])
            .await?;
    }
    Ok(())
}

/// Hide the exceptions at or after `at` (a cut took their occurrences away).
///
/// # Errors
/// The engine's error.
pub async fn hide(c: &Conn<'_>, master: ObjectId, at: i64) -> Result<()> {
    c.exec("UPDATE calendar_appointment SET hidden_by = ? WHERE master = ? AND recurrence_id >= ? AND hidden_by IS NULL", vec![int(at), id_value(master), int(at)]).await.map(|_| ())
}

/// Show the exceptions that the cut at `at` hid.
///
/// # Errors
/// The engine's error.
pub async fn unhide(c: &Conn<'_>, master: ObjectId, at: i64) -> Result<()> {
    c.exec(
        "UPDATE calendar_appointment SET hidden_by = NULL WHERE master = ? AND hidden_by = ?",
        vec![id_value(master), int(at)],
    )
    .await
    .map(|_| ())
}

/// Give the visible exceptions at or after `at` to the tail series (the split of "this and the following").
///
/// # Errors
/// The engine's error.
pub async fn move_to_tail(
    c: &Conn<'_>,
    head_id: ObjectId,
    tail: &Appointment,
    at: i64,
) -> Result<()> {
    c.exec("UPDATE calendar_appointment SET master = ?, uid = ? WHERE master = ? AND recurrence_id >= ? AND hidden_by IS NULL", vec![id_value(tail.id), text(&tail.uid), id_value(head_id), int(at)]).await.map(|_| ())
}
