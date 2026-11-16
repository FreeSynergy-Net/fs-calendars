//! The time of an event (JSON to `When`): zone, floating, all-day (ADR 0085).
//!
//! Pattern: Adapter. A time without a zone is `Timed` in the assumed zone with `zone_assumed`, unless the file says
//! floating (`X-FS-FLOATING`); all-day is a date and never moved; an unknown or custom zone is replaced and reported.

use std::collections::HashMap;

use fs_calendars_testdata::draft::{ImportContext, LocalDateTime, Loss, Result, Span, When, Zone};
use fs_calendars_testdata::zone::{iana, resolve};
use serde_json::Value;

use crate::codec::invalid;
use crate::jscal_in::text;
use crate::jscal_util::parse_duration;

fn is_floating(v: &Value) -> bool {
    v.get("freesynergy.net:floating").and_then(Value::as_bool) == Some(true)
        || v.pointer("/iCalendar/properties")
            .and_then(Value::as_array)
            .is_some_and(|ps| {
                ps.iter().any(|p| {
                    p.get(0).and_then(Value::as_str) == Some("x-fs-floating")
                        && p.get(3)
                            .and_then(Value::as_str)
                            .is_some_and(|t| t.eq_ignore_ascii_case("true"))
                })
            })
}

fn zone_of_json(
    name: &str,
    uid: &str,
    ctx: &ImportContext,
    losses: &mut Vec<Loss>,
) -> (Zone, bool) {
    let z = iana(name);
    if !name.starts_with('/') && resolve(&z, &HashMap::new()).is_ok() {
        return (z, false);
    }
    losses.push(Loss {
        uid: Some(uid.to_owned()),
        field: "timeZone".into(),
        reason: format!("zone {name} is unknown or custom; replaced by the assumed zone"),
    });
    (ctx.assume_zone.clone(), true)
}

pub fn when_of(
    v: &Value,
    uid: &str,
    ctx: &ImportContext,
    losses: &mut Vec<Loss>,
) -> Result<(When, bool)> {
    let start = text(v, "start").ok_or_else(|| invalid("start", "missing"))?;
    let start =
        LocalDateTime::parse(start).map_err(|_| invalid("start", "not a local date-time"))?;
    let length = text(v, "duration")
        .map_or(
            Some(Span {
                days: 0,
                seconds: 0,
            }),
            parse_duration,
        )
        .ok_or_else(|| invalid("duration", "not a duration"))?;
    if v.get("showWithoutTime").and_then(Value::as_bool) == Some(true) {
        if v.get("timeZone").is_some_and(|z| !z.is_null()) {
            losses.push(Loss {
                uid: Some(uid.to_owned()),
                field: "timeZone".into(),
                reason: "an all-day appointment has no zone; the zone is dropped".into(),
            });
        }
        if length.seconds != 0 {
            losses.push(Loss {
                uid: Some(uid.to_owned()),
                field: "duration".into(),
                reason: "an all-day appointment lasts whole days; the seconds are dropped".into(),
            });
        }
        return Ok((
            When::AllDay {
                first: start.date(),
                days: length.days.max(1),
            },
            false,
        ));
    }
    match text(v, "timeZone") {
        Some(z) => {
            let (zone, assumed) = zone_of_json(z, uid, ctx, losses);
            let end_zone = text(v, "endTimeZone").map(|e| zone_of_json(e, uid, ctx, losses).0);
            Ok((
                When::Timed {
                    start,
                    zone,
                    length,
                    end_zone,
                    zone_assumed: assumed,
                },
                assumed,
            ))
        }
        None if is_floating(v) => Ok((When::Floating { start, length }, false)),
        None => Ok((
            When::Timed {
                start,
                zone: ctx.assume_zone.clone(),
                length,
                end_zone: None,
                zone_assumed: true,
            },
            true,
        )),
    }
}
