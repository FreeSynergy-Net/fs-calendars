//! Small readers and writers for the JSCalendar JSON forms: durations, UTC times, local times, maps of flags.
//!
//! Pattern: Value Object helpers (pure functions on text and numbers).

use fs_calendars_testdata::draft::{BasicValue, LocalDateTime, Span, Utc};
use fs_calendars_testdata::zone::{from_utc_ms, naive_date, to_utc_ms};
use std::fmt::Write;

use serde_json::Value;

/// `P1DT2H30M`, `PT90S`, `P2W`: nominal days and exact seconds (RFC 8984 `Duration`); `None` when it is not one.
#[must_use]
pub fn parse_duration(t: &str) -> Option<Span> {
    let rest = t.strip_prefix('P')?;
    let (date, time) = rest.split_once('T').map_or((rest, ""), |(d, t)| (d, t));
    let (mut days, mut secs) = (0_u64, 0_u64);
    let mut num = String::new();
    for ch in date.chars() {
        match ch {
            '0'..='9' => num.push(ch),
            'W' => days += std::mem::take(&mut num).parse::<u64>().ok()? * 7,
            'D' => days += std::mem::take(&mut num).parse::<u64>().ok()?,
            _ => return None,
        }
    }
    for ch in time.chars() {
        let n = || num.parse::<u64>().ok();
        match ch {
            '0'..='9' => num.push(ch),
            'H' => secs += n()? * 3600,
            'M' => secs += n()? * 60,
            'S' => secs += n()?,
            _ => return None,
        }
        if !ch.is_ascii_digit() {
            num.clear();
        }
    }
    Some(Span {
        days: u32::try_from(days).ok()?,
        seconds: u32::try_from(secs).ok()?,
    })
}

/// The shortest RFC 8984 form of a span.
#[must_use]
pub fn format_duration(s: Span) -> String {
    let (h, m, sec) = (s.seconds / 3600, s.seconds % 3600 / 60, s.seconds % 60);
    let mut out = String::from("P");
    if s.days > 0 {
        let _ = write!(out, "{}D", s.days);
    }
    if s.seconds > 0 || s.days == 0 {
        out.push('T');
        for (n, u) in [(h, 'H'), (m, 'M'), (sec, 'S')] {
            if n > 0 {
                let _ = write!(out, "{n}{u}");
            }
        }
        if s.seconds == 0 {
            out.push_str("0S");
        }
    }
    out
}

/// A signed offset (`-PT30M`) in seconds.
#[must_use]
pub fn parse_offset(t: &str) -> Option<i64> {
    let (sign, rest) = t.strip_prefix('-').map_or((1, t), |r| (-1, r));
    let s = parse_duration(rest)?;
    Some(sign * (i64::from(s.days) * 86_400 + i64::from(s.seconds)))
}

/// `2026-11-25T11:00:00Z` as an instant.
#[must_use]
pub fn parse_utc(t: &str) -> Option<Utc> {
    let local = LocalDateTime::parse(t.strip_suffix('Z')?).ok()?;
    let n = fs_calendars_testdata::zone::naive(local)?;
    Some(to_utc_ms(n))
}

/// An instant as `2026-11-25T11:00:00Z`.
#[must_use]
pub fn format_utc(u: Utc) -> String {
    format!("{}Z", from_utc_ms(u).format("%Y-%m-%dT%H:%M:%S"))
}

/// The keys of a JSON map of `true` flags (`keywords`, `categories`, `roles`), in key order.
#[must_use]
pub fn flag_keys(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter(|(_, f)| f.as_bool() == Some(true))
                .map(|(k, _)| k.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// A calendar date `YYYY-MM-DD` of a local time.
#[must_use]
pub fn day_of(l: LocalDateTime) -> String {
    l.date().canonical()
}

/// Whether the date exists (kept for symmetry with the typed API).
#[must_use]
pub fn real_date(l: LocalDateTime) -> bool {
    naive_date(l.date()).is_some()
}
