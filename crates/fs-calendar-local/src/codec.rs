//! Between the trait's types and the stored values: ids, times, enum words, rows.
//!
//! Pattern: Adapter (typed values to engine values and back). A stored value that cannot be read back is
//! `Unavailable("damaged row ...")`, never a guess.

use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime};
use fs_calendar_testdata::draft::{
    BasicValue, Date, Error, LocalDateTime, ObjectId, PersonArt, PersonRef, Problem, Reference,
    Result, Utc, Zone,
};
use fs_calendar_testdata::zone::{local, naive, naive_date, plain};
use fs_objects::v5::codec as layer;
use turso::Value;

use crate::bridge::adapt;

// What does not touch a trait type is the layer's own code (one place): rows and engine values.
pub use layer::{Row, boolean, int, opt_int, opt_text, text};

#[must_use]
pub fn invalid(field: &str, reason: &str) -> Error {
    Error::Invalid(vec![Problem {
        field: field.to_owned(),
        reason: reason.to_owned(),
    }])
}

#[must_use]
pub fn damaged(what: &str) -> Error {
    Error::Unavailable(format!("damaged row: {what}"))
}

/// The UUID text of an object id (`8-4-4-4-12`, lower case).
#[must_use]
pub fn id_text(id: ObjectId) -> String {
    let h: String =
        id.0.iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .concat();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// The object id of UUID text; `None` when it is not one.
#[must_use]
pub fn parse_id(text: &str) -> Option<ObjectId> {
    let h: String = text.chars().filter(|c| *c != '-').collect();
    if text.len() != 36 || h.len() != 32 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0_u8; 16];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&h[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(ObjectId(out))
}

#[must_use]
pub fn id_value(id: ObjectId) -> Value {
    Value::Text(id_text(id))
}

/// Reading a column of a row (the layer's readers; a damaged row is the same `Unavailable`).
pub fn get_int(row: &Row, i: usize) -> Result<i64> {
    layer::get_int(row, i).map_err(adapt)
}
pub fn get_text(row: &Row, i: usize) -> Result<String> {
    layer::get_text(row, i).map_err(adapt)
}
pub fn get_opt_text(row: &Row, i: usize) -> Result<Option<String>> {
    layer::get_opt_text(row, i).map_err(adapt)
}
pub fn get_opt_int(row: &Row, i: usize) -> Result<Option<i64>> {
    layer::get_opt_int(row, i).map_err(adapt)
}
pub fn get_bool(row: &Row, i: usize) -> Result<bool> {
    layer::get_bool(row, i).map_err(adapt)
}
pub fn get_id(row: &Row, i: usize) -> Result<ObjectId> {
    parse_id(&get_text(row, i)?).ok_or_else(|| damaged("not an object id"))
}

/// `LOCAL`: seconds since 1970-01-01T00:00:00, read as wall clock.
pub fn local_to_int(l: LocalDateTime) -> Result<i64> {
    naive(l)
        .map(|n| n.and_utc().timestamp())
        .ok_or_else(|| invalid("time", "no such date or time"))
}
pub fn int_to_local(v: i64) -> Result<LocalDateTime> {
    DateTime::from_timestamp(v, 0)
        .map(|d| local(d.naive_utc()))
        .ok_or_else(|| damaged("local time out of range"))
}
/// `DATE`: days since 1970-01-01.
pub fn date_to_int(d: Date) -> Result<i64> {
    naive_date(d)
        .map(|n| i64::from(n.num_days_from_ce()) - EPOCH_DAYS)
        .ok_or_else(|| invalid("date", "no such date"))
}
pub fn int_to_date(v: i64) -> Result<Date> {
    let days = v
        .checked_add(EPOCH_DAYS)
        .and_then(|d| i32::try_from(d).ok())
        .ok_or_else(|| damaged("date out of range"))?;
    NaiveDate::from_num_days_from_ce_opt(days)
        .map(plain)
        .ok_or_else(|| damaged("date out of range"))
}
const EPOCH_DAYS: i64 = 719_163;

/// The wall clock as seconds (as-if-UTC) of a naive time.
#[must_use]
pub fn naive_secs(n: NaiveDateTime) -> i64 {
    n.and_utc().timestamp()
}

pub fn zone_of(text: &str) -> Result<Zone> {
    if text.starts_with('/') {
        Zone::custom(text)
    } else {
        Zone::iana(text)
    }
    .map_err(|_| damaged("zone"))
}
pub fn reference_of(text: &str) -> Result<Reference> {
    Reference::parse(text).map_err(|_| damaged("reference"))
}
pub fn utc_of(v: i64) -> Utc {
    Utc::from_millis(v)
}

const ARTS: [(PersonArt, &str); 4] = [
    (PersonArt::Profile, "profile"),
    (PersonArt::Person, "person"),
    (PersonArt::Group, "group"),
    (PersonArt::Organization, "organization"),
];
#[must_use]
pub fn art_word(a: PersonArt) -> &'static str {
    ARTS.iter()
        .find(|(x, _)| *x == a)
        .map_or("person", |(_, w)| w)
}
pub fn art_of(word: &str) -> Result<PersonArt> {
    ARTS.iter()
        .find(|(_, w)| *w == word)
        .map(|(x, _)| *x)
        .ok_or_else(|| damaged("person art"))
}
/// The three columns of a `PERSON` (id, art, abzug).
#[must_use]
pub fn person_values(p: &PersonRef) -> [Value; 3] {
    [
        text(&p.id.canonical()),
        text(art_word(p.art)),
        p.abzug.map_or(Value::Null, id_value),
    ]
}
pub fn person_of(row: &Row, at: usize) -> Result<PersonRef> {
    let abzug = match row.get(at + 2) {
        Some(Value::Null) | None => None,
        _ => Some(get_id(row, at + 2)?),
    };
    Ok(PersonRef {
        id: reference_of(&get_text(row, at)?)?,
        art: art_of(&get_text(row, at + 1)?)?,
        abzug,
    })
}
