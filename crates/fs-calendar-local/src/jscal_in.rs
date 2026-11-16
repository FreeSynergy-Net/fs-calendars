//! A JSCalendar `Event` (JSON) read into the typed values of the trait: the normalization at the border (ADR 0085).
//!
//! Pattern: Adapter. Nothing is lost quietly: what this program does not store (yet) or cannot interpret goes into
//! `Read::losses`, and the received file stays as the original. Time: a time without a zone becomes `Timed` in the
//! assumed zone with `zone_assumed`, unless the file says "floating" (`X-FS-FLOATING`); all-day is a date, never moved.

#![allow(clippy::too_many_lines, clippy::large_enum_variant)]

use fs_calendar_testdata::draft::{
    AppointmentPatch, BasicValue, ImportContext, LocalDateTime, Loss, ParticipationStatus,
    RecurrenceRule, Result, Roles, TextType, When,
};
use fs_calendar_testdata::recur::check;
use serde_json::Value;

use crate::codec::invalid;
use crate::jscal_parts::{alerts_of, patch_of, people_of};
use crate::jscal_rule::rule_from_json;
use crate::jscal_time::when_of;
use crate::jscal_util::flag_keys;
use crate::words::{FREE_BUSY, PRIVACY, STATUS, from_word};

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
    pub unread_rule: bool,
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

pub(crate) fn text<'a>(o: &'a Value, k: &str) -> Option<&'a str> {
    o.get(k).and_then(Value::as_str)
}

pub(crate) fn pick<T: Copy>(t: &[(T, &'static str)], o: &Value, k: &str) -> Option<T> {
    text(o, k).and_then(|w| from_word(t, w).ok())
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
    let (mut read, mut unread): (Vec<RecurrenceRule>, bool) = (Vec::new(), false);
    for r in rules {
        match rule_from_json(r).filter(|x| check(x).is_empty()) {
            Some(x) => read.push(x),
            None => unread = true,
        }
    }
    if unread {
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
            losses.push(Loss {
                uid: Some(uid.clone()),
                field: format!("recurrenceOverrides/{k}"),
                reason: "not a local date-time: left out, kept only in the original".into(),
            });
            continue;
        };
        if unread {
            losses.push(Loss {
                uid: Some(uid.clone()),
                field: format!("recurrenceOverrides/{k}"),
                reason: "the series is not read, so its exceptions are not either; kept in the original and written back from it".into(),
            });
            continue;
        }
        let change = if o.get("excluded").and_then(Value::as_bool) == Some(true) {
            RawChange::Excluded
        } else {
            RawChange::Changed(patch_of(o, &when, &uid, &mut losses))
        };
        overrides.push((rid, change));
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
        unread_rule: unread,
        losses,
        zone_assumed,
        prod_id: text(v, "prodId").map(str::to_owned),
    })
}
