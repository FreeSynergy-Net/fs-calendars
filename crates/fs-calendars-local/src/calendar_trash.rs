//! The trash of calendars: delete with everything in it, bring back, purge for good.
//!
//! Pattern: Transaction script (as `calendar_ops`). The appointments of a calendar that goes to the trash go with it
//! (same deadline) and are recorded in `calendar_cascade`; only that record brings them back with the calendar (an
//! appointment deleted earlier keeps its own deadline). A retention beats `purge`.

use fs_calendars_testdata::draft::{
    CalendarDeleted, CalendarInfo, Error, HistoryKind, ObjectId, PersonRef, Result, Revision,
};

use crate::calendar_ops::{in_trash, kept_by, one};
use crate::calendar_tables::CASCADE;
use crate::codec::{get_int, id_value, invalid};
use crate::envelope::{log, state, trash, untrash};
use crate::restorable::{is_kept, restorable};
use crate::store::Conn;
use crate::sweep::{Guarded, guard, unguard};

/// The living appointments (masters) of a calendar with their revisions.
async fn living(c: &Conn<'_>, cal: ObjectId) -> Result<Vec<(ObjectId, Revision)>> {
    let sql = "SELECT t.id, o.rev FROM calendar_appointment t JOIN objects o ON o.id = t.id WHERE t.collection = ? AND t.master IS NULL AND o.deleted IS NULL ORDER BY t.id";
    c.query(sql, vec![id_value(cal)])
        .await?
        .iter()
        .map(|r| {
            Ok((
                crate::codec::get_id(r, 0)?,
                Revision(u64::try_from(get_int(r, 1)?).unwrap_or(0)),
            ))
        })
        .collect()
}

/// Delete a calendar: it and its living appointments go to the trash at one instant, with one deadline.
///
/// # Errors
/// `NotFound` (also when it is in the trash already), `Changed`.
pub async fn delete_calendar(
    c: Conn<'_>,
    actor: &PersonRef,
    now: i64,
    id: ObjectId,
    expected: Revision,
) -> Result<CalendarDeleted> {
    let r = delete_in(&c, actor, now, id, expected).await;
    c.finish(r).await
}

async fn delete_in(
    c: &Conn<'_>,
    actor: &PersonRef,
    now: i64,
    id: ObjectId,
    expected: Revision,
) -> Result<CalendarDeleted> {
    let (_, deleted) = one(c, id, now).await?.ok_or(Error::NotFound(id))?;
    if deleted.is_some() {
        return Err(Error::NotFound(id));
    }
    let inside = living(c, id).await?;
    let (_, until) = trash(c, id, expected, now).await?;
    for (appointment, rev) in &inside {
        trash(c, *appointment, *rev, now).await?;
        c.insert(&CASCADE, vec![id_value(*appointment), id_value(id)])
            .await?;
        log(c, *appointment, HistoryKind::Delete, actor, now, &[]).await?;
    }
    log(c, id, HistoryKind::Delete, actor, now, &[]).await?;
    let ids = kept_by(c, id).await?;
    Ok(CalendarDeleted {
        id,
        restorable: restorable(c, &ids, until, now).await?,
        appointments: u32::try_from(inside.len()).unwrap_or(u32::MAX),
    })
}

/// Bring a calendar back with the appointments that went with it (those in `calendar_cascade`).
///
/// # Errors
/// `NotFound` (unknown, or the period is over, or purged), `Invalid` (not in the trash).
pub async fn restore_calendar(
    c: Conn<'_>,
    actor: &PersonRef,
    now: i64,
    id: ObjectId,
) -> Result<CalendarInfo> {
    let r = restore_in(&c, actor, now, id).await;
    c.finish(r).await
}

async fn restore_in(
    c: &Conn<'_>,
    actor: &PersonRef,
    now: i64,
    id: ObjectId,
) -> Result<CalendarInfo> {
    let (_, deleted) = one(c, id, now).await?.ok_or(Error::NotFound(id))?;
    if deleted.is_none() {
        return Err(invalid("id", "is not in the trash"));
    }
    let sql = "SELECT appointment FROM calendar_cascade WHERE collection = ? ORDER BY appointment";
    let with: Vec<ObjectId> = c
        .query(sql, vec![id_value(id)])
        .await?
        .iter()
        .map(|r| crate::codec::get_id(r, 0))
        .collect::<Result<_>>()?;
    // Shown as `Held`, it can be restored: the layer's `untrash` looks at the hold of each object, but only the kept
    // one has it, so the others get a retention for the moment (the same transaction, taken off again at once).
    let mut guarded = Guarded::new();
    if is_kept(c, &kept_by(c, id).await?, now).await? {
        let mut all = with.clone();
        all.push(id);
        guarded = guard(c, &all, Some(now.saturating_add(1)), now).await?;
    }
    untrash(c, id, now).await?;
    for a in with {
        untrash(c, a, now).await?;
        c.exec(
            "DELETE FROM calendar_cascade WHERE appointment = ?",
            vec![id_value(a)],
        )
        .await?;
        log(c, a, HistoryKind::Undo, actor, now, &[]).await?;
    }
    unguard(c, guarded).await?;
    log(c, id, HistoryKind::Undo, actor, now, &[]).await?;
    one(c, id, now)
        .await?
        .map(|x| x.0)
        .ok_or(Error::NotFound(id))
}

/// Delete something in the trash for good (an appointment, or a calendar with its appointments), with its log. A
/// retention beats it (`Forbidden`); not in the trash is `Invalid`.
///
/// # Errors
/// `NotFound`, `Invalid`, `Forbidden { trash.purge }`.
pub async fn purge(c: Conn<'_>, now: i64, id: ObjectId) -> Result<()> {
    let r = purge_in(&c, now, id).await;
    c.finish(r).await
}

async fn purge_in(c: &Conn<'_>, now: i64, id: ObjectId) -> Result<()> {
    let st = state(c, id).await?.ok_or(Error::NotFound(id))?;
    if st.deleted.is_none() {
        return Err(invalid("id", "is not in the trash"));
    }
    let mut ids = vec![id];
    if is_calendar(c, id).await? {
        ids = kept_by(c, id).await?;
    } else if let Some(cal) = cascaded_from(c, id).await? {
        // An appointment that went with its calendar is kept by the calendar's retention too.
        ids.push(cal);
    }
    if is_kept(c, &ids, now).await? {
        return Err(Error::Forbidden {
            capability: "trash.purge".to_owned(),
        });
    }
    crate::envelope::delete_final(c, id).await
}

async fn is_calendar(c: &Conn<'_>, id: ObjectId) -> Result<bool> {
    Ok(!c
        .query(
            "SELECT 1 FROM calendar_collection WHERE id = ?",
            vec![id_value(id)],
        )
        .await?
        .is_empty())
}

/// The calendar an appointment was trashed with, if it was.
///
/// # Errors
/// The engine's error.
pub async fn cascaded_from(c: &Conn<'_>, id: ObjectId) -> Result<Option<ObjectId>> {
    let rows = c
        .query(
            "SELECT collection FROM calendar_cascade WHERE appointment = ?",
            vec![id_value(id)],
        )
        .await?;
    rows.first().map(|r| crate::codec::get_id(r, 0)).transpose()
}

/// The error for a write on an appointment that is in the trash: `NotFound(<calendar>)` when its calendar is in the
/// trash (like a task in a deleted list), else `NotFound(<appointment>)`.
///
/// # Errors
/// The engine's error.
pub async fn gone(c: &Conn<'_>, appointment: ObjectId, calendar: ObjectId) -> Result<Error> {
    Ok(Error::NotFound(if in_trash(c, calendar).await? {
        calendar
    } else {
        appointment
    }))
}
