//! Calendars: list (with the trash), create, change, delete with everything in it, bring back, purge.
//!
//! Pattern: Transaction script; each write is one transaction over the envelope, the collection row and the log (R3).
//! The appointments of a calendar that goes to the trash go with it (same instant, same deadline); they are recognised
//! by `calendar_cascade` when the calendar comes back (an appointment deleted earlier keeps its own deadline).

use fs_calendars_testdata::draft::{
    CalendarInfo, CalendarSource, Error, FieldChange, HistoryKind, ObjectId, PersonArt, PersonRef,
    Result, Revision, Trash, Value as Val, Zone,
};

use crate::calendar_tables::COLLECTION;
use crate::clock::new_id;
use crate::codec::{get_int, get_opt_int, id_value, invalid, person_of};
use crate::envelope::{insert, log};
use crate::read::Rd;
use crate::restorable::restorable;
use crate::store::Conn;
use crate::write::write_collection;

/// A calendar row with what the envelope says of its trash state.
struct Row {
    info: CalendarInfo,
    deleted: Option<i64>,
    purge_after: Option<i64>,
}

async fn rows(c: &Conn<'_>, filter: &str, params: Vec<turso::Value>) -> Result<Vec<Row>> {
    let cols: Vec<String> = COLLECTION
        .cols
        .iter()
        .map(|x| format!("k.{}", x.name))
        .collect();
    let sql = format!(
        "SELECT {}, o.rev, o.deleted, o.purge_after FROM calendar_collection k JOIN objects o ON o.id = k.id WHERE {filter} ORDER BY o.name_sort, k.id",
        cols.join(", ")
    );
    let n = COLLECTION.cols.len();
    c.query(&sql, params)
        .await?
        .iter()
        .map(|row| {
            let r = Rd {
                t: &COLLECTION,
                row,
            };
            Ok(Row {
                info: CalendarInfo {
                    id: r.id("id")?,
                    name: r.text("name")?,
                    color: r.opt_text("color")?,
                    default_zone: r
                        .opt_text("default_zone")?
                        .map(|z| crate::codec::zone_of(&z))
                        .transpose()?,
                    owner: person_of(row, 4)?,
                    source: CalendarSource::Own,
                    trash: None,
                    revision: Revision(u64::try_from(get_int(row, n)?).unwrap_or(0)),
                },
                deleted: get_opt_int(row, n + 1)?,
                purge_after: get_opt_int(row, n + 2)?,
            })
        })
        .collect()
}

/// The ids whose retention counts for a calendar in the trash: itself and what was trashed in it.
pub(crate) async fn kept_by(c: &Conn<'_>, cal: ObjectId) -> Result<Vec<ObjectId>> {
    let sql = "SELECT t.id FROM calendar_appointment t JOIN objects o ON o.id = t.id WHERE t.collection = ? AND t.master IS NULL AND o.deleted IS NOT NULL";
    let mut ids = vec![cal];
    for r in c.query(sql, vec![id_value(cal)]).await? {
        ids.push(crate::codec::get_id(&r, 0)?);
    }
    Ok(ids)
}

async fn shown(c: &Conn<'_>, mut r: Row, now: i64) -> Result<CalendarInfo> {
    if let Some(end) = r.purge_after.filter(|_| r.deleted.is_some()) {
        let ids = kept_by(c, r.info.id).await?;
        r.info.trash = Some(restorable(c, &ids, end, now).await?);
    }
    Ok(r.info)
}

/// The calendars the asker sees; `trash` chooses the living, the deleted or both.
///
/// # Errors
/// `Unavailable` for a row that does not read back.
pub async fn calendars(c: &Conn<'_>, trash: Trash, now: i64) -> Result<Vec<CalendarInfo>> {
    let filter = match trash {
        Trash::Exclude => "o.deleted IS NULL",
        Trash::Include => "1 = 1",
        Trash::Only => "o.deleted IS NOT NULL",
    };
    let mut found = rows(c, filter, Vec::new()).await?;
    if trash == Trash::Only {
        found.sort_by_key(|r| (r.deleted, r.info.id));
    }
    let mut out = Vec::new();
    for r in found {
        out.push(shown(c, r, now).await?);
    }
    Ok(out)
}

pub(crate) async fn one(
    c: &Conn<'_>,
    id: ObjectId,
    now: i64,
) -> Result<Option<(CalendarInfo, Option<i64>)>> {
    let mut found = rows(c, "k.id = ?", vec![id_value(id)]).await?;
    let Some(r) = found.pop() else {
        return Ok(None);
    };
    let deleted = r.deleted;
    Ok(Some((shown(c, r, now).await?, deleted)))
}

/// Whether the calendar is in the trash (an unknown id is not).
///
/// # Errors
/// The engine's error.
pub async fn in_trash(c: &Conn<'_>, id: ObjectId) -> Result<bool> {
    let sql = "SELECT 1 FROM calendar_collection k JOIN objects o ON o.id = k.id WHERE k.id = ? AND o.deleted IS NOT NULL";
    Ok(!c.query(sql, vec![id_value(id)]).await?.is_empty())
}

/// An owner must be a person, group or organisation; a profile never (`Invalid`).
pub(crate) fn owner_ok(owner: &PersonRef) -> Result<()> {
    if owner.art == PersonArt::Profile {
        return Err(invalid("owner", "owner.profile"));
    }
    Ok(())
}

pub(crate) fn custom_zone(zone: Option<&Zone>) -> Result<()> {
    if zone.is_some_and(crate::write::custom) {
        return Err(Error::Unsupported {
            capability: "calendar.create".to_owned(),
            reason: "custom zones are not stored yet (the calendar_custom_zone tables are missing)"
                .to_owned(),
        });
    }
    Ok(())
}

/// Create a calendar (envelope, collection row, log) in one transaction; `owner = None`: the asker.
///
/// # Errors
/// `Invalid` (empty name, unknown zone, a profile as owner), or the engine's error.
pub async fn create_calendar(
    c: Conn<'_>,
    actor: &PersonRef,
    now: i64,
    new: (&str, Option<&str>, Option<Zone>, Option<PersonRef>),
) -> Result<CalendarInfo> {
    let r = create_in(&c, actor, now, new).await;
    c.finish(r).await
}

async fn create_in(
    c: &Conn<'_>,
    actor: &PersonRef,
    now: i64,
    (name, color, zone, owner): (&str, Option<&str>, Option<Zone>, Option<PersonRef>),
) -> Result<CalendarInfo> {
    let owner = owner.unwrap_or_else(|| actor.clone());
    owner_ok(&owner)?;
    if name.trim().is_empty() {
        return Err(invalid("name", "must not be empty"));
    }
    custom_zone(zone.as_ref())?;
    if let Some(z) = &zone {
        fs_calendars_testdata::zone::resolve_at(
            z,
            &std::collections::HashMap::new(),
            "default_zone",
        )?;
    }
    let id = new_id(now)?;
    insert(c, id, "Calendar", name, &owner, now).await?;
    write_collection(c, id, name, color, zone.as_ref(), &owner).await?;
    let ch = [FieldChange {
        field: "name".to_owned(),
        row_key: None,
        old: None,
        new: Some(Val::Text(name.to_owned())),
    }];
    log(c, id, HistoryKind::Create, actor, now, &ch).await?;
    one(c, id, now)
        .await?
        .map(|x| x.0)
        .ok_or(Error::NotFound(id))
}
