//! The border for the values of the log: the trait's `Value` and `FieldChange` as the layer's `LogValue` and
//! `FieldChange` (the types that go into the typed log columns), and back.
//!
//! Pattern: Adapter (one function per type and direction; the time and zone types go through their canonical text).

use fs_calendar_testdata::draft as si;
use fs_calendar_testdata::draft::BasicValue as _;
use fs_objects::v5::log::FieldChange as LayerChange;
use fs_objects::v5::log_codec::LogValue;
use fs_types as ft;
use ft::BasicValue as _;

use crate::bridge::{oid, oid_back, person, person_back, reference, reference_back};

fn bad(why: &str) -> si::Error {
    si::Error::Invalid(vec![si::Problem {
        field: "changes".to_owned(),
        reason: why.to_owned(),
    }])
}

fn date(v: si::Date) -> si::Result<ft::Date> {
    ft::Date::parse(&v.canonical()).map_err(|_| bad("date"))
}

fn date_back(v: ft::Date) -> si::Result<si::Date> {
    si::Date::parse(&v.canonical()).map_err(|_| bad("date"))
}

/// A wall-clock time of the trait as the layer's.
///
/// # Errors
/// `Invalid` for a value `fs-types` refuses.
pub fn local(v: si::LocalDateTime) -> si::Result<ft::LocalDateTime> {
    ft::LocalDateTime::new(date(v.date())?, v.hour(), v.minute(), v.second())
        .map_err(|_| bad("time"))
}

fn local_back(v: ft::LocalDateTime) -> si::Result<si::LocalDateTime> {
    si::LocalDateTime::new(date_back(v.date())?, v.hour(), v.minute(), v.second())
        .map_err(|_| bad("time"))
}

fn zone(v: &si::Zone) -> si::Result<ft::Zone> {
    match v.kind() {
        si::ZoneKind::Iana => ft::Zone::iana(v.id()),
        si::ZoneKind::Custom => ft::Zone::custom(v.id()),
    }
    .map_err(|_| bad("zone"))
}

fn zone_back(v: &ft::Zone) -> si::Result<si::Zone> {
    match v.kind() {
        ft::ZoneKind::Iana => si::Zone::iana(v.id()),
        ft::ZoneKind::Custom => si::Zone::custom(v.id()),
    }
    .map_err(|_| bad("zone"))
}

fn attendee(v: &si::Attendee) -> si::Result<ft::Attendee> {
    Ok(match v {
        si::Attendee::Actor(p) => ft::Attendee::Actor(person(p)?),
        si::Attendee::Resource(r) => ft::Attendee::Resource(reference(r)?),
        si::Attendee::Location(r) => ft::Attendee::Location(reference(r)?),
    })
}

fn attendee_back(v: &ft::Attendee) -> si::Result<si::Attendee> {
    Ok(match v {
        ft::Attendee::Actor(p) => si::Attendee::Actor(person_back(p)?),
        ft::Attendee::Resource(r) => si::Attendee::Resource(reference_back(r)?),
        ft::Attendee::Location(r) => si::Attendee::Location(reference_back(r)?),
    })
}

/// A value of the trait's log as the layer's.
///
/// # Errors
/// `Invalid` for a value `fs-types` refuses.
pub fn log_value(v: &si::Value) -> si::Result<LogValue> {
    Ok(match v {
        si::Value::Int(i) => LogValue::Int(*i),
        si::Value::Real(f) => LogValue::Real(*f),
        si::Value::Text(t) => LogValue::Text(t.clone()),
        si::Value::Bool(b) => LogValue::Bool(*b),
        si::Value::Utc(u) => LogValue::Utc(ft::Utc::from_millis(u.millis())),
        si::Value::Local(l) => LogValue::Local(local(*l)?),
        si::Value::Date(d) => LogValue::Date(date(*d)?),
        si::Value::Zone(z) => LogValue::Zone(zone(z)?),
        si::Value::Object(o) => LogValue::Object(oid(*o)),
        si::Value::Reference(r) => LogValue::Reference(reference(r)?),
        si::Value::Person(p) => LogValue::Person(person(p)?),
        si::Value::Attendee(a) => LogValue::Attendee(attendee(a)?),
    })
}

/// A value of the layer's log as the trait's.
///
/// # Errors
/// `Invalid` for a value the stand-in refuses.
pub fn log_value_back(v: &LogValue) -> si::Result<si::Value> {
    Ok(match v {
        LogValue::Int(i) => si::Value::Int(*i),
        LogValue::Real(f) => si::Value::Real(*f),
        LogValue::Text(t) => si::Value::Text(t.clone()),
        LogValue::Bool(b) => si::Value::Bool(*b),
        LogValue::Utc(u) => si::Value::Utc(si::Utc::from_millis(u.millis())),
        LogValue::Local(l) => si::Value::Local(local_back(*l)?),
        LogValue::Date(d) => si::Value::Date(date_back(*d)?),
        LogValue::Zone(z) => si::Value::Zone(zone_back(z)?),
        LogValue::Object(o) => si::Value::Object(oid_back(*o)),
        LogValue::Reference(r) => si::Value::Reference(reference_back(r)?),
        LogValue::Person(p) => si::Value::Person(person_back(p)?),
        LogValue::Attendee(a) => si::Value::Attendee(attendee_back(a)?),
    })
}

/// A change of the trait as the layer's.
///
/// # Errors
/// `Invalid` for a value `fs-types` refuses.
pub fn change(c: &si::FieldChange) -> si::Result<LayerChange> {
    Ok(LayerChange {
        field: c.field.clone(),
        row_key: c.row_key.clone(),
        old: c.old.as_ref().map(log_value).transpose()?,
        new: c.new.as_ref().map(log_value).transpose()?,
    })
}

/// A change of the layer as the trait's.
///
/// # Errors
/// `Invalid` for a value the stand-in refuses.
pub fn change_back(c: &LayerChange) -> si::Result<si::FieldChange> {
    Ok(si::FieldChange {
        field: c.field.clone(),
        row_key: c.row_key.clone(),
        old: c.old.as_ref().map(log_value_back).transpose()?,
        new: c.new.as_ref().map(log_value_back).transpose()?,
    })
}
