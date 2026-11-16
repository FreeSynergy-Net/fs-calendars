//! Typed queries (R6): the list by hull and keyset, the window by hull and exact placement in the viewer's zone.
//!
//! Pattern: Query object (the SQL is built from the typed query; values are bound, never pasted).

use std::collections::HashMap;

use fs_calendar_testdata::draft::{
    Appointment, AppointmentQuery, AppointmentSummary, Attendee, Damaged, Error, Instance,
    ObjectId, Page, ParticipationStatus, Place, Result, Utc, Window, Zone, ZoneKind,
};
use fs_calendar_testdata::zone::bounds;
use turso::Value;

use crate::codec::{get_id, get_int, get_text, id_text, id_value, int, text};
use crate::read::{Loaded, head, select};
use crate::store::Conn;
use crate::words::{PARTICIPATION, from_word};

fn calendars_in(calendars: &[ObjectId], sql: &mut String, params: &mut Vec<Value>) {
    if !calendars.is_empty() {
        let marks = vec!["?"; calendars.len()].join(",");
        sql.push_str(" AND t.collection IN (");
        sql.push_str(&marks);
        sql.push(')');
        params.extend(calendars.iter().map(|c| id_value(*c)));
    }
}

fn place_of(l: &Loaded, min: i64) -> Place {
    Place(format!("{min}:{}", id_text(l.appt.id)))
}

fn after_of(p: &Place) -> Result<(i64, String)> {
    let (m, id) =
        p.0.split_once(':')
            .ok_or_else(|| crate::codec::invalid("after", "not a place of this provider"))?;
    Ok((
        m.parse()
            .map_err(|_| crate::codec::invalid("after", "not a place of this provider"))?,
        id.to_owned(),
    ))
}

/// The appointments of a query, by hull start then id, `size` per page.
///
/// # Errors
/// `Invalid` for a place of another provider; `Unavailable` for a row that does not read back.
pub async fn list(
    c: &Conn<'_>,
    q: &AppointmentQuery,
    after: Option<&Place>,
    size: usize,
) -> Result<Page<AppointmentSummary>> {
    let mut sql = format!("{} WHERE t.master IS NULL", select());
    let mut params: Vec<Value> = Vec::new();
    if !q.include_deleted {
        sql.push_str(" AND o.deleted IS NULL");
    }
    calendars_in(&q.calendars, &mut sql, &mut params);
    if let Some(t) = &q.text {
        sql.push_str(" AND (instr(lower(t.title), ?) > 0 OR instr(lower(t.description), ?) > 0)");
        params.extend([text(&t.to_lowercase()), text(&t.to_lowercase())]);
    }
    if let Some(from) = q.from {
        sql.push_str(" AND t.when_utc_max > ?");
        params.push(int(from.millis()));
    }
    if let Some(to) = q.to {
        sql.push_str(" AND t.when_utc_min < ?");
        params.push(int(to.millis()));
    }
    if let Some(who) = &q.participant {
        let (kind, id) = match who {
            Attendee::Actor(p) => (
                "actor",
                fs_calendar_testdata::draft::BasicValue::canonical(&p.id),
            ),
            Attendee::Resource(r) => (
                "resource",
                fs_calendar_testdata::draft::BasicValue::canonical(r),
            ),
            Attendee::Location(r) => (
                "location",
                fs_calendar_testdata::draft::BasicValue::canonical(r),
            ),
        };
        sql.push_str(" AND EXISTS (SELECT 1 FROM calendar_participant p WHERE p.appointment = t.id AND p.who_kind = ? AND p.who_id = ?)");
        params.extend([text(kind), text(&id)]);
    }
    if let Some(p) = after {
        let (m, id) = after_of(p)?;
        sql.push_str(" AND (t.when_utc_min > ? OR (t.when_utc_min = ? AND t.id > ?))");
        params.extend([int(m), int(m), text(&id)]);
    }
    sql.push_str(" ORDER BY t.when_utc_min, t.id LIMIT ?");
    params.push(int(i64::try_from(size.max(1) + 1).unwrap_or(i64::MAX)));
    let rows = c.query(&sql, params).await?;
    let mut items = Vec::new();
    let mut last = None;
    for row in rows.iter().take(size.max(1)) {
        let l = head(row)?;
        let min = get_int(
            row,
            crate::calendar_tables::APPOINTMENT
                .cols
                .iter()
                .position(|x| x.name == "when_utc_min")
                .unwrap_or(0),
        )?;
        last = Some(place_of(&l, min));
        let a = &l.appt;
        items.push(AppointmentSummary {
            id: a.id,
            calendar: a.calendar,
            uid: a.uid.clone(),
            title: a.title.clone(),
            when: a.when.clone(),
            recurring: false,
            revision: a.revision,
            restorable_until: l.purge_after.map(Utc::from_millis),
        });
    }
    let next = if rows.len() > size.max(1) { last } else { None };
    Ok(Page { items, next })
}

async fn instance(
    c: &Conn<'_>,
    a: &Appointment,
    view: &Zone,
    me: &Attendee,
) -> Result<Option<(Utc, Utc, Instance)>> {
    let (s, e) = bounds(&a.when, &HashMap::new(), view)?;
    let count = c
        .query(
            "SELECT count(*) FROM calendar_participant WHERE appointment = ?",
            vec![id_value(a.id)],
        )
        .await?;
    let mine = match me {
        Attendee::Actor(p) => c.query("SELECT status FROM calendar_participant WHERE appointment = ? AND who_kind = 'actor' AND who_id = ?", vec![id_value(a.id), text(&fs_calendar_testdata::draft::BasicValue::canonical(&p.id))]).await?,
        _ => Vec::new(),
    };
    let my_status: Option<ParticipationStatus> = mine
        .first()
        .map(|r| get_text(r, 0).and_then(|w| from_word(&PARTICIPATION, &w)))
        .transpose()?;
    let n = count.first().map_or(Ok(0), |r| get_int(r, 0))?;
    Ok(Some((
        s,
        e,
        Instance {
            appointment: a.id,
            calendar: a.calendar,
            recurrence_id: None,
            title: a.title.clone(),
            when: a.when.clone(),
            start: s,
            end: e,
            status: a.status,
            free_busy: a.free_busy,
            privacy: a.privacy,
            overridden: false,
            recurring: false,
            participant_count: u32::try_from(n).unwrap_or(0),
            my_status,
        },
    )))
}

/// The occurrences that overlap `[from, to)`, placed in the viewer's zone.
///
/// # Errors
/// `UnknownZone` for a custom `view_zone`; a damaged row costs only itself (`Window::damaged`).
pub async fn window(
    c: &Conn<'_>,
    from: Utc,
    to: Utc,
    view: &Zone,
    calendars: &[ObjectId],
    me: &Attendee,
) -> Result<Window> {
    if view.kind() == ZoneKind::Custom {
        return Err(Error::UnknownZone(view.id().to_owned()));
    }
    let mut sql = format!(
        "{} WHERE t.master IS NULL AND o.deleted IS NULL AND t.when_utc_max > ? AND t.when_utc_min < ?",
        select()
    );
    let mut params = vec![int(from.millis()), int(to.millis())];
    calendars_in(calendars, &mut sql, &mut params);
    let rows = c
        .query(&format!("{sql} ORDER BY t.when_utc_min, t.id"), params)
        .await?;
    let mut out = Window::default();
    for row in &rows {
        match head(row) {
            Ok(l) => match instance(c, &l.appt, view, me).await {
                Ok(Some((s, e, i))) if s < to && (e > from || (s == e && s >= from)) => {
                    out.instances.push(i);
                }
                Ok(_) => {}
                Err(Error::UnknownZone(z)) => out.damaged.push(Damaged {
                    appointment: l.appt.id,
                    recurrence_id: None,
                    reason: format!("unknown zone {z}"),
                    cut: false,
                }),
                Err(e) => return Err(e),
            },
            Err(e) => {
                if let Ok(id) = get_id(row, 0) {
                    out.damaged.push(Damaged {
                        appointment: id,
                        recurrence_id: None,
                        reason: format!("{e:?}"),
                        cut: false,
                    });
                }
            }
        }
    }
    out.instances.sort_by_key(|i| (i.start, i.appointment));
    Ok(out)
}
