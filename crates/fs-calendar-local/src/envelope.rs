//! The envelope row of an object and the log: compare-and-set on `rev` (R7), trash instead of delete (R4), the typed
//! log (R10). Every function runs inside the caller's transaction (R3).
//!
//! Pattern: Transaction script (small functions on one connection guard; the caller owns commit and rollback).

use fs_calendar_testdata::draft::{
    Error, FieldChange, HistoryKind, ObjectId, PersonRef, Result, Revision,
};
use turso::Value;

use crate::change_codec::encode;
use crate::clock::new_id;
use crate::codec::{
    get_int, get_opt_int, get_text, id_text, id_value, int, opt_text, person_values, text,
};
use crate::object_tables::{ACTIVITY, ACTIVITY_CHANGE, OBJECTS};
use crate::store::Conn;
use crate::words::{HISTORY_KIND, word};

pub const DEFAULT_TRASH_DAYS: i64 = 30;
const DAY_MS: i64 = 86_400_000;

/// What the envelope says about an object.
#[derive(Clone, Copy, Debug)]
pub struct State {
    pub rev: u64,
    pub deleted: Option<i64>,
    pub purge_after: Option<i64>,
}

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
    let [pid, part, pabzug] = person_values(owner);
    c.insert(
        &OBJECTS,
        vec![
            id_value(id),
            Value::Null,
            text(kind),
            text(name),
            text(&name.to_lowercase()),
            pid,
            part,
            pabzug,
            int(now),
            int(now),
            Value::Null,
            Value::Null,
            Value::Null,
            int(1),
            Value::Null,
            Value::Null,
        ],
    )
    .await
}

/// The envelope state of an object, `None` when there is no such row.
///
/// # Errors
/// The engine's error, or a damaged row.
pub async fn state(c: &Conn<'_>, id: ObjectId) -> Result<Option<State>> {
    let rows = c
        .query(
            "SELECT rev, deleted, purge_after FROM objects WHERE id = ?",
            vec![id_value(id)],
        )
        .await?;
    rows.first()
        .map(|r| {
            Ok(State {
                rev: u64::try_from(get_int(r, 0)?).unwrap_or(0),
                deleted: get_opt_int(r, 1)?,
                purge_after: get_opt_int(r, 2)?,
            })
        })
        .transpose()
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
    let exp = i64::try_from(expected.0).unwrap_or(i64::MAX);
    let n = match name {
        Some(n) => c.exec("UPDATE objects SET rev = rev + 1, updated = ?, name = ?, name_sort = ? WHERE id = ? AND rev = ? AND deleted IS NULL", vec![int(now), text(n), text(&n.to_lowercase()), id_value(id), int(exp)]).await?,
        None => c.exec("UPDATE objects SET rev = rev + 1, updated = ? WHERE id = ? AND rev = ? AND deleted IS NULL", vec![int(now), id_value(id), int(exp)]).await?,
    };
    if n == 1 {
        return Ok(Revision(expected.0 + 1));
    }
    match state(c, id).await? {
        Some(s) if s.deleted.is_none() => Err(Error::Changed {
            current: Revision(s.rev),
        }),
        _ => Err(Error::NotFound(id)),
    }
}

/// The trash period in days: `setting.trash_days`, else 30 (draft section 3).
///
/// # Errors
/// The engine's error.
pub async fn trash_days(c: &Conn<'_>) -> Result<i64> {
    let rows = c
        .query(
            "SELECT int_value FROM setting WHERE key = 'trash_days'",
            Vec::new(),
        )
        .await?;
    rows.first()
        .map_or(Ok(DEFAULT_TRASH_DAYS), |r| get_int(r, 0))
}

/// Move an object to the trash: `deleted = now`, `purge_after = now + trash_days`, fixed now (draft R4).
///
/// # Errors
/// As [`bump`].
pub async fn trash(
    c: &Conn<'_>,
    id: ObjectId,
    expected: Revision,
    now: i64,
) -> Result<(Revision, i64)> {
    let until = now + trash_days(c).await? * DAY_MS;
    let exp = i64::try_from(expected.0).unwrap_or(i64::MAX);
    let n = c.exec("UPDATE objects SET rev = rev + 1, updated = ?, deleted = ?, purge_after = ? WHERE id = ? AND rev = ? AND deleted IS NULL", vec![int(now), int(now), int(until), id_value(id), int(exp)]).await?;
    if n == 1 {
        return Ok((Revision(expected.0 + 1), until));
    }
    match state(c, id).await? {
        Some(s) if s.deleted.is_none() => Err(Error::Changed {
            current: Revision(s.rev),
        }),
        _ => Err(Error::NotFound(id)),
    }
}

/// Take an object out of the trash while `purge_after` has not passed.
///
/// # Errors
/// `NotFound` when there is no row or the period is over; `Invalid` when it is not in the trash.
pub async fn untrash(c: &Conn<'_>, id: ObjectId, now: i64) -> Result<Revision> {
    let s = state(c, id).await?.ok_or(Error::NotFound(id))?;
    match (s.deleted, s.purge_after) {
        (Some(_), Some(until)) if until > now => {
            c.exec("UPDATE objects SET rev = rev + 1, updated = ?, deleted = NULL, purge_after = NULL WHERE id = ?", vec![int(now), id_value(id)]).await?;
            Ok(Revision(s.rev + 1))
        }
        (Some(_), _) => Err(Error::NotFound(id)),
        _ => Err(crate::codec::invalid("id", "not in the trash")),
    }
}

/// Write one log entry with its typed changes.
///
/// # Errors
/// `Invalid` (a value does not fit) or the engine's error.
pub async fn log(
    c: &Conn<'_>,
    id: ObjectId,
    kind: HistoryKind,
    actor: &PersonRef,
    now: i64,
    changes: &[FieldChange],
) -> Result<()> {
    let unlogged = c
        .query(
            "SELECT 1 FROM unlogged_ids WHERE object_id = ?",
            vec![id_value(id)],
        )
        .await?;
    if !unlogged.is_empty() {
        return Ok(());
    }
    let aid = id_text(new_id(now));
    let [pid, part, pabzug] = person_values(actor);
    let kind_word = word(&HISTORY_KIND, kind);
    c.insert(
        &ACTIVITY,
        vec![
            text(&aid),
            text(kind_word),
            id_value(id),
            pid,
            part,
            pabzug,
            int(now),
            Value::Null,
            int(0),
            int(0),
            int(1),
        ],
    )
    .await?;
    for (seq, ch) in changes.iter().enumerate() {
        let (ko, old) = encode(ch.old.as_ref())?;
        let (kn, new) = encode(ch.new.as_ref())?;
        let vk = kn
            .or(ko)
            .ok_or_else(|| crate::codec::invalid("changes", "a change without any value"))?;
        let mut row = vec![
            text(&aid),
            int(i64::try_from(seq).unwrap_or(0)),
            text(&ch.field),
            opt_text(ch.row_key.as_deref()),
            text(vk),
        ];
        row.extend(old);
        row.extend(new);
        c.insert(&ACTIVITY_CHANGE, row).await?;
    }
    c.exec("INSERT INTO meta (key, value) VALUES ('activity_high', ?) ON CONFLICT (key) DO UPDATE SET value = excluded.value", vec![text(&aid)]).await?;
    Ok(())
}

/// Log purge (ADR 0029): remove the log of an object, keep the object, remember the id in `purged_objects`.
///
/// # Errors
/// `NotFound` when there is no such object.
pub async fn purge_log(c: &Conn<'_>, id: ObjectId) -> Result<()> {
    state(c, id).await?.ok_or(Error::NotFound(id))?;
    c.exec("DELETE FROM activity WHERE object = ?", vec![id_value(id)])
        .await?;
    c.exec(
        "INSERT INTO purged_objects (object_id) VALUES (?) ON CONFLICT (object_id) DO NOTHING",
        vec![id_value(id)],
    )
    .await?;
    Ok(())
}

/// From now on the writes of this object are not logged (as v4 `unlogged_ids`).
///
/// # Errors
/// `NotFound` when there is no such object.
pub async fn mark_unlogged(c: &Conn<'_>, id: ObjectId) -> Result<()> {
    state(c, id).await?.ok_or(Error::NotFound(id))?;
    c.exec(
        "INSERT INTO unlogged_ids (object_id) VALUES (?) ON CONFLICT (object_id) DO NOTHING",
        vec![id_value(id)],
    )
    .await?;
    Ok(())
}

/// Final delete (end of the trash, ADR 0073): the object leaves with every row and its log. Only for an object in the
/// trash; the envelopes of the exceptions of a series go with it.
///
/// # Errors
/// `NotFound`, or `Invalid` when the object is not in the trash.
pub async fn delete_final(c: &Conn<'_>, id: ObjectId) -> Result<()> {
    let s = state(c, id).await?.ok_or(Error::NotFound(id))?;
    if s.deleted.is_none() {
        return Err(crate::codec::invalid("id", "not in the trash"));
    }
    let ex = c
        .query(
            "SELECT id FROM calendar_appointment WHERE master = ?",
            vec![id_value(id)],
        )
        .await?;
    for r in &ex {
        c.exec(
            "DELETE FROM objects WHERE id = ?",
            vec![Value::Text(get_text(r, 0)?)],
        )
        .await?;
    }
    c.exec("DELETE FROM objects WHERE id = ?", vec![id_value(id)])
        .await?;
    Ok(())
}
