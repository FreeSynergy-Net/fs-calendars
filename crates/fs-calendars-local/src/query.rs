//! Typed queries (R6): the list by hull and keyset, the window by hull and exact placement in the viewer's zone.
//!
//! Pattern: Query object (the SQL is built from the typed query; values are bound, never pasted). The window reads
//! the counts it needs in the same query (no extra query per occurrence); a series is resolved by the pilot's engine.

use std::collections::HashMap;

use fs_calendars_testdata::draft::{
    Appointment, AppointmentQuery, AppointmentSummary, Attendee, BasicValue, Damaged, Error,
    Instance, ObjectId, Page, ParticipationStatus, PersonRef, Place, Result, Utc, Window, Zone,
    ZoneKind,
};
use fs_calendars_testdata::instances::in_window;
use fs_calendars_testdata::zone::bounds;
use turso::Value;

use crate::calendar_tables::APPOINTMENT;
use crate::codec::{get_id, get_int, get_opt_text, id_text, id_value, int, invalid, text};
use crate::read::{fill, fill_series, head, select_with};
use crate::store::Conn;
use crate::words::{PARTICIPATION, from_word};

const RULES: &str =
    "(SELECT count(*) FROM calendar_rule r WHERE r.appointment = t.id AND r.excluded = 0)";

fn calendars_in(calendars: &[ObjectId], sql: &mut String, params: &mut Vec<Value>) {
    if !calendars.is_empty() {
        let marks = vec!["?"; calendars.len()].join(",");
        sql.push_str(" AND t.collection IN (");
        sql.push_str(&marks);
        sql.push(')');
        params.extend(calendars.iter().map(|c| id_value(*c)));
    }
}

fn after_of(p: &Place) -> Result<(i64, String)> {
    let bad = || invalid("after", "not a place of this provider");
    let (m, id) = p.0.split_once(':').ok_or_else(bad)?;
    Ok((m.parse().map_err(|_| bad())?, id.to_owned()))
}

fn col(name: &str) -> usize {
    APPOINTMENT
        .cols
        .iter()
        .position(|x| x.name == name)
        .unwrap_or(0)
}

fn who_key(who: &Attendee) -> (&'static str, String) {
    match who {
        Attendee::Actor(p) => ("actor", p.id.canonical()),
        Attendee::Resource(r) => ("resource", r.canonical()),
        Attendee::Location(r) => ("location", r.canonical()),
    }
}

/// The appointments of a query, by hull start then id, `size` per page.
///
/// # Errors
/// `Invalid` for a place of another provider; `Unavailable` for a row that does not read back.
pub async fn list(
    c: &Conn<'_>,
    q: &AppointmentQuery,
    after: Option<&Place>,
    size: usize,
) -> Result<Page<AppointmentSummary>> {
    let mut sql = format!(
        "{} WHERE t.master IS NULL",
        select_with(&format!(", {RULES}"))
    );
    let mut params: Vec<Value> = Vec::new();
    if !q.include_deleted {
        sql.push_str(" AND o.deleted IS NULL");
    }
    calendars_in(&q.calendars, &mut sql, &mut params);
    if let Some(t) = &q.text {
        // The same Unicode lower-casing as the writer (`search_text`), never SQL `lower()`.
        sql.push_str(" AND instr(t.search_text, ?) > 0");
        params.push(text(&t.to_lowercase()));
    }
    if let Some(from) = q.from {
        sql.push_str(" AND (t.series_end_utc IS NULL OR t.series_end_utc >= ?)");
        params.push(int(from.millis()));
    }
    if let Some(to) = q.to {
        sql.push_str(" AND t.when_utc_min < ?");
        params.push(int(to.millis()));
    }
    if let Some(who) = &q.participant {
        let (kind, id) = who_key(who);
        sql.push_str(" AND EXISTS (SELECT 1 FROM calendar_participant p WHERE p.appointment = t.id AND p.who_kind = ? AND p.who_id = ?)");
        params.extend([text(kind), text(&id)]);
    }
    if let Some(p) = after {
        let (m, id) = after_of(p)?;
        sql.push_str(" AND (t.when_utc_min > ? OR (t.when_utc_min = ? AND t.id > ?))");
        params.extend([int(m), int(m), text(&id)]);
    }
    sql.push_str(" ORDER BY t.when_utc_min, t.id LIMIT ?");
    let size = size.max(1);
    params.push(int(i64::try_from(size + 1).unwrap_or(i64::MAX)));
    let rows = c.query(&sql, params).await?;
    let n = APPOINTMENT.cols.len();
    let mut items = Vec::new();
    let mut last = None;
    for row in rows.iter().take(size) {
        let l = head(row)?;
        last = Some(Place(format!(
            "{}:{}",
            get_int(row, col("when_utc_min"))?,
            id_text(l.appt.id)
        )));
        let a = &l.appt;
        items.push(AppointmentSummary {
            id: a.id,
            calendar: a.calendar,
            uid: a.uid.clone(),
            title: a.title.clone(),
            when: a.when.clone(),
            recurring: get_int(row, n + 3)? > 0,
            revision: a.revision,
            restorable_until: l.purge_after.map(Utc::from_millis),
        });
    }
    Ok(Page {
        items,
        next: if rows.len() > size { last } else { None },
    })
}

/// A single appointment placed in the viewer's zone; `None` when it does not overlap `[from, to)`.
fn plain(
    a: &Appointment,
    view: &Zone,
    count: i64,
    mine: Option<ParticipationStatus>,
    from: Utc,
    to: Utc,
) -> Result<Option<Instance>> {
    let (s, e) = bounds(&a.when, &HashMap::new(), view)?;
    // A length-0 appointment at the start of the window belongs to it (K5).
    if !(s < to && (e > from || (s == e && s >= from))) {
        return Ok(None);
    }
    Ok(Some(Instance {
        appointment: a.id,
        calendar: a.calendar,
        recurrence_id: None,
        title: a.title.clone(),
        when: a.when.clone(),
        start: s,
        end: e,
        status: a.status,
        free_busy: a.free_busy,
        privacy: a.privacy,
        overridden: false,
        recurring: false,
        participant_count: u32::try_from(count).unwrap_or(0),
        my_status: mine,
    }))
}

/// The occurrences that overlap `[from, to)`, placed in the viewer's zone; a series is resolved occurrence by
/// occurrence. D2: all-day and floating rows are found through the widened UTC hull, then placed exactly.
///
/// # Errors
/// `UnknownZone` for a custom `view_zone`; any other fault costs only the occurrence it touches (`Window::damaged`).
pub async fn window(
    c: &Conn<'_>,
    from: Utc,
    to: Utc,
    view: &Zone,
    calendars: &[ObjectId],
    me: &PersonRef,
) -> Result<Window> {
    if view.kind() == ZoneKind::Custom {
        return Err(Error::UnknownZone(view.id().to_owned()));
    }
    let extra = format!(
        ", (SELECT count(*) FROM calendar_participant p WHERE p.appointment = t.id), (SELECT p.status FROM calendar_participant p WHERE p.appointment = t.id AND p.who_kind = 'actor' AND p.who_id = ? LIMIT 1), {RULES}"
    );
    let mut params = vec![text(&me.id.canonical())];
    let mut sql = format!(
        "{} WHERE t.master IS NULL AND o.deleted IS NULL AND ((t.when_utc_min < ? AND (t.series_end_utc IS NULL OR t.series_end_utc >= ?)) \
         OR EXISTS (SELECT 1 FROM calendar_appointment e WHERE e.master = t.id AND e.hidden_by IS NULL AND e.excluded = 0 AND e.when_utc_max >= ? AND e.when_utc_min < ?))",
        select_with(&extra)
    );
    params.extend([
        int(to.millis()),
        int(from.millis()),
        int(from.millis()),
        int(to.millis()),
    ]);
    calendars_in(calendars, &mut sql, &mut params);
    let rows = c
        .query(&format!("{sql} ORDER BY t.when_utc_min, t.id"), params)
        .await?;
    let n = APPOINTMENT.cols.len();
    let mut out = Window::default();
    for row in &rows {
        let id = get_id(row, 0)?;
        let one = async {
            let mut a = head(row)?.appt;
            let rules = get_int(row, n + 5)?;
            if rules > 0 {
                fill(c, &mut a).await?;
                fill_series(c, &mut a).await?;
                let (found, damaged) = in_window(&a, &HashMap::new(), Some(me), view, from, to);
                out.instances.extend(found);
                out.damaged.extend(damaged);
                return Ok(());
            }
            let mine = get_opt_text(row, n + 4)?
                .map(|w| from_word(&PARTICIPATION, &w))
                .transpose()?;
            if let Some(i) = plain(&a, view, get_int(row, n + 3)?, mine, from, to)? {
                out.instances.push(i);
            }
            Ok::<(), Error>(())
        }
        .await;
        if let Err(e) = one {
            out.damaged.push(Damaged {
                appointment: id,
                recurrence_id: None,
                reason: format!("{e:?}"),
                cut: false,
            });
        }
    }
    out.instances.sort_by_key(|i| {
        (
            i.start,
            i.appointment,
            i.recurrence_id.map(|r| r.canonical()),
        )
    });
    Ok(out)
}
