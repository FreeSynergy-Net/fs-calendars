//! The period run: what a deadline made final (ADR 0073). Cuts whose restore period is over are forgotten, and the
//! exceptions that such a cut had hidden go with them.
//!
//! Pattern: Transaction script (one pass over the expired cuts, in the caller's transaction).

use fs_calendar_testdata::draft::{LocalDateTime, ObjectId, PersonRef, Result};

use crate::codec::{get_id, get_int, id_value, int};
use crate::read::load;
use crate::series::scrub_exception;
use crate::store::Conn;

/// What a run removed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Swept {
    /// Cuts (`calendar_cut` and their rule ends) whose period is over.
    pub cuts: usize,
    /// Hidden exceptions of `ThisAndFollowing` cuts (rows and envelopes).
    pub hidden: usize,
    /// Deleted changed occurrences made a plain `Excluded` (their fields are gone, ADR 0073 data minimisation).
    pub scrubbed: usize,
}

/// Remove the cuts with `until <= now` and the exceptions they hid.
///
/// # Errors
/// The engine's error.
pub async fn sweep_expired(c: &Conn<'_>, now: i64, actor: &PersonRef) -> Result<Swept> {
    let cuts = c
        .query(
            "SELECT appointment, recurrence_id, following FROM calendar_cut WHERE until <= ?",
            vec![int(now)],
        )
        .await?;
    let mut out = Swept::default();
    for cut in &cuts {
        let (master, at) = (get_id(cut, 0)?, get_int(cut, 1)?);
        if get_int(cut, 2)? != 0 {
            let hidden = c
                .query(
                    "SELECT id FROM calendar_appointment WHERE master = ? AND hidden_by = ?",
                    vec![id_value(master), int(at)],
                )
                .await?;
            for h in &hidden {
                c.exec(
                    "DELETE FROM objects WHERE id = ?",
                    vec![id_value(get_id(h, 0)?)],
                )
                .await?;
                out.hidden += 1;
            }
        }
        if get_int(cut, 2)? == 0 {
            out.scrubbed += scrub(c, master, at, now, actor).await?;
        }
        c.exec("DELETE FROM calendar_cut WHERE appointment = ? AND recurrence_id = ? AND following = ?", vec![id_value(master), int(at), int(get_int(cut, 2)?)]).await?;
        out.cuts += 1;
    }
    Ok(out)
}

/// An expired `This` cut of a changed occurrence: the row still holds the fields of the change (`restore` needed
/// them). After the period they are removed: the occurrence stays deleted, as a plain `Excluded`.
async fn scrub(
    c: &Conn<'_>,
    master: ObjectId,
    at: i64,
    now: i64,
    actor: &PersonRef,
) -> Result<usize> {
    let rows = c
        .query(
            "SELECT 1 FROM calendar_appointment e WHERE e.master = ? AND e.recurrence_id = ? AND e.excluded = 1 \
             AND EXISTS (SELECT 1 FROM calendar_override_field f WHERE f.appointment = e.id)",
            vec![id_value(master), int(at)],
        )
        .await?;
    if rows.is_empty() {
        return Ok(0);
    }
    let Some(m) = load(c, "t.id = ? AND t.master IS NULL", vec![id_value(master)]).await? else {
        return Ok(0);
    };
    let rid: LocalDateTime = crate::codec::int_to_local(at)?;
    scrub_exception(c, &m.appt, rid, actor, now).await?;
    Ok(1)
}
