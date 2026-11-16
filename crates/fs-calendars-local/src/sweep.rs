//! The period run: what a deadline made final (ADR 0073). The layer runs it (`purge::sweep`); the calendar says what
//! an expired cut means: the exceptions that a `ThisAndFollowing` cut hid go, a changed occurrence that a `This` cut
//! deleted loses its fields (a plain `Excluded`) and the changed values leave the log.
//!
//! A retention (`hold`) of an appointment keeps its trashed calendar too (the trait rule "a retention goes before the
//! period"): the layer's `due` looks only at the calendar's own retention and would take the appointments along, so the
//! run first puts a temporary retention on such a calendar and its trashed appointments and takes it off afterwards
//! (the same transaction; nothing of it is visible outside). The calendar's own retention counts as well: a kept
//! calendar keeps the appointments that went with it.
//!
//! The layer lists a due calendar AND its due appointments and the final delete of the calendar takes the appointments
//! along, so the next delete would meet `NotFound` and roll the whole run back for good. The run therefore deletes the
//! due calendars first (appointments after them are not listed any more).
//!
//! Pattern: Template method (the layer owns the loop and the final delete of the trash; the hook is the calendar's
//! part), plus an Adapter for the counts.

use fs_calendars_testdata::draft::{LocalDateTime, ObjectId, PersonRef, Result};
use fs_objects::v5;
use fs_objects::v5::hold;
use fs_objects::v5::log::redact;
use fs_objects::v5::purge::{Done, HookFuture};
use fs_types as ft;

use crate::codec::{Row, get_id, get_int, id_value, int};
use crate::convert::{adapt, to_v5};
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
    let guarded = guard_kept(c, now).await?;
    let calendars = delete_due_calendars(c, now).await?;
    let done = v5::purge::sweep(c.inner(), &DOMAIN, now, actor)
        .await
        .map_err(adapt)?;
    unguard(c, guarded).await?;
    Ok(Swept {
        cuts: done.rows.iter().map(|(_, n)| *n).sum(),
        hidden: done.changed_of(HIDDEN),
        scrubbed: done.changed_of(SCRUBBED),
        objects: done.objects + calendars,
    })
}

/// The final delete of the calendars whose period is over (and that nothing keeps), with everything in them; the count
/// is every object that went (the calendar and the appointments taken along that were due themselves).
async fn delete_due_calendars(c: &Conn<'_>, now: i64) -> Result<usize> {
    let due = v5::envelope::due(c.inner(), now).await.map_err(adapt)?;
    let mut calendars = Vec::new();
    for id in &due {
        let is_calendar = c
            .query(
                "SELECT 1 FROM calendar_collection WHERE id = ?",
                vec![id_value(*id)],
            )
            .await?;
        if !is_calendar.is_empty() {
            calendars.push(*id);
        }
    }
    for id in &calendars {
        crate::envelope::delete_final(c, *id).await?;
    }
    let mut gone = 0;
    for id in &due {
        if !calendars.contains(id)
            && c.query("SELECT 1 FROM objects WHERE id = ?", vec![id_value(*id)])
                .await?
                .is_empty()
        {
            gone += 1;
        }
    }
    Ok(gone + calendars.len())
}

/// What a temporary retention replaced: the object and its own retention before (`None` = none).
type Guarded = Vec<(ObjectId, Option<Option<i64>>)>;

/// Trashed calendars that something keeps (an appointment of them, or the calendar itself): they and their trashed
/// appointments get the latest end of those retentions (`None` = without end) until the run is done.
async fn guard_kept(c: &Conn<'_>, now: i64) -> Result<Guarded> {
    let due = c
        .query(
            "SELECT k.id FROM calendar_collection k JOIN objects o ON o.id = k.id WHERE o.deleted IS NOT NULL \
             AND o.purge_after IS NOT NULL",
            Vec::new(),
        )
        .await?;
    let mut done = Guarded::new();
    for row in &due {
        let ids = crate::calendar_ops::kept_by(c, get_id(row, 0)?).await?;
        let mut latest: Option<Option<i64>> = None;
        for id in &ids {
            match hold::held_until(c.inner(), *id).await.map_err(adapt)? {
                Some(None) => latest = Some(None),
                Some(Some(t)) if t > now && latest != Some(None) => {
                    latest = Some(Some(latest.flatten().map_or(t, |b: i64| b.max(t))));
                }
                _ => {}
            }
        }
        let Some(until) = latest else { continue };
        for id in ids {
            let before = hold::held_until(c.inner(), id).await.map_err(adapt)?;
            if hold::is_held(c.inner(), id, now).await.map_err(adapt)? {
                continue;
            }
            hold::hold(c.inner(), id, until).await.map_err(adapt)?;
            done.push((id, before));
        }
    }
    Ok(done)
}

/// Put the retentions back as they were before the run.
async fn unguard(c: &Conn<'_>, guarded: Guarded) -> Result<()> {
    for (id, before) in guarded {
        match before {
            Some(until) => hold::hold(c.inner(), id, until).await,
            None => hold::release(c.inner(), id).await,
        }
        .map_err(adapt)?;
    }
    Ok(())
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
        expired(&c, row, now, actor).await.map_err(to_v5)
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
    let key = rid.canonical();
    redact(c.inner(), master, &key, "override.%")
        .await
        .map_err(adapt)?;
    Ok(1)
}
