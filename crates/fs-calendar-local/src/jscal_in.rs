//! A JSCalendar `Event` (JSON) read into the typed values of the trait: the normalization at the border (ADR 0085).
//!
//! Pattern: Adapter. Nothing is lost quietly: what this program does not store (yet) or cannot interpret goes into
//! `Read::losses`, and the received file stays as the original. Time: a time without a zone becomes `Timed` in the
//! assumed zone with `zone_assumed`, unless the file says "floating" (`X-FS-FLOATING`); all-day is a date, never moved.

use std::collections::HashMap;

use fs_calendar_testdata::draft::{
    Alert, AlertAction, AppointmentPatch, BasicValue, ImportContext, LocalDateTime, Loss,
    ParticipationStatus, RecurrenceRule, Result, Roles, Span, TextType, Trigger, When, Zone,
};
use fs_calendar_testdata::recur::check;
use fs_calendar_testdata::zone::{iana, resolve};
use serde_json::Value;

use crate::codec::invalid;
use crate::jscal_rule::rule_from_json;
use crate::jscal_util::{flag_keys, parse_duration, parse_offset, parse_utc};
use crate::words::{
    ALERT_ACTION, FREE_BUSY, PARTICIPATION, PRIVACY, RELATIVE_TO, STATUS, from_word,
};

/// A participant as the file names it, before the person directory has answered.
#[derive(Clone, Debug)]
pub struct RawPerson {
    pub key: String,
    pub name: Option<String>,
    pub address: Option<String>,
    pub kind: String,
    pub roles: Roles,
    pub status: ParticipationStatus,
    pub expect_reply: bool,
}

/// What the file says about one occurrence.
#[derive(Clone, Debug)]
pub enum RawChange {
    Excluded,
    Changed(AppointmentPatch),
}

/// An event read from JSON.
#[derive(Clone, Debug)]
pub struct Read {
    pub uid: String,
    pub title: String,
    pub when: When,
    pub rest: AppointmentPatch,
    pub people: Vec<RawPerson>,
    pub overrides: Vec<(LocalDateTime, RawChange)>,
    /// The rule as written, when it is outside what the engine expands (kept, shown as the first occurrence only).
    pub unread_rrule: Option<String>,
    pub losses: Vec<Loss>,
    pub zone_assumed: bool,
    pub prod_id: Option<String>,
}

const IGNORED: [&str; 12] = [
    "@type",
    "version",
    "method",
    "iCalendar",
    "updated",
    "created",
    "sequence",
    "uid",
    "prodId",
    "organizerCalendarAddress",
    "freesynergy.net:floating",
    "timeZones",
];
const NOT_STORED: [&str; 6] = [
    "locations",
    "virtualLocations",
    "links",
    "relatedTo",
    "localizations",
    "replyTo",
];

fn text<'a>(o: &'a Value, k: &str) -> Option<&'a str> {
    o.get(k).and_then(Value::as_str)
}

fn pick<T: Copy>(t: &[(T, &'static str)], o: &Value, k: &str) -> Option<T> {
    text(o, k).and_then(|w| from_word(t, w).ok())
}

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

fn when_of(
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

fn alerts_of(v: &Value, uid: &str, losses: &mut Vec<Loss>) -> Vec<Alert> {
    let Some(m) = v.get("alerts").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (key, a) in m {
        let trig = a.get("trigger");
        let trigger = match trig.and_then(|t| text(t, "@type")) {
            Some("AbsoluteTrigger") => trig
                .and_then(|t| text(t, "when"))
                .and_then(parse_utc)
                .map(|at| Trigger::Absolute { at }),
            _ => trig
                .and_then(|t| text(t, "offset"))
                .and_then(parse_offset)
                .map(|seconds| Trigger::Offset {
                    seconds,
                    relative_to: trig
                        .and_then(|t| pick(&RELATIVE_TO, t, "relativeTo"))
                        .unwrap_or(fs_calendar_testdata::draft::RelativeTo::Start),
                }),
        };
        match trigger {
            Some(trigger) => out.push(Alert {
                key: key.clone(),
                trigger,
                action: pick(&ALERT_ACTION, a, "action").unwrap_or(AlertAction::Display),
                acknowledged: text(a, "acknowledged").and_then(parse_utc),
            }),
            None => losses.push(Loss {
                uid: Some(uid.to_owned()),
                field: format!("alerts/{key}"),
                reason: "the trigger cannot be read".into(),
            }),
        }
    }
    out
}

fn people_of(v: &Value, uid: &str, losses: &mut Vec<Loss>) -> Vec<RawPerson> {
    let Some(m) = v.get("participants").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (key, p) in m {
        let address = text(p, "calendarAddress")
            .map(str::to_owned)
            .or_else(|| text(p, "email").map(|e| format!("mailto:{e}")))
            .or_else(|| {
                p.pointer("/sendTo/imip")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
        let roles = flag_keys(p.get("roles"));
        let has = |r: &str| roles.iter().any(|x| x == r);
        out.push(RawPerson {
            key: key.clone(),
            name: text(p, "name").map(str::to_owned),
            address,
            kind: text(p, "kind").unwrap_or("individual").to_owned(),
            roles: Roles {
                owner: has("owner"),
                attendee: has("attendee") || has("required"),
                optional: has("optional"),
                informational: has("informational"),
                chair: has("chair"),
                contact: has("contact"),
            },
            status: pick(&PARTICIPATION, p, "participationStatus")
                .unwrap_or(ParticipationStatus::NeedsAction),
            expect_reply: p
                .get("expectReply")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        });
    }
    let _ = (uid, losses);
    out
}

fn patch_of(o: &Value, base: &When, uid: &str, losses: &mut Vec<Loss>) -> AppointmentPatch {
    let mut p = AppointmentPatch {
        title: text(o, "title").map(str::to_owned),
        status: pick(&STATUS, o, "status"),
        free_busy: pick(&FREE_BUSY, o, "freeBusyStatus"),
        privacy: pick(&PRIVACY, o, "privacy"),
        priority: o
            .get("priority")
            .and_then(Value::as_u64)
            .and_then(|n| u8::try_from(n).ok()),
        ..AppointmentPatch::default()
    };
    if let Some(d) = text(o, "description") {
        p.description = Some((d.to_owned(), TextType::Plain));
    }
    if let Some(s) = text(o, "start") {
        let mut w = base.clone();
        if let (Ok(l), When::Timed { start, .. } | When::Floating { start, .. }) =
            (LocalDateTime::parse(s), &mut w)
        {
            *start = l;
            p.when = Some(w);
        }
    }
    for k in o.as_object().into_iter().flat_map(|m| m.keys()) {
        if ![
            "title",
            "description",
            "start",
            "status",
            "freeBusyStatus",
            "privacy",
            "priority",
            "excluded",
        ]
        .contains(&k.as_str())
        {
            losses.push(Loss {
                uid: Some(uid.to_owned()),
                field: format!("recurrenceOverrides/{k}"),
                reason: "this change of one occurrence is not read".into(),
            });
        }
    }
    p
}

/// Read one event.
///
/// # Errors
/// `Invalid` for a missing `uid` or `start`, or a start or duration that is not one.
pub fn read_event(v: &Value, ctx: &ImportContext) -> Result<Read> {
    let uid = text(v, "uid")
        .ok_or_else(|| invalid("uid", "missing"))?
        .to_owned();
    let mut losses = Vec::new();
    let (when, zone_assumed) = when_of(v, &uid, ctx, &mut losses)?;
    let mut rest = AppointmentPatch {
        status: pick(&STATUS, v, "status"),
        free_busy: pick(&FREE_BUSY, v, "freeBusyStatus"),
        privacy: pick(&PRIVACY, v, "privacy"),
        priority: v
            .get("priority")
            .and_then(Value::as_u64)
            .and_then(|n| u8::try_from(n).ok()),
        ..AppointmentPatch::default()
    };
    if let Some(d) = text(v, "description") {
        let html = text(v, "descriptionContentType") == Some("text/html");
        rest.description = Some((
            d.to_owned(),
            if html {
                TextType::Html
            } else {
                TextType::Plain
            },
        ));
    }
    rest.locale = text(v, "locale").map(|l| Some(l.to_owned()));
    rest.color = text(v, "color").map(|c| Some(c.to_owned()));
    rest.keywords = Some(flag_keys(v.get("keywords")));
    rest.categories = Some(flag_keys(v.get("categories")));
    rest.use_default_alerts = v.get("useDefaultAlerts").and_then(Value::as_bool);
    rest.alerts = Some(alerts_of(v, &uid, &mut losses));
    let mut rules: Vec<&Value> = v.get("recurrenceRule").into_iter().collect();
    rules.extend(
        v.get("recurrenceRules")
            .and_then(Value::as_array)
            .into_iter()
            .flatten(),
    );
    let (mut read, mut unread): (Vec<RecurrenceRule>, Option<String>) = (Vec::new(), None);
    for r in rules {
        match rule_from_json(r).filter(|x| check(x).is_empty()) {
            Some(x) => read.push(x),
            None => unread = Some(r.to_string()),
        }
    }
    if unread.is_some() {
        losses.push(Loss { uid: Some(uid.clone()), field: "recurrence".into(), reason: "Serie nur teilweise darstellbar: the rule is outside what is expanded; kept as written, only the first occurrence is shown".into() });
        read.clear();
    }
    rest.recurrence = Some(read);
    if v.get("excludedRecurrenceRules").is_some() {
        losses.push(Loss {
            uid: Some(uid.clone()),
            field: "excludedRecurrenceRules".into(),
            reason: "not part of JSCalendar 2.0; kept only in the original".into(),
        });
    }
    let mut overrides = Vec::new();
    for (k, o) in v
        .get("recurrenceOverrides")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        let Ok(rid) = LocalDateTime::parse(k) else {
            continue;
        };
        let change = if o.get("excluded").and_then(Value::as_bool) == Some(true) {
            RawChange::Excluded
        } else {
            RawChange::Changed(patch_of(o, &when, &uid, &mut losses))
        };
        if unread.is_none() {
            overrides.push((rid, change));
        }
    }
    for k in NOT_STORED {
        if v.get(k).is_some_and(|x| !x.is_null()) {
            losses.push(Loss {
                uid: Some(uid.clone()),
                field: k.to_owned(),
                reason: "not stored yet; kept only in the original".into(),
            });
        }
    }
    let x: Vec<String> = v
        .pointer("/iCalendar/properties")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| p.get(0).and_then(Value::as_str))
        .filter(|n| n.starts_with("x-") && *n != "x-fs-floating")
        .map(str::to_owned)
        .collect();
    if !x.is_empty() {
        losses.push(Loss {
            uid: Some(uid.clone()),
            field: "x-properties".into(),
            reason: format!("{}: kept only in the original", x.join(", ")),
        });
    }
    for k in v.as_object().into_iter().flat_map(|m| m.keys()) {
        let handled = IGNORED.contains(&k.as_str())
            || NOT_STORED.contains(&k.as_str())
            || [
                "title",
                "description",
                "descriptionContentType",
                "locale",
                "color",
                "keywords",
                "categories",
                "useDefaultAlerts",
                "alerts",
                "participants",
                "recurrenceRule",
                "recurrenceRules",
                "excludedRecurrenceRules",
                "recurrenceOverrides",
                "start",
                "duration",
                "timeZone",
                "endTimeZone",
                "showWithoutTime",
                "status",
                "freeBusyStatus",
                "privacy",
                "priority",
            ]
            .contains(&k.as_str());
        if !handled {
            losses.push(Loss {
                uid: Some(uid.clone()),
                field: k.clone(),
                reason: "not read; kept only in the original".into(),
            });
        }
    }
    let people = people_of(v, &uid, &mut losses);
    let title = text(v, "title").unwrap_or_default().to_owned();
    Ok(Read {
        uid,
        title,
        when,
        rest,
        people,
        overrides,
        unread_rrule: unread,
        losses,
        zone_assumed,
        prod_id: text(v, "prodId").map(str::to_owned),
    })
}
