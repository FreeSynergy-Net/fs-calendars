//! The border to the shared layer (`fs-objects` v5): the error, the kind of an activity and the values of the log.
//!
//! Pattern: Adapter (one function per type and direction; no state). Since the type swap the ids, revisions,
//! persons, references, zones and times are the same `fs-types` types on both sides and need no conversion; what is
//! left are the enums the trait and the layer each define (`Error`, `HistoryKind`, `Value`). The layer is
//! authoritative: the error mapping follows it, the calendar adapts.

use fs_calendars_testdata::draft as si;
use fs_objects::v5;
use fs_objects::v5::log::{FieldChange as LayerChange, Kind};
use fs_objects::v5::log_codec::LogValue;

/// The error of the layer as the trait's: what the trait has no word for is `Unavailable`.
#[must_use]
pub fn adapt(e: v5::Error) -> si::Error {
    match e {
        v5::Error::NotFound(id) => si::Error::NotFound(id),
        v5::Error::Invalid(ps) => si::Error::Invalid(ps),
        v5::Error::Changed { current } => si::Error::Changed { current },
        other => si::Error::Unavailable(other.to_string()),
    }
}

/// The error of the trait as the layer's (for a hook that runs inside the layer's period run).
#[must_use]
pub fn to_v5(e: si::Error) -> v5::Error {
    match e {
        si::Error::NotFound(id) => v5::Error::NotFound(id),
        si::Error::Invalid(ps) => v5::Error::Invalid(ps),
        si::Error::Changed { current } => v5::Error::Changed { current },
        other => v5::Error::Unavailable(format!("{other:?}")),
    }
}

/// The kind of an activity of the trait as the layer's.
#[must_use]
pub fn kind(k: si::HistoryKind) -> Kind {
    match k {
        si::HistoryKind::Create => Kind::Create,
        si::HistoryKind::Delete => Kind::Delete,
        si::HistoryKind::Undo => Kind::Undo,
        si::HistoryKind::Invite => Kind::Invite,
        si::HistoryKind::Accept => Kind::Accept,
        si::HistoryKind::TentativeAccept => Kind::TentativeAccept,
        si::HistoryKind::Reject => Kind::Reject,
        // Update, and a kind this version does not know (`HistoryKind` is `non_exhaustive`): logged as an update.
        _ => Kind::Update,
    }
}

/// The kind of an activity of the layer as the trait's.
#[must_use]
pub fn kind_back(k: Kind) -> si::HistoryKind {
    match k {
        Kind::Create => si::HistoryKind::Create,
        Kind::Update => si::HistoryKind::Update,
        Kind::Delete => si::HistoryKind::Delete,
        Kind::Undo => si::HistoryKind::Undo,
        Kind::Invite => si::HistoryKind::Invite,
        Kind::Accept => si::HistoryKind::Accept,
        Kind::TentativeAccept => si::HistoryKind::TentativeAccept,
        Kind::Reject => si::HistoryKind::Reject,
    }
}

/// A value of the trait's log as the layer's.
#[must_use]
pub fn log_value(v: &si::Value) -> LogValue {
    match v {
        si::Value::Int(i) => LogValue::Int(*i),
        si::Value::Real(f) => LogValue::Real(*f),
        si::Value::Text(t) => LogValue::Text(t.clone()),
        si::Value::Bool(b) => LogValue::Bool(*b),
        si::Value::Utc(u) => LogValue::Utc(*u),
        si::Value::Local(l) => LogValue::Local(*l),
        si::Value::Date(d) => LogValue::Date(*d),
        si::Value::Zone(z) => LogValue::Zone(z.clone()),
        si::Value::Object(o) => LogValue::Object(*o),
        si::Value::Reference(r) => LogValue::Reference(r.clone()),
        si::Value::Person(p) => LogValue::Person(p.clone()),
        si::Value::Attendee(a) => LogValue::Attendee(a.clone()),
        // `Value` is `non_exhaustive`: a value this version does not know is its debug text, never lost.
        other => LogValue::Text(format!("{other:?}")),
    }
}

/// A value of the layer's log as the trait's.
#[must_use]
pub fn log_value_back(v: &LogValue) -> si::Value {
    match v {
        LogValue::Int(i) => si::Value::Int(*i),
        LogValue::Real(f) => si::Value::Real(*f),
        LogValue::Text(t) => si::Value::Text(t.clone()),
        LogValue::Bool(b) => si::Value::Bool(*b),
        LogValue::Utc(u) => si::Value::Utc(*u),
        LogValue::Local(l) => si::Value::Local(*l),
        LogValue::Date(d) => si::Value::Date(*d),
        LogValue::Zone(z) => si::Value::Zone(z.clone()),
        LogValue::Object(o) => si::Value::Object(*o),
        LogValue::Reference(r) => si::Value::Reference(r.clone()),
        LogValue::Person(p) => si::Value::Person(p.clone()),
        LogValue::Attendee(a) => si::Value::Attendee(a.clone()),
    }
}

/// A change of the trait as the layer's.
#[must_use]
pub fn change(c: &si::FieldChange) -> LayerChange {
    LayerChange {
        field: c.field.clone(),
        row_key: c.row_key.clone(),
        old: c.old.as_ref().map(log_value),
        new: c.new.as_ref().map(log_value),
    }
}

/// A change of the layer as the trait's.
#[must_use]
pub fn change_back(c: &LayerChange) -> si::FieldChange {
    si::FieldChange {
        field: c.field.clone(),
        row_key: c.row_key.clone(),
        old: c.old.as_ref().map(log_value_back),
        new: c.new.as_ref().map(log_value_back),
    }
}
