//! Writes with a series scope: one occurrence (`This`) and this and the following (`ThisAndFollowing`).
//!
//! Pattern: Transaction script (as `ops`). D8: "this and the following" at the first occurrence is the whole series.
//! A partial delete leaves a cut (`calendar_cut`) so that `restore(id, scope)` can undo it until the period is over.

use chrono::Duration;
use fs_calendars_testdata::draft::{
    Appointment, AppointmentPatch, Error, FieldChange, HistoryKind, LocalDateTime, ObjectId,
    Override, OverrideChange, PersonRef, Recurrence, Related, Relation, Result, Revision, RuleEnd,
    Utc, Value as Val,
};
use fs_calendars_testdata::instances::{series_start, when_at};
use fs_calendars_testdata::patch::{apply, merge};
use fs_calendars_testdata::recur::occurrences;
use fs_calendars_testdata::scope::{occurrence, truncate};
use fs_calendars_testdata::zone::naive;

use crate::clock::new_id;
use crate::codec::{id_text, naive_secs};
use crate::envelope::{bump, insert, log};
use crate::fields::{Flat, diff, flatten};
use crate::read::{Loaded, load};
use crate::series::put_exception;
use crate::series_hide::move_to_tail;
use crate::store::Conn;
use crate::write::{validate, write_appointment};

/// A master that is live (not in the trash) and at `expected`.
pub async fn live(c: &Conn<'_>, id: ObjectId, expected: Revision) -> Result<Loaded> {
    let l = load(
        c,
        "t.id = ? AND t.master IS NULL",
        vec![crate::codec::id_value(id)],
    )
    .await?
    .ok_or(Error::NotFound(id))?;
    if l.deleted.is_some() {
        return Err(Error::NotFound(id));
    }
    if l.appt.revision != expected {
        return Err(Error::Changed {
            current: l.appt.revision,
        });
    }
    Ok(l)
}

pub fn marked(rid: LocalDateTime, what: &str) -> FieldChange {
    FieldChange {
        field: "override".to_owned(),
        row_key: Some(rid.canonical()),
        old: None,
        new: Some(Val::Text(what.to_owned())),
    }
}

/// Change one occurrence.
///
/// # Errors
/// `NotFound`, `Changed`, `Invalid` (not an occurrence), `Unsupported`.
pub async fn update_this(
    c: &Conn<'_>,
    actor: &PersonRef,
    now: i64,
    id: ObjectId,
    expected: Revision,
    p: &AppointmentPatch,
    rid: LocalDateTime,
) -> Result<Appointment> {
    let cur = live(c, id, expected).await?.appt;
    occurrence(&cur, rid)?;
    let existing = cur.overrides.iter().find(|o| o.recurrence_id == rid);
    let before = match existing.map(|o| &o.change) {
        Some(OverrideChange::Changed(b)) => b.clone(),
        _ => AppointmentPatch::default(),
    };
    let merged = merge(&before, p);
    if merged == AppointmentPatch::default() || (existing.is_some() && merged == before) {
        return Ok(cur);
    }
    let mut probe = cur.clone();
    apply(&mut probe, &merged);
    let mut one = cur.clone();
    one.overrides.push(Override {
        recurrence_id: rid,
        change: OverrideChange::Changed(merged.clone()),
    });
    validate(&one, "event.update")?;
    let mut changes = vec![marked(rid, "changed")];
    changes.extend(
        diff(&flatten(&cur), &flatten(&probe))
            .into_iter()
            .map(|mut ch| {
                ch.field = format!("override.{}", ch.field);
                ch.row_key = Some(rid.canonical());
                ch
            }),
    );
    put_exception(
        c,
        &cur,
        &Override {
            recurrence_id: rid,
            change: OverrideChange::Changed(merged),
        },
        actor,
        now,
    )
    .await?;
    bump(c, id, expected, None, now).await?;
    log(c, id, HistoryKind::Update, actor, now, &changes).await?;
    Ok(load(c, "t.id = ?", vec![crate::codec::id_value(id)])
        .await?
        .ok_or(Error::NotFound(id))?
        .appt)
}

/// Change this and the following occurrences: the series is split (new `uid`, `First`/`Next`); the tail is returned.
///
/// # Errors
/// As [`update_this`].
pub async fn update_following(
    c: &Conn<'_>,
    actor: &PersonRef,
    now: i64,
    id: ObjectId,
    expected: Revision,
    p: &AppointmentPatch,
    rid: LocalDateTime,
) -> Result<Appointment> {
    let cur = live(c, id, expected).await?.appt;
    let at = occurrence(&cur, rid)?;
    let start = series_start(&cur.when).unwrap_or(at);
    let tail_id = new_id(now)?;
    let first_uid = cur
        .related
        .iter()
        .find(|r| r.relation == Relation::First)
        .map_or_else(|| cur.uid.clone(), |r| r.uid.clone());
    let mut tail = cur.clone();
    tail.id = tail_id;
    tail.uid = format!("{}@local", id_text(tail_id));
    (tail.revision, tail.sequence, tail.created, tail.updated) =
        (Revision(1), 0, Utc::from_millis(now), Utc::from_millis(now));
    tail.when = when_at(&cur.when, at);
    tail.overrides
        .retain(|o| naive(o.recurrence_id).is_some_and(|n| n >= at));
    // Every rule counts its own occurrences: the part of a `Count` that lies before the split stays with the head.
    for (t, r) in tail.recurrence.iter_mut().zip(&cur.recurrence) {
        if let (Recurrence::Rule(t), Recurrence::Rule(r)) = (t, r) {
            if let RuleEnd::Count(n) = t.end {
                let before = occurrences(r, start, at - Duration::seconds(1)).len();
                t.end = RuleEnd::Count(n.saturating_sub(u32::try_from(before).unwrap_or(u32::MAX)));
            }
        }
    }
    tail.related.retain(|r| r.relation != Relation::First);
    tail.related.push(Related {
        uid: first_uid,
        relation: Relation::First,
    });
    apply(&mut tail, p);
    validate(&tail, "event.update")?;
    let mut head = cur.clone();
    truncate(&mut head, at);
    head.related.retain(|r| r.relation != Relation::Next);
    head.related.push(Related {
        uid: tail.uid.clone(),
        relation: Relation::Next,
    });
    head.sequence += 1;
    head.updated = Utc::from_millis(now);
    bump(c, id, expected, None, now).await?;
    write_appointment(c, &head).await?;
    insert(c, tail_id, "Event", &tail.title, actor, now).await?;
    write_appointment(c, &tail).await?;
    move_to_tail(c, id, &tail, naive_secs(at)).await?;
    // The head logs what changed on it: the end of the rules, `related` (`Next`) and the sequence.
    let changes = diff(&flatten(&cur), &flatten(&head));
    log(c, id, HistoryKind::Update, actor, now, &changes).await?;
    log(
        c,
        tail_id,
        HistoryKind::Create,
        actor,
        now,
        &diff(&Flat::new(), &flatten(&tail)),
    )
    .await?;
    Ok(load(c, "t.id = ?", vec![crate::codec::id_value(tail_id)])
        .await?
        .ok_or(Error::NotFound(tail_id))?
        .appt)
}
