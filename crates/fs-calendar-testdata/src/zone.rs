//! From wall-clock time in a zone to an instant (ADR 0085 time cases).
//!
//! Pattern: Strategy (a zone rule is either an IANA zone or a fixed offset of a custom zone).
//! Gap (spring): the offset BEFORE the gap applies; ambiguity (autumn): the FIRST occurrence.

use std::collections::HashMap;

use chrono::{
    Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, Offset, TimeZone, Timelike,
};
use chrono_tz::Tz;

use crate::draft::{Error, LocalDateTime, PlainDate, Problem, Result, Span, Utc, When, Zone};

/// Custom zones (`/...`, from a `VTIMEZONE`): name to fixed offset in seconds east of UTC.
pub type CustomZones = HashMap<String, i32>;

/// A resolved zone.
#[derive(Clone, Copy, Debug)]
pub enum ZoneRule {
    /// An IANA zone.
    Iana(Tz),
    /// A custom zone, reduced to one fixed offset (the fake does not interpret `VTIMEZONE` rules).
    Fixed(i32),
}

/// Find the rule of a zone; an unknown one is [`Error::UnknownZone`], never guessed.
///
/// # Errors
/// `UnknownZone` when the name is neither an IANA zone nor a registered custom zone.
pub fn resolve(zone: &Zone, customs: &CustomZones) -> Result<ZoneRule> {
    match zone {
        Zone::Iana(name) => name
            .parse::<Tz>()
            .map(ZoneRule::Iana)
            .map_err(|_| Error::UnknownZone(name.clone())),
        Zone::Custom(name) => customs
            .get(name)
            .map(|o| ZoneRule::Fixed(*o))
            .ok_or_else(|| Error::UnknownZone(name.clone())),
    }
}

impl ZoneRule {
    /// The wall-clock time as a naive UTC time.
    #[must_use]
    pub fn to_utc(self, local: NaiveDateTime) -> NaiveDateTime {
        let offset = match self {
            ZoneRule::Fixed(seconds) => seconds,
            ZoneRule::Iana(tz) => match tz.offset_from_local_datetime(&local) {
                LocalResult::Single(o) | LocalResult::Ambiguous(o, _) => o.fix().local_minus_utc(),
                LocalResult::None => {
                    match tz.offset_from_local_datetime(&(local - Duration::hours(24))) {
                        LocalResult::Single(o) | LocalResult::Ambiguous(o, _) => {
                            o.fix().local_minus_utc()
                        }
                        LocalResult::None => 0,
                    }
                }
            },
        };
        local - Duration::seconds(i64::from(offset))
    }
}

/// `PlainDate` to a calendar date, `None` if it does not exist.
#[must_use]
pub fn naive_date(d: PlainDate) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(d.year, u32::from(d.month), u32::from(d.day))
}

/// A calendar date as `PlainDate`.
#[must_use]
pub fn plain(d: NaiveDate) -> PlainDate {
    PlainDate {
        year: d.year(),
        month: u8::try_from(d.month()).unwrap_or(1),
        day: u8::try_from(d.day()).unwrap_or(1),
    }
}

/// `LocalDateTime` to a naive date-time, `None` if it does not exist.
#[must_use]
pub fn naive(l: LocalDateTime) -> Option<NaiveDateTime> {
    naive_date(l.date)?.and_hms_opt(u32::from(l.hour), u32::from(l.minute), u32::from(l.second))
}

/// A naive date-time as `LocalDateTime`.
#[must_use]
pub fn local(n: NaiveDateTime) -> LocalDateTime {
    LocalDateTime {
        date: plain(n.date()),
        hour: u8::try_from(n.hour()).unwrap_or(0),
        minute: u8::try_from(n.minute()).unwrap_or(0),
        second: u8::try_from(n.second()).unwrap_or(0),
    }
}

/// Milliseconds since the epoch of a naive UTC time.
#[must_use]
pub fn to_utc_ms(n: NaiveDateTime) -> Utc {
    Utc(n.and_utc().timestamp_millis())
}

/// A naive UTC time from milliseconds.
#[must_use]
pub fn from_utc_ms(u: Utc) -> NaiveDateTime {
    chrono::DateTime::from_timestamp_millis(u.0)
        .map_or(chrono::DateTime::UNIX_EPOCH.naive_utc(), |d| d.naive_utc())
}

fn invalid(field: &str, reason: &str) -> Error {
    Error::Invalid(vec![Problem {
        field: field.to_owned(),
        reason: reason.to_owned(),
    }])
}

/// The first and the last instant of one occurrence. A timed one: start in its zone, end = start plus the nominal
/// days (in the zone) plus the seconds (exact). An all-day one and a floating one have no zone of their own:
/// they are placed in the zone of the viewer `view` (ADR 0085), so their instants differ per viewer.
///
/// # Errors
/// `UnknownZone`, or `Invalid` for a date or time that does not exist.
pub fn bounds(when: &When, customs: &CustomZones, view: &Zone) -> Result<(Utc, Utc)> {
    match when {
        When::Timed {
            start,
            zone,
            length,
            ..
        } => placed(resolve(zone, customs)?, *start, *length),
        When::Floating { start, length } => placed(resolve(view, customs)?, *start, *length),
        When::AllDay { first, days } => {
            let rule = resolve(view, customs)?;
            let d = naive_date(*first).ok_or_else(|| invalid("when.first", "no such date"))?;
            let s = d.and_time(chrono::NaiveTime::MIN);
            Ok((
                to_utc_ms(rule.to_utc(s)),
                to_utc_ms(rule.to_utc(s + Duration::days(i64::from(*days)))),
            ))
        }
    }
}

fn placed(rule: ZoneRule, start: LocalDateTime, length: Span) -> Result<(Utc, Utc)> {
    let n = naive(start).ok_or_else(|| invalid("when.start", "no such date or time"))?;
    Ok((to_utc_ms(rule.to_utc(n)), end_of(rule, n, length)))
}

fn end_of(rule: ZoneRule, start: NaiveDateTime, length: Span) -> Utc {
    let end_local = start + Duration::days(i64::from(length.days));
    Utc(to_utc_ms(rule.to_utc(end_local)).0 + i64::from(length.seconds) * 1000)
}
