//! Partial deletes of a series and their undo: one occurrence (`This`) or this and the following.
//!
//! Pattern: Transaction script (as `scope_ops`). A cut (`calendar_cut`) lets `restore(id, scope)` undo it until the
//! period is over; `ThisAndFollowing` without `expected` reports `Changed` when the series moved on (PLAN section 10).

use fs_calendar_testdata::draft::{
    Appointment, Deleted, Error, FieldChange, HistoryKind, LocalDateTime, ObjectId, Override,
    OverrideChange, PersonRef, Result, Revision, RuleEnd, SeriesScope, Utc, Value as Val,
};
use fs_calendar_testdata::instances::series_start;
use fs_calendar_testdata::scope::{occurrence, truncate};
use fs_calendar_testdata::zone::naive;

use crate::codec::{invalid, naive_secs};
use crate::cuts::{save_cut, take_cut};
use crate::envelope::{bump, log, trash, trash_days};
use crate::read::load;
use crate::scope_ops::live;
use crate::series::{hide, put_exception, remove_exception, unhide};
use crate::store::Conn;
use crate::write::write_appointment;

const DAY_MS: i64 = 86_400_000;

fn marked(rid: LocalDateTime, what: &str) -> FieldChange {
    FieldChange {
        field: "override".to_owned(),
        row_key: Some(fs_calendar_testdata::draft::BasicValue::canonical(&rid)),
        old: None,
        new: Some(Val::Text(what.to_owned())),
    }
}

/// Delete one occurrence (an `Excluded` exception) or cut the series (this and the following); both are undoable.
///
/// # Errors
/// `NotFound`, `Changed`, `Invalid` (not an occurrence).
pub async fn delete_partial(
    c: &Conn<'_>,
    actor: &PersonRef,
    now: i64,
    id: ObjectId,
    expected: Revision,
    scope: SeriesScope,
) -> Result<Deleted> {
    let cur = live(c, id, expected).await?.appt;
    let (rid, following) = match scope {
        SeriesScope::This { recurrence_id } => (recurrence_id, false),
        SeriesScope::ThisAndFollowing { recurrence_id } => (recurrence_id, true),
        SeriesScope::Whole => return Err(invalid("scope", "not a partial scope")),
    };
    let at = occurrence(&cur, rid)?;
    if following && series_start(&cur.when) == Some(at) {
        let (_, until) = trash(c, id, expected, now).await?;
        log(c, id, HistoryKind::Delete, actor, now, &[]).await?;
        return Ok(Deleted {
            id,
            scope,
            restorable_until: Some(Utc::from_millis(until)),
        });
    }
    let until = now + trash_days(c).await? * DAY_MS;
    if following {
        let ends: Vec<RuleEnd> = cur.recurrence.iter().map(|r| r.end).collect();
        let mut head = cur.clone();
        truncate(&mut head, at);
        write_appointment(c, &head).await?;
        hide(c, id, naive_secs(at)).await?;
        let rev = bump(c, id, expected, None, now).await?;
        save_cut(
            c,
            id,
            rid,
            true,
            until,
            i64::try_from(rev.0).unwrap_or(0),
            &ends,
        )
        .await?;
    } else {
        put_exception(
            c,
            &cur,
            &Override {
                recurrence_id: rid,
                change: OverrideChange::Excluded,
            },
            actor,
            now,
        )
        .await?;
        let rev = bump(c, id, expected, None, now).await?;
        save_cut(
            c,
            id,
            rid,
            false,
            until,
            i64::try_from(rev.0).unwrap_or(0),
            &[],
        )
        .await?;
    }
    log(
        c,
        id,
        HistoryKind::Delete,
        actor,
        now,
        &[marked(rid, if following { "cut" } else { "excluded" })],
    )
    .await?;
    Ok(Deleted {
        id,
        scope,
        restorable_until: Some(Utc::from_millis(until)),
    })
}

/// Undo a partial delete. `ThisAndFollowing` without `expected` reports `Changed` when the series moved on since
/// the cut (PLAN section 10): the old rules would otherwise overwrite newer ones.
///
/// # Errors
/// `NotFound` (period over), `Invalid` (nothing was deleted there), `Changed`.
pub async fn restore_partial(
    c: &Conn<'_>,
    actor: &PersonRef,
    now: i64,
    cur: Appointment,
    scope: SeriesScope,
) -> Result<Appointment> {
    let (rid, following) = match scope {
        SeriesScope::This { recurrence_id } => (recurrence_id, false),
        SeriesScope::ThisAndFollowing { recurrence_id } => (recurrence_id, true),
        SeriesScope::Whole => return Err(invalid("id", "is not in the trash")),
    };
    let id = cur.id;
    let Some(cut) = take_cut(c, id, rid, following).await? else {
        return Err(invalid("scope", "nothing was deleted there"));
    };
    if cut.until <= now {
        return Err(Error::NotFound(id));
    }
    if following {
        if i64::try_from(cur.revision.0).unwrap_or(-1) != cut.rev_after {
            return Err(Error::Changed {
                current: cur.revision,
            });
        }
        let mut back = cur.clone();
        for (r, end) in back.recurrence.iter_mut().zip(&cut.ends) {
            r.end = *end;
        }
        write_appointment(c, &back).await?;
        let at =
            naive(rid).ok_or_else(|| invalid("scope.recurrence_id", "no such date or time"))?;
        unhide(c, id, naive_secs(at)).await?;
    } else if cur
        .overrides
        .iter()
        .any(|o| o.recurrence_id == rid && o.change == OverrideChange::Excluded)
    {
        remove_exception(c, id, rid).await?;
    }
    bump(c, id, cur.revision, None, now).await?;
    log(
        c,
        id,
        HistoryKind::Undo,
        actor,
        now,
        &[marked(rid, "restored")],
    )
    .await?;
    Ok(load(c, "t.id = ?", vec![crate::codec::id_value(id)])
        .await?
        .ok_or(Error::NotFound(id))?
        .appt)
}
