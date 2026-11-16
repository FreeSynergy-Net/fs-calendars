//! What `restorable` says for a trashed object: the period, a retention (`object_hold`) and "never automatically".
//!
//! Pattern: Adapter over the layer's hold functions (fs-objects v0.0.3). The layer shows `Never` for BOTH "a hold
//! without end" and "the trash is not emptied"; the trait wants more: `Held(Some(end))` for a hold WITH an end even
//! when nothing is emptied automatically (K8), `Held(None)` without end, `Manual` for "never automatically" alone.
//! A retention that has ended does not count.

use fs_calendars_testdata::draft::{ObjectId, Restorable, Result, Utc};
use fs_objects::v5::hold;

use crate::convert::adapt;
use crate::store::Conn;

/// The state of something trashed with `period_end` (UTC ms, the layer's `purge_after`), kept by the holds of `ids`.
///
/// # Errors
/// The engine's error, or a damaged row.
pub async fn restorable(
    c: &Conn<'_>,
    ids: &[ObjectId],
    period_end: i64,
    now: i64,
) -> Result<Restorable> {
    let auto = hold::trash_auto(c.inner()).await.map_err(adapt)?;
    let mut best: Option<i64> = None;
    for id in ids {
        match hold::held_until(c.inner(), *id).await.map_err(adapt)? {
            Some(None) => return Ok(Restorable::Held(None)),
            Some(Some(t)) if t > now => best = Some(best.map_or(t, |b| b.max(t))),
            _ => {}
        }
    }
    Ok(match best {
        Some(t) => Restorable::Held(Some(Utc::from_millis(if auto {
            t.max(period_end)
        } else {
            t
        }))),
        None if auto => Restorable::Until(Utc::from_millis(period_end)),
        None => Restorable::Manual,
    })
}

/// Whether any of these objects is kept by a retention that still lasts.
///
/// # Errors
/// The engine's error, or a damaged row.
pub async fn is_kept(c: &Conn<'_>, ids: &[ObjectId], now: i64) -> Result<bool> {
    for id in ids {
        if hold::is_held(c.inner(), *id, now).await.map_err(adapt)? {
            return Ok(true);
        }
    }
    Ok(false)
}
