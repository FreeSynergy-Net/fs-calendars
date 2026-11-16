//! Writing an appointment: the row, the derived hull (R11) and the child rows, inside the caller's transaction (R3).
//!
//! Pattern: Transaction script. The row is written with an upsert, never `DELETE` + `INSERT` (draft R4); child
//! tables have no children of their own in this slice, so they are replaced.

use std::collections::{BTreeSet, HashMap};

use fs_calendar_testdata::check;
use fs_calendar_testdata::draft::{
    Alert, Appointment, Attendee, Error, ObjectId, Participant, PersonRef, Result, Trigger, When,
    Zone,
};
use fs_calendar_testdata::zone::{bounds, iana};
use turso::Value;

use crate::calendar_tables::{ALERT, APPOINTMENT, CATEGORY, COLLECTION, KEYWORD, PARTICIPANT};
use crate::codec::{
    boolean, date_to_int, id_value, int, local_to_int, opt_int, opt_text, person_values, text,
};
use crate::fields::unsupported;
use crate::store::Conn;
use crate::words::{
    ALERT_ACTION, FREE_BUSY, PARTICIPATION, PRIVACY, RELATIVE_TO, STATUS, TEXT_TYPE, word,
};

/// Largest zone offset the hull allows for (draft section 2.3).
const HULL_PAD_MS: i64 = 16 * 3_600_000;

/// Check everything before anything is written: the pilot's checks, then what this slice cannot store.
///
/// # Errors
/// `Invalid`/`UnknownZone` from the checks; `Unsupported` for a part that is not stored yet (never dropped quietly).
pub fn validate(a: &Appointment, capability: &str) -> Result<()> {
    check::appointment(a, &HashMap::new())?;
    match unsupported(a) {
        Some(part) => Err(Error::Unsupported {
            capability: capability.to_owned(),
            reason: format!("{part} is not stored yet (core step 1b)"),
        }),
        None => Ok(()),
    }
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

fn when_values(w: &When) -> Result<Vec<Value>> {
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

fn appointment_values(a: &Appointment) -> Result<Vec<Value>> {
    let (min, max) = hull(&a.when)?;
    let mut v = vec![
        id_value(a.id),
        id_value(a.calendar),
        text(&a.uid),
        Value::Null,
        Value::Null,
        boolean(false),
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
    ]);
    Ok(v)
}

fn participant_values(id: ObjectId, p: &Participant) -> Vec<Value> {
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

fn alert_values(id: ObjectId, a: &Alert) -> Vec<Value> {
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

/// Write the appointment row and replace its child rows. The envelope row must exist already.
///
/// # Errors
/// `Invalid` naming `table.column`, `UnknownZone`, or the engine's error.
pub async fn write_appointment(c: &Conn<'_>, a: &Appointment) -> Result<()> {
    c.upsert(&APPOINTMENT, &["id"], appointment_values(a)?)
        .await?;
    let id = id_value(a.id);
    for t in [
        "calendar_participant",
        "calendar_alert",
        "calendar_keyword",
        "calendar_category",
    ] {
        c.exec(
            &format!("DELETE FROM {t} WHERE appointment = ?"),
            vec![id.clone()],
        )
        .await?;
    }
    for p in &a.participants {
        c.insert(&PARTICIPANT, participant_values(a.id, p)).await?;
    }
    for x in &a.alerts {
        c.insert(&ALERT, alert_values(a.id, x)).await?;
    }
    for (t, list) in [(&KEYWORD, &a.keywords), (&CATEGORY, &a.categories)] {
        for v in list.iter().collect::<BTreeSet<_>>() {
            c.insert(t, vec![id.clone(), text(v)]).await?;
        }
    }
    Ok(())
}

/// Insert the row of a calendar (the envelope row must exist).
///
/// # Errors
/// `Invalid` naming `table.column`, or the engine's error.
pub async fn write_collection(
    c: &Conn<'_>,
    id: ObjectId,
    name: &str,
    color: Option<&str>,
    zone: Option<&Zone>,
    owner: &PersonRef,
) -> Result<()> {
    let [oid, oart, oabzug] = person_values(owner);
    c.insert(
        &COLLECTION,
        vec![
            id_value(id),
            text(name),
            opt_text(color),
            opt_text(zone.map(Zone::id)),
            oid,
            oart,
            oabzug,
        ],
    )
    .await
}
