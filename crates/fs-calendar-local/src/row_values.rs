//! The values of the rows of an appointment (column order of the specification), the hull (R11) and the series end.
//!
//! Pattern: Adapter (the trait's types to engine values); pure functions, no engine access.

use std::collections::HashMap;

use fs_calendar_testdata::draft::{
    Alert, Appointment, Attendee, LocalDateTime, ObjectId, Participant, Result, RuleEnd, Trigger,
    When, Zone,
};
use fs_calendar_testdata::instances::{series_start, when_at};
use fs_calendar_testdata::recur::occurrences;
use fs_calendar_testdata::zone::{bounds, iana};
use turso::Value;

use crate::codec::{
    boolean, date_to_int, id_value, int, local_to_int, opt_int, opt_text, person_values, text,
};
use crate::words::{
    ALERT_ACTION, FREE_BUSY, PARTICIPATION, PRIVACY, RELATIVE_TO, STATUS, TEXT_TYPE, word,
};

/// Largest zone offset the hull allows for (draft section 2.3).
const HULL_PAD_MS: i64 = 16 * 3_600_000;

/// The end of the whole series in ms (`None` = endless); a single appointment ends with its hull.
///
/// # Errors
/// `UnknownZone`, or `Invalid` for a time that does not exist.
pub fn series_end(a: &Appointment) -> Result<Option<i64>> {
    let (_, own_end) = hull(&a.when)?;
    if a.recurrence.is_empty() {
        return Ok(Some(own_end));
    }
    if a.recurrence.iter().any(|r| r.end == RuleEnd::Never) {
        return Ok(None);
    }
    let Some(start) = series_start(&a.when) else {
        return Ok(Some(own_end));
    };
    let far = chrono::NaiveDate::from_ymd_opt(9999, 12, 31)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .unwrap_or(start);
    let last = a
        .recurrence
        .iter()
        .filter_map(|r| occurrences(r, start, far).into_iter().max())
        .max();
    Ok(Some(match last {
        Some(at) => hull(&when_at(&a.when, at))?.1,
        None => own_end,
    }))
}

/// The hull `(utc_min, utc_max)` in ms: exact for a timed one, widened by 16 h for floating and all-day (R11).
///
/// # Errors
/// `UnknownZone`, or `Invalid` for a time that does not exist.
pub fn hull(w: &When) -> Result<(i64, i64)> {
    let (s, e) = bounds(w, &HashMap::new(), &iana("UTC"))?;
    Ok(match w {
        When::Timed { .. } => (s.millis(), e.millis()),
        _ => (s.millis() - HULL_PAD_MS, e.millis() + HULL_PAD_MS),
    })
}

pub fn when_values(w: &When) -> Result<Vec<Value>> {
    let zone = |z: &Zone| text(z.id());
    Ok(match w {
        When::Timed {
            start,
            zone: z,
            length,
            end_zone,
            zone_assumed,
        } => vec![
            text("timed"),
            int(local_to_int(*start)?),
            Value::Null,
            zone(z),
            end_zone.as_ref().map_or(Value::Null, zone),
            boolean(*zone_assumed),
            int(i64::from(length.days)),
            int(i64::from(length.seconds)),
        ],
        When::Floating { start, length } => vec![
            text("floating"),
            int(local_to_int(*start)?),
            Value::Null,
            Value::Null,
            Value::Null,
            boolean(false),
            int(i64::from(length.days)),
            int(i64::from(length.seconds)),
        ],
        When::AllDay { first, days } => vec![
            text("all_day"),
            Value::Null,
            int(date_to_int(*first)?),
            Value::Null,
            Value::Null,
            boolean(false),
            int(i64::from(*days)),
            int(0),
        ],
    })
}

/// An exception row's link to its master: `(master, recurrence_id, excluded)`.
pub type Link = (ObjectId, LocalDateTime, bool);

pub fn appointment_values(a: &Appointment, link: Option<Link>) -> Result<Vec<Value>> {
    let (min, max) = hull(&a.when)?;
    let (master, rid, excluded) = match link {
        Some((m, r, e)) => (id_value(m), int(local_to_int(r)?), e),
        None => (Value::Null, Value::Null, false),
    };
    let mut v = vec![
        id_value(a.id),
        id_value(a.calendar),
        text(&a.uid),
        master,
        rid,
        boolean(excluded),
        int(i64::from(a.sequence)),
        int(a.created.millis()),
        int(a.updated.millis()),
        text(&a.title),
        text(&a.description),
        text(word(&TEXT_TYPE, a.description_type)),
        opt_text(a.locale.as_deref()),
    ];
    v.extend(when_values(&a.when)?);
    v.extend([
        int(min),
        int(max),
        text(word(&STATUS, a.status)),
        text(word(&FREE_BUSY, a.free_busy)),
        text(word(&PRIVACY, a.privacy)),
        int(i64::from(a.priority)),
        opt_text(a.color.as_deref()),
    ]);
    v.extend([
        boolean(a.use_default_alerts),
        boolean(a.hide_attendees),
        boolean(a.may_invite_self),
        boolean(a.may_invite_others),
        opt_int(series_end(a)?),
        text(&format!(
            "{}\n{}",
            a.title.to_lowercase(),
            a.description.to_lowercase()
        )),
        Value::Null,
    ]);
    Ok(v)
}

pub fn participant_values(id: ObjectId, p: &Participant) -> Vec<Value> {
    let (kind, who, art, abzug) = match &p.who {
        Attendee::Actor(r) => {
            let [i, a, z] = person_values(r);
            ("actor", i, a, z)
        }
        Attendee::Resource(r) => (
            "resource",
            text(&fs_calendar_testdata::draft::BasicValue::canonical(r)),
            Value::Null,
            Value::Null,
        ),
        Attendee::Location(r) => (
            "location",
            text(&fs_calendar_testdata::draft::BasicValue::canonical(r)),
            Value::Null,
            Value::Null,
        ),
    };
    let r = p.roles;
    vec![
        id_value(id),
        text(&p.key),
        text(kind),
        who,
        art,
        abzug,
        boolean(r.owner),
        boolean(r.attendee),
        boolean(r.optional),
        boolean(r.informational),
        boolean(r.chair),
        boolean(r.contact),
        text(word(&PARTICIPATION, p.status)),
        boolean(p.expect_reply),
        opt_text(p.comment.as_deref()),
        opt_text(p.invited_by.as_deref()),
        int(i64::from(p.schedule_sequence)),
        opt_int(p.schedule_updated.map(|u| u.millis())),
    ]
}

pub fn alert_values(id: ObjectId, a: &Alert) -> Vec<Value> {
    let (kind, secs, rel, at) = match a.trigger {
        Trigger::Offset {
            seconds,
            relative_to,
        } => (
            "offset",
            int(seconds),
            text(word(&RELATIVE_TO, relative_to)),
            Value::Null,
        ),
        Trigger::Absolute { at } => ("absolute", Value::Null, Value::Null, int(at.millis())),
    };
    vec![
        id_value(id),
        text(&a.key),
        text(kind),
        secs,
        rel,
        at,
        text(word(&ALERT_ACTION, a.action)),
        opt_int(a.acknowledged.map(|u| u.millis())),
    ]
}
