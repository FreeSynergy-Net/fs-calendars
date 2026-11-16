//! The writes and reads of `LocalAppointments`, one function per trait method (the trait impl only delegates).
//!
//! Pattern: Transaction script; each write is one transaction over the envelope, the domain rows and the log (R3).

use fs_calendar_testdata::draft::{
    Appointment, AppointmentDraft, AppointmentPatch, CalendarInfo, Deleted, Error, HistoryKind,
    ObjectId, PersonRef, Result, Revision, SeriesScope, Utc, Zone,
};
use fs_calendar_testdata::patch::apply;
use fs_calendar_testdata::state::blank;

use crate::calendar_tables::COLLECTION;
use crate::clock::new_id;
use crate::codec::{id_text, id_value, invalid, person_of, text};
use crate::envelope::{bump, insert, log, trash, untrash};
use crate::fields::{diff, flatten};
use crate::read::{Rd, load};
use crate::scope_delete::{delete_partial, restore_partial};
use crate::scope_ops::{update_following, update_this};
use crate::store::Conn;
use crate::write::{validate, write_appointment, write_collection};
use fs_calendar_testdata::draft::{FieldChange, Value as Val};
use fs_calendar_testdata::instances::series_start;
use fs_calendar_testdata::zone::naive;

async fn calendar_exists(c: &Conn<'_>, id: ObjectId) -> Result<bool> {
    let r = c.query("SELECT 1 FROM calendar_collection k JOIN objects o ON o.id = k.id WHERE k.id = ? AND o.deleted IS NULL", vec![id_value(id)]).await?;
    Ok(!r.is_empty())
}

async fn must_load(c: &Conn<'_>, id: ObjectId) -> Result<crate::read::Loaded> {
    load(c, "t.id = ? AND t.master IS NULL", vec![id_value(id)])
        .await?
        .ok_or(Error::NotFound(id))
}

/// All calendars that are not in the trash.
///
/// # Errors
/// `Unavailable` for a row that does not read back.
pub async fn calendars(c: &Conn<'_>) -> Result<Vec<CalendarInfo>> {
    let cols: Vec<String> = COLLECTION
        .cols
        .iter()
        .map(|x| format!("k.{}", x.name))
        .collect();
    let rows = c.query(&format!("SELECT {}, o.rev FROM calendar_collection k JOIN objects o ON o.id = k.id WHERE o.deleted IS NULL ORDER BY o.name_sort, k.id", cols.join(", ")), Vec::new()).await?;
    rows.iter()
        .map(|row| {
            let r = Rd {
                t: &COLLECTION,
                row,
            };
            Ok(CalendarInfo {
                id: r.id("id")?,
                name: r.text("name")?,
                color: r.opt_text("color")?,
                default_zone: r
                    .opt_text("default_zone")?
                    .map(|z| crate::codec::zone_of(&z))
                    .transpose()?,
                owner: person_of(row, 4)?,
                revision: Revision(
                    u64::try_from(crate::codec::get_int(row, COLLECTION.cols.len())?).unwrap_or(0),
                ),
            })
        })
        .collect()
}

/// Create a calendar (envelope, collection row, log) in one transaction.
///
/// # Errors
/// `Invalid` (empty name), `UnknownZone`, or the engine's error.
pub async fn create_calendar(
    c: Conn<'_>,
    owner: &PersonRef,
    now: i64,
    name: &str,
    color: Option<&str>,
    zone: Option<Zone>,
) -> Result<CalendarInfo> {
    let r = create_calendar_in(&c, owner, now, name, color, zone).await;
    c.finish(r).await
}

async fn create_calendar_in(
    c: &Conn<'_>,
    owner: &PersonRef,
    now: i64,
    name: &str,
    color: Option<&str>,
    zone: Option<Zone>,
) -> Result<CalendarInfo> {
    if name.trim().is_empty() {
        return Err(invalid("name", "must not be empty"));
    }
    if zone.as_ref().is_some_and(crate::write::custom) {
        return Err(Error::Unsupported {
            capability: "calendar.create".to_owned(),
            reason: "custom zones are not stored yet (the calendar_custom_zone tables are missing)"
                .to_owned(),
        });
    }
    let id = new_id(now)?;
    insert(c, id, "Calendar", name, owner, now).await?;
    write_collection(c, id, name, color, zone.as_ref(), owner).await?;
    let ch = [FieldChange {
        field: "name".to_owned(),
        row_key: None,
        old: None,
        new: Some(Val::Text(name.to_owned())),
    }];
    log(c, id, HistoryKind::Create, owner, now, &ch).await?;
    calendars(c)
        .await?
        .into_iter()
        .find(|x| x.id == id)
        .ok_or(Error::NotFound(id))
}

/// Create an appointment: checks first, then envelope, rows and log in one transaction.
///
/// # Errors
/// `NotFound` (calendar), `Invalid` (including a `uid` that exists), `Unsupported` (parts not stored yet).
pub async fn create(
    c: Conn<'_>,
    actor: &PersonRef,
    now: i64,
    d: AppointmentDraft,
) -> Result<Appointment> {
    let r = create_in(&c, actor, now, d).await;
    c.finish(r).await
}

pub async fn create_in(
    c: &Conn<'_>,
    actor: &PersonRef,
    now: i64,
    d: AppointmentDraft,
) -> Result<Appointment> {
    let id = new_id(now)?;
    let uid = d
        .uid
        .clone()
        .unwrap_or_else(|| format!("{}@local", id_text(id)));
    let mut a = blank(
        id,
        d.calendar,
        uid,
        &d.title,
        d.when.clone(),
        Utc::from_millis(now),
    );
    apply(&mut a, &d.rest);
    validate(&a, "event.create")?;
    if !calendar_exists(c, d.calendar).await? {
        return Err(Error::NotFound(d.calendar));
    }
    if !c
        .query(
            "SELECT 1 FROM calendar_appointment WHERE uid = ? AND master IS NULL",
            vec![text(&a.uid)],
        )
        .await?
        .is_empty()
    {
        return Err(invalid("uid", "exists already"));
    }
    insert(c, id, "Event", &a.title, actor, now).await?;
    write_appointment(c, &a).await?;
    log(
        c,
        id,
        HistoryKind::Create,
        actor,
        now,
        &diff(&crate::fields::Flat::new(), &flatten(&a)),
    )
    .await?;
    Ok(must_load(c, id).await?.appt)
}

/// Change an appointment (whole): compare-and-set, no write when nothing changes.
///
/// # Errors
/// `NotFound`, `Changed`, `Invalid`, `Unsupported`.
pub async fn update(
    c: Conn<'_>,
    actor: &PersonRef,
    now: i64,
    id: ObjectId,
    expected: Revision,
    p: AppointmentPatch,
    scope: SeriesScope,
) -> Result<Appointment> {
    let r = async {
        crate::origin::refuse_rule_edit(&c, id, &p).await?;
        match scope {
            SeriesScope::This { recurrence_id } => {
                return update_this(&c, actor, now, id, expected, &p, recurrence_id).await;
            }
            SeriesScope::ThisAndFollowing { recurrence_id } => {
                let l = must_load(&c, id).await?;
                let first = series_start(&l.appt.when).zip(naive(recurrence_id));
                if first.is_none_or(|(s, r)| s != r) {
                    return update_following(&c, actor, now, id, expected, &p, recurrence_id).await;
                }
            }
            SeriesScope::Whole => {}
        }
        let old = must_load(&c, id).await?;
        if old.deleted.is_some() {
            return Err(Error::NotFound(id));
        }
        if old.appt.revision != expected {
            return Err(Error::Changed {
                current: old.appt.revision,
            });
        }
        let mut new = old.appt.clone();
        apply(&mut new, &p);
        if diff(&flatten(&old.appt), &flatten(&new)).is_empty() {
            return Ok(old.appt);
        }
        new.sequence += 1;
        new.updated = Utc::from_millis(now);
        validate(&new, "event.update")?;
        bump(&c, id, expected, Some(&new.title), now).await?;
        write_appointment(&c, &new).await?;
        log(
            &c,
            id,
            HistoryKind::Update,
            actor,
            now,
            &diff(&flatten(&old.appt), &flatten(&new)),
        )
        .await?;
        Ok(must_load(&c, id).await?.appt)
    }
    .await;
    c.finish(r).await
}

/// Delete (whole) = trash; `restorable_until` is `purge_after`.
///
/// # Errors
/// `NotFound`, `Changed`, `Unsupported` (series scopes).
pub async fn delete(
    c: Conn<'_>,
    actor: &PersonRef,
    now: i64,
    id: ObjectId,
    expected: Revision,
    scope: SeriesScope,
) -> Result<Deleted> {
    let r = async {
        if !matches!(scope, SeriesScope::Whole) {
            return delete_partial(&c, actor, now, id, expected, scope).await;
        }
        let old = must_load(&c, id).await?;
        if old.deleted.is_some() {
            return Err(Error::NotFound(id));
        }
        let (_, until) = trash(&c, id, expected, now).await?;
        log(&c, id, HistoryKind::Delete, actor, now, &[]).await?;
        Ok(Deleted {
            id,
            scope,
            restorable_until: Some(Utc::from_millis(until)),
        })
    }
    .await;
    c.finish(r).await
}

/// Restore (whole) while `purge_after` has not passed.
///
/// # Errors
/// `NotFound` (none, or the period is over), `Invalid` (not in the trash), `Unsupported`.
pub async fn restore(
    c: Conn<'_>,
    actor: &PersonRef,
    now: i64,
    id: ObjectId,
    scope: SeriesScope,
) -> Result<Appointment> {
    let r = async {
        let l = must_load(&c, id).await?;
        if l.deleted.is_none() {
            return restore_partial(&c, actor, now, l.appt, scope).await;
        }
        untrash(&c, id, now).await?;
        log(&c, id, HistoryKind::Undo, actor, now, &[]).await?;
        Ok(must_load(&c, id).await?.appt)
    }
    .await;
    c.finish(r).await
}
