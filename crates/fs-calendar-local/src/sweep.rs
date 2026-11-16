//! The period run: what a deadline made final (ADR 0073). Cuts whose restore period is over are forgotten, and the
//! exceptions that such a cut had hidden go with them.
//!
//! Pattern: Transaction script (one pass over the expired cuts, in the caller's transaction).

use fs_calendar_testdata::draft::Result;

use crate::codec::{get_id, get_int, id_value, int};
use crate::store::Conn;

/// What a run removed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Swept {
    /// Cuts (`calendar_cut` and their rule ends) whose period is over.
    pub cuts: usize,
    /// Hidden exceptions of `ThisAndFollowing` cuts (rows and envelopes).
    pub hidden: usize,
}

/// Remove the cuts with `until <= now` and the exceptions they hid.
///
/// # Errors
/// The engine's error.
pub async fn sweep_expired(c: &Conn<'_>, now: i64) -> Result<Swept> {
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
        c.exec("DELETE FROM calendar_cut WHERE appointment = ? AND recurrence_id = ? AND following = ?", vec![id_value(master), int(at), int(get_int(cut, 2)?)]).await?;
        out.cuts += 1;
    }
    Ok(out)
}
