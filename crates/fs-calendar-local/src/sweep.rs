//! The period run: what a deadline made final (ADR 0073). The layer runs it (`purge::sweep`); the calendar says what
//! an expired cut means: the exceptions that a `ThisAndFollowing` cut hid go, a changed occurrence that a `This` cut
//! deleted loses its fields (a plain `Excluded`) and the changed values leave the log.
//!
//! Pattern: Template method (the layer owns the loop and the final delete of the trash; the hook is the calendar's
//! part), plus an Adapter for the counts.

use fs_calendar_testdata::draft::{LocalDateTime, ObjectId, PersonRef, Result};
use fs_objects::v5;
use fs_objects::v5::log::redact;
use fs_objects::v5::purge::{Done, HookFuture};
use fs_types as ft;

use crate::bridge::{adapt, oid, person, person_back, to_v5};
use crate::codec::{Row, get_id, get_int, id_value, int};
use crate::read::load;
use crate::registry::DOMAIN;
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
    /// Objects whose trash period was over, removed for good with everything that hangs on them.
    pub objects: usize,
}

/// Run the deadlines of the calendar: expired cuts, then the final delete of what the trash period ended. The counts
/// are what the layer and the hook really did, not a count taken before.
///
/// # Errors
/// The engine's error.
pub async fn sweep_expired(c: &Conn<'_>, now: i64, actor: &PersonRef) -> Result<Swept> {
    let done = v5::purge::sweep(c.inner(), &DOMAIN, now, &person(actor)?)
        .await
        .map_err(adapt)?;
    Ok(Swept {
        cuts: done.rows.iter().map(|(_, n)| *n).sum(),
        hidden: done.changed_of(HIDDEN),
        scrubbed: done.changed_of(SCRUBBED),
        objects: done.objects,
    })
}

/// The kinds the hook reports (`Swept::changed` of the layer).
const HIDDEN: &str = "hidden";
const SCRUBBED: &str = "scrubbed";

/// The hook of `calendar_cut`: the layer calls it for every expired cut, before the row is deleted.
pub fn on_cut<'a>(
    c: &'a v5::Conn<'_>,
    row: &'a Row,
    now: i64,
    actor: &'a ft::PersonRef,
) -> HookFuture<'a> {
    Box::pin(async move {
        let c = Conn::Borrowed(c);
        let actor = person_back(actor).map_err(to_v5)?;
        expired(&c, row, now, &actor).await.map_err(to_v5)
    })
}

async fn expired(c: &Conn<'_>, cut: &Row, now: i64, actor: &PersonRef) -> Result<Done> {
    let (master, at) = (get_id(cut, 0)?, get_int(cut, 1)?);
    if get_int(cut, 2)? != 0 {
        let hidden = c
            .query(
                "SELECT id FROM calendar_appointment WHERE master = ? AND hidden_by = ?",
                vec![id_value(master), int(at)],
            )
            .await?;
        let mut gone = 0;
        for h in &hidden {
            let n = c
                .exec(
                    "DELETE FROM objects WHERE id = ?",
                    vec![id_value(get_id(h, 0)?)],
                )
                .await?;
            gone += usize::try_from(n).unwrap_or(0);
        }
        Ok((HIDDEN, gone))
    } else {
        Ok((SCRUBBED, scrub(c, master, at, now, actor).await?))
    }
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
    // The log of the master names the changed fields of that occurrence (`override.*`, the occurrence as row key):
    // the changes go, their entries are marked `redacted`. `touch` writes no log entry; none is needed here.
    let key = fs_calendar_testdata::draft::BasicValue::canonical(&rid);
    redact(c.inner(), oid(master), &key, "override.%")
        .await
        .map_err(adapt)?;
    Ok(1)
}
