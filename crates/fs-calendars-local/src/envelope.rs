//! The envelope row of an object and the log (R7, R4, R10): the shared layer's functions with the trait's types.
//!
//! Pattern: Adapter (each function converts the id, revision, person or change at the border, calls the layer inside
//! the caller's transaction and maps the error). The rules (compare-and-set, trash, final delete, typed log) are the
//! layer's; when the trait moves to `fs-types` this file collapses to a `pub use`.

use fs_calendars_testdata::draft::{
    FieldChange, HistoryKind, ObjectId, PersonRef, Result, Revision,
};
use fs_objects::v5::envelope as layer;
use fs_objects::v5::log as layer_log;

pub use layer::{DEFAULT_TRASH_DAYS, State};

use crate::convert::{adapt, change, kind};
use crate::registry::DOMAIN;
use crate::store::Conn;

/// Insert the envelope of a new object, `rev = 1`.
///
/// # Errors
/// `Invalid` (a value does not fit its column) or the engine's error.
pub async fn insert(
    c: &Conn<'_>,
    id: ObjectId,
    kind: &str,
    name: &str,
    owner: &PersonRef,
    now: i64,
) -> Result<()> {
    layer::insert(c.inner(), id, kind, name, owner, now)
        .await
        .map_err(adapt)
}

/// The envelope state of an object, `None` when there is no such row.
///
/// # Errors
/// The engine's error, or a damaged row.
pub async fn state(c: &Conn<'_>, id: ObjectId) -> Result<Option<State>> {
    layer::state(c.inner(), id).await.map_err(adapt)
}

/// Compare-and-set: `rev + 1` only if the row is at `expected` and not in the trash.
///
/// # Errors
/// `NotFound` (no row, or in the trash), `Changed { current }` (someone wrote before).
pub async fn bump(
    c: &Conn<'_>,
    id: ObjectId,
    expected: Revision,
    name: Option<&str>,
    now: i64,
) -> Result<Revision> {
    layer::bump(c.inner(), id, expected, name, now)
        .await
        .map_err(adapt)
}

/// A period run changed the data of an object: `rev + 1` without an expected revision, the envelope follows the data.
///
/// # Errors
/// `NotFound` when there is no such row.
pub async fn touch(c: &Conn<'_>, id: ObjectId, name: Option<&str>, now: i64) -> Result<Revision> {
    layer::touch(c.inner(), id, name, now).await.map_err(adapt)
}

/// The trash period in days: `setting.trash_days`, else 30.
///
/// # Errors
/// The engine's error.
pub async fn trash_days(c: &Conn<'_>) -> Result<i64> {
    layer::trash_days(c.inner()).await.map_err(adapt)
}

/// Move an object to the trash: `deleted = now`, `purge_after = now + trash_days`.
///
/// # Errors
/// As [`bump`].
pub async fn trash(
    c: &Conn<'_>,
    id: ObjectId,
    expected: Revision,
    now: i64,
) -> Result<(Revision, i64)> {
    layer::trash(c.inner(), id, expected, now)
        .await
        .map_err(adapt)
}

/// Take an object out of the trash while `purge_after` has not passed.
///
/// # Errors
/// `NotFound` when there is no row or the period is over; `Invalid` when it is not in the trash.
pub async fn untrash(c: &Conn<'_>, id: ObjectId, now: i64) -> Result<Revision> {
    layer::untrash(c.inner(), id, now).await.map_err(adapt)
}

/// Write one log entry with its typed changes (nothing for an object in `unlogged_ids`).
///
/// # Errors
/// `Invalid` (a value does not fit) or the engine's error.
pub async fn log(
    c: &Conn<'_>,
    id: ObjectId,
    kind_of: HistoryKind,
    actor: &PersonRef,
    now: i64,
    changes: &[FieldChange],
) -> Result<()> {
    let changes = changes.iter().map(change).collect::<Vec<_>>();
    layer_log::log(c.inner(), id, kind(kind_of), actor, now, &changes)
        .await
        .map_err(adapt)
}

/// Log purge (ADR 0029): remove the log of an object, keep the object.
///
/// # Errors
/// `NotFound` when there is no such object.
pub async fn purge_log(c: &Conn<'_>, id: ObjectId) -> Result<()> {
    layer_log::purge_log(c.inner(), id).await.map_err(adapt)
}

/// From now on the writes of this object are not logged.
///
/// # Errors
/// `NotFound` when there is no such object.
pub async fn mark_unlogged(c: &Conn<'_>, id: ObjectId) -> Result<()> {
    layer_log::mark_unlogged(c.inner(), id).await.map_err(adapt)
}

/// Final delete: the object leaves with every row and its log; the envelopes of its exceptions go with it.
///
/// # Errors
/// `NotFound`, or `Invalid` when the object is not in the trash.
pub async fn delete_final(c: &Conn<'_>, id: ObjectId) -> Result<()> {
    layer::delete_final(c.inner(), &DOMAIN, id)
        .await
        .map_err(adapt)
}
