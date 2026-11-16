//! The typed history of an appointment, read back from `activity` and `activity_change` (R10).
//!
//! Pattern: Adapter (rows to `HistoryEntry`); oldest first, keyset on the activity id (monotonic).

use fs_calendar_testdata::draft::{Error, HistoryEntry, ObjectId, Page, Place, Result, Utc};
use fs_objects::v5::log::{Kind, changes_of};

use crate::bridge::{adapt, kind_back};
use crate::bridge_values::change_back;
use crate::codec::{get_int, get_text, id_value, int, person_of, text};
use crate::envelope::state;
use crate::store::Conn;

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
    let mut items = Vec::new();
    let mut last = None;
    for row in rows.iter().take(size.max(1)) {
        let aid = get_text(row, 0)?;
        let changes = changes_of(c.inner(), &aid)
            .await
            .map_err(adapt)?
            .iter()
            .map(change_back)
            .collect::<Result<Vec<_>>>()?;
        items.push(HistoryEntry {
            at: Utc::from_millis(get_int(row, 5)?),
            actor: person_of(row, 2)?,
            kind: kind_back(Kind::from_word(&get_text(row, 1)?).map_err(adapt)?),
            changes,
        });
        last = Some(Place(aid));
    }
    Ok(Page {
        items,
        next: if rows.len() > size.max(1) { last } else { None },
    })
}
