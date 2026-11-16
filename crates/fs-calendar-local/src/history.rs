//! The typed history of an appointment, read back from `activity` and `activity_change` (R10).
//!
//! Pattern: Adapter (rows to `HistoryEntry`); oldest first, keyset on the activity id (monotonic).

use fs_calendar_testdata::draft::{
    Error, FieldChange, HistoryEntry, ObjectId, Page, Place, Result, Utc,
};

use crate::change_codec::decode;
use crate::codec::{get_int, get_text, id_value, int, person_of, text};
use crate::envelope::state;
use crate::object_tables::ACTIVITY_CHANGE;
use crate::store::Conn;
use crate::words::{HISTORY_KIND, from_word};

/// One page of the log of an object.
///
/// # Errors
/// `NotFound` when there is no such object; `Unavailable` for a row that does not read back.
pub async fn history(
    c: &Conn<'_>,
    id: ObjectId,
    after: Option<&Place>,
    size: usize,
) -> Result<Page<HistoryEntry>> {
    state(c, id).await?.ok_or(Error::NotFound(id))?;
    let mut sql =
        "SELECT id, kind, actor_id, actor_art, actor_abzug, at FROM activity WHERE object = ?"
            .to_owned();
    let mut params = vec![id_value(id)];
    if let Some(p) = after {
        sql.push_str(" AND id > ?");
        params.push(text(&p.0));
    }
    sql.push_str(" ORDER BY id LIMIT ?");
    params.push(int(i64::try_from(size.max(1) + 1).unwrap_or(i64::MAX)));
    let rows = c.query(&sql, params).await?;
    let names: Vec<&str> = ACTIVITY_CHANGE.cols.iter().map(|x| x.name).collect();
    let mut items = Vec::new();
    let mut last = None;
    for row in rows.iter().take(size.max(1)) {
        let aid = get_text(row, 0)?;
        let mut changes = Vec::new();
        let cr = c
            .query(
                &format!(
                    "SELECT {} FROM activity_change WHERE activity = ? ORDER BY seq",
                    names.join(", ")
                ),
                vec![text(&aid)],
            )
            .await?;
        for r in &cr {
            let kind = get_text(r, 4)?;
            changes.push(FieldChange {
                field: get_text(r, 2)?,
                row_key: crate::codec::get_opt_text(r, 3)?,
                old: decode(&kind, r, 5)?,
                new: decode(&kind, r, 15)?,
            });
        }
        items.push(HistoryEntry {
            at: Utc::from_millis(get_int(row, 5)?),
            actor: person_of(row, 2)?,
            kind: from_word(&HISTORY_KIND, &get_text(row, 1)?)?,
            changes,
        });
        last = Some(Place(aid));
    }
    Ok(Page {
        items,
        next: if rows.len() > size.max(1) { last } else { None },
    })
}
