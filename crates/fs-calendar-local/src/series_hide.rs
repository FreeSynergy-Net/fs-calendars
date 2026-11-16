//! Hiding, showing and moving exceptions when a series is cut or split.
//!
//! Pattern: Transaction script (single `UPDATE`s on the exception rows; never `DELETE` + `INSERT`).

use fs_calendar_testdata::draft::{Appointment, ObjectId, Result};

use crate::codec::{id_value, int, text};
use crate::store::Conn;

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
