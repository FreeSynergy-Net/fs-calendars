//! The parts of an event that have a map of their own in JSON: alerts, participants, the change of one occurrence.
//!
//! Pattern: Adapter (JSON objects to typed values); what cannot be read is a loss, never dropped quietly.

use fs_calendar_testdata::draft::{
    Alert, AlertAction, AppointmentPatch, BasicValue, LocalDateTime, Loss, ParticipationStatus,
    Roles, TextType, Trigger, When,
};
use serde_json::Value;

use crate::jscal_in::{RawPerson, pick, text};
use crate::jscal_util::{flag_keys, parse_offset, parse_utc};
use crate::jscal_util::{format_duration, format_utc};
use crate::words::{ALERT_ACTION, FREE_BUSY, PARTICIPATION, PRIVACY, RELATIVE_TO, STATUS, word};
use serde_json::json;

pub fn alerts_of(v: &Value, uid: &str, losses: &mut Vec<Loss>) -> Vec<Alert> {
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

pub fn people_of(v: &Value, uid: &str, losses: &mut Vec<Loss>) -> Vec<RawPerson> {
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

pub fn patch_of(o: &Value, base: &When, uid: &str, losses: &mut Vec<Loss>) -> AppointmentPatch {
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

/// An alert as JSON.
#[must_use]
pub fn alert_json(a: &Alert) -> Value {
    let trigger = match a.trigger {
        Trigger::Offset {
            seconds,
            relative_to,
        } => {
            let sign = if seconds < 0 { "-" } else { "" };
            let mag = seconds.unsigned_abs();
            let span = fs_calendar_testdata::draft::Span {
                days: 0,
                seconds: u32::try_from(mag).unwrap_or(u32::MAX),
            };
            json!({"@type": "OffsetTrigger", "offset": format!("{sign}{}", format_duration(span)), "relativeTo": word(&RELATIVE_TO, relative_to)})
        }
        Trigger::Absolute { at } => json!({"@type": "AbsoluteTrigger", "when": format_utc(at)}),
    };
    json!({"@type": "Alert", "trigger": trigger, "action": word(&ALERT_ACTION, a.action)})
}
