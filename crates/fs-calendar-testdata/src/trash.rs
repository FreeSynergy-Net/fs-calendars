//! Delete and restore: the trash for a whole appointment, an `Excluded` override or a cut for a series scope.
//!
//! Pattern: Command executor (shares the commit path of `scope.rs`).

use crate::draft::{
    Appointment, Deleted, Error, HistoryKind, ObjectId, Override, OverrideChange, Result,
    SeriesScope,
};
use crate::instances::series_start;
use crate::scope::{checked, commit, invalid, occurrence, truncate, upsert};
use crate::state::{Item, State, TRASH_MS};

/// Delete with a scope: the whole thing goes to the trash; one occurrence becomes an `Excluded` override;
/// this and the following cut the series.
///
/// # Errors
/// `NotFound`, `Changed`, `Invalid`.
pub fn delete(
    state: &mut State,
    id: ObjectId,
    expected: crate::draft::Revision,
    scope: SeriesScope,
) -> Result<Deleted> {
    let cur = checked(state, id, Some(expected))?;
    let whole = |state: &mut State, cur: &Appointment| {
        let until = Some(crate::draft::Utc(state.now.0 + TRASH_MS));
        state.items.insert(
            cur.id,
            Item {
                appt: cur.clone(),
                deleted: Some(state.now),
            },
        );
        state.record(cur, HistoryKind::Delete, Vec::new());
        Deleted {
            id: cur.id,
            scope,
            restorable_until: until,
        }
    };
    match scope {
        SeriesScope::Whole => Ok(whole(state, &cur)),
        SeriesScope::This { recurrence_id } => {
            occurrence(&cur, recurrence_id)?;
            let mut new = cur.clone();
            upsert(
                &mut new,
                Override {
                    recurrence_id,
                    change: OverrideChange::Excluded,
                },
            );
            commit(state, cur.revision, new, HistoryKind::Delete, Vec::new());
            Ok(Deleted {
                id,
                scope,
                restorable_until: None,
            })
        }
        SeriesScope::ThisAndFollowing { recurrence_id } => {
            let at = occurrence(&cur, recurrence_id)?;
            if series_start(&cur.when) == Some(at) {
                return Ok(whole(state, &cur));
            }
            let mut new = cur.clone();
            truncate(&mut new, at);
            commit(state, cur.revision, new, HistoryKind::Delete, Vec::new());
            Ok(Deleted {
                id,
                scope,
                restorable_until: None,
            })
        }
    }
}

/// Bring a trashed appointment back while the trash still holds it.
///
/// # Errors
/// `NotFound` for an unknown id; `Invalid` when it is not in the trash or the period is over.
pub fn restore(state: &mut State, id: ObjectId) -> Result<Appointment> {
    let item = state.items.get(&id).ok_or(Error::NotFound(id))?.clone();
    let Some(at) = item.deleted else {
        return Err(invalid("id", "is not in the trash"));
    };
    if state.now.0 > at.0 + TRASH_MS {
        return Err(invalid("id", "the restore period is over"));
    }
    let new = commit(
        state,
        item.appt.revision,
        item.appt,
        HistoryKind::Undo,
        Vec::new(),
    );
    Ok(new)
}
