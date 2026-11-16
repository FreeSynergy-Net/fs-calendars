//! Change a calendar: name, colour, zone, owner (compare-and-set).
//!
//! Pattern: Transaction script (as `calendar_ops`).

use fs_calendars_testdata::draft::{
    CalendarInfo, CalendarPatch, Error, FieldChange, HistoryKind, ObjectId, PersonRef, Result,
    Revision, Value as Val, Zone,
};

use crate::calendar_ops::{custom_zone, one, owner_ok};
use crate::codec::{id_value, invalid, person_values, text};
use crate::envelope::{bump, log};
use crate::store::Conn;

/// Change name, colour, zone or owner: compare-and-set. A calendar in the trash is `NotFound`.
///
/// # Errors
/// `NotFound`, `Changed`, `Invalid` (empty name, zone, a profile as owner, `url`/`refresh` on an own calendar).
pub async fn update_calendar(
    c: Conn<'_>,
    actor: &PersonRef,
    now: i64,
    id: ObjectId,
    expected: Revision,
    p: &CalendarPatch,
) -> Result<CalendarInfo> {
    let r = update_in(&c, actor, now, id, expected, p).await;
    c.finish(r).await
}

async fn update_in(
    c: &Conn<'_>,
    actor: &PersonRef,
    now: i64,
    id: ObjectId,
    expected: Revision,
    p: &CalendarPatch,
) -> Result<CalendarInfo> {
    let (cur, deleted) = one(c, id, now).await?.ok_or(Error::NotFound(id))?;
    if deleted.is_some() {
        return Err(Error::NotFound(id));
    }
    if cur.revision != expected {
        return Err(Error::Changed {
            current: cur.revision,
        });
    }
    if p.url.is_some() {
        return Err(invalid("url", "only a subscription has it"));
    }
    if p.refresh.is_some() {
        return Err(invalid("refresh", "only a subscription has it"));
    }
    if let Some(o) = &p.owner {
        owner_ok(o)?;
    }
    let name = p.name.clone().unwrap_or_else(|| cur.name.clone());
    if name.trim().is_empty() {
        return Err(invalid("name", "must not be empty"));
    }
    let color = p.color.clone().unwrap_or_else(|| cur.color.clone());
    let zone = p
        .default_zone
        .clone()
        .unwrap_or_else(|| cur.default_zone.clone());
    custom_zone(zone.as_ref())?;
    if let Some(z) = &zone {
        fs_calendars_testdata::zone::resolve_at(
            z,
            &std::collections::HashMap::new(),
            "default_zone",
        )?;
    }
    let owner = p.owner.clone().unwrap_or_else(|| cur.owner.clone());
    let mut lines = Vec::new();
    if name != cur.name {
        lines.push(line(
            "name",
            Val::Text(cur.name.clone()),
            Val::Text(name.clone()),
        ));
    }
    if owner != cur.owner {
        lines.push(line(
            "owner",
            Val::Person(cur.owner.clone()),
            Val::Person(owner.clone()),
        ));
    }
    if lines.is_empty() && color == cur.color && zone == cur.default_zone {
        return Ok(cur);
    }
    bump(c, id, expected, Some(&name), now).await?;
    let [oid, oart, oabzug] = person_values(&owner);
    c.exec(
        "UPDATE calendar_collection SET name = ?, color = ?, default_zone = ?, owner_id = ?, owner_art = ?, owner_abzug = ? WHERE id = ?",
        vec![
            text(&name),
            crate::codec::opt_text(color.as_deref()),
            crate::codec::opt_text(zone.as_ref().map(Zone::id)),
            oid.clone(),
            oart.clone(),
            oabzug.clone(),
            id_value(id),
        ],
    )
    .await?;
    c.exec(
        "UPDATE objects SET attributed_to_id = ?, attributed_to_art = ?, attributed_to_abzug = ? WHERE id = ?",
        vec![oid, oart, oabzug, id_value(id)],
    )
    .await?;
    log(c, id, HistoryKind::Update, actor, now, &lines).await?;
    one(c, id, now)
        .await?
        .map(|x| x.0)
        .ok_or(Error::NotFound(id))
}

fn line(field: &str, old: Val, new: Val) -> FieldChange {
    FieldChange {
        field: field.to_owned(),
        row_key: None,
        old: Some(old),
        new: Some(new),
    }
}
