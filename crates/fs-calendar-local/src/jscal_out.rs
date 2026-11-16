//! A stored appointment as a JSCalendar `Event` (JSON), the one form both export formats start from.
//!
//! Pattern: Adapter. What the form cannot hold (`excludedRecurrenceRules`, a second rule) goes into the loss list,
//! never away quietly. Participants carry the address the person directory gave; one without an address is a loss.

#![allow(clippy::too_many_lines)]

use std::collections::HashMap;

use fs_calendar_testdata::draft::{
    Appointment, AppointmentPatch, Attendee, BasicValue, Loss, OverrideChange, Participant,
    TextType, When,
};
use serde_json::{Map, Value, json};

use crate::jscal_rule::rule_to_json;
use crate::jscal_util::{format_duration, format_utc};
use crate::words::{FREE_BUSY, PARTICIPATION, PRIVACY, STATUS, word};

/// The members of an event that make up its recurrence.
pub const RECURRENCE_KEYS: [&str; 4] = [
    "recurrenceRule",
    "recurrenceRules",
    "excludedRecurrenceRules",
    "recurrenceOverrides",
];

/// What the person directory knows of a participant: `(name, address)`, by participant key.
pub type People = HashMap<String, (Option<String>, Option<String>)>;

fn flags<'a>(it: impl IntoIterator<Item = &'a String>) -> Value {
    Value::Object(
        it.into_iter()
            .map(|k| (k.clone(), Value::Bool(true)))
            .collect(),
    )
}

fn put_when(o: &mut Map<String, Value>, w: &When) {
    match w {
        When::Timed {
            start,
            zone,
            length,
            end_zone,
            ..
        } => {
            o.insert("start".into(), json!(start.canonical()));
            o.insert("timeZone".into(), json!(zone.id()));
            if let Some(z) = end_zone {
                o.insert("endTimeZone".into(), json!(z.id()));
            }
            o.insert("duration".into(), json!(format_duration(*length)));
        }
        When::Floating { start, length } => {
            o.insert("start".into(), json!(start.canonical()));
            o.insert("duration".into(), json!(format_duration(*length)));
            o.insert(
                "iCalendar".into(),
                json!({"name": "vevent", "properties": [["x-fs-floating", {}, "unknown", "TRUE"]]}),
            );
        }
        When::AllDay { first, days } => {
            o.insert(
                "start".into(),
                json!(format!("{}T00:00:00", first.canonical())),
            );
            o.insert("showWithoutTime".into(), json!(true));
            o.insert("duration".into(), json!(format!("P{days}D")));
        }
    }
}

fn participant(
    p: &Participant,
    people: &People,
    uid: &str,
    losses: &mut Vec<Loss>,
) -> Option<Value> {
    let (name, addr) = people.get(&p.key).cloned().unwrap_or((None, None));
    let (kind, fallback) = match &p.who {
        Attendee::Actor(r) => (
            if matches!(
                r.art,
                fs_calendar_testdata::draft::PersonArt::Group
                    | fs_calendar_testdata::draft::PersonArt::Organization
            ) {
                "group"
            } else {
                "individual"
            },
            None,
        ),
        Attendee::Resource(r) => ("resource", Some(r.canonical())),
        Attendee::Location(r) => ("location", Some(r.canonical())),
    };
    let Some(address) = addr.or(fallback.filter(|a| a.starts_with("mailto:"))) else {
        losses.push(Loss {
            uid: Some(uid.to_owned()),
            field: format!("participants/{}", p.key),
            reason: "no address for this person: left out of the file".into(),
        });
        return None;
    };
    let r = p.roles;
    let roles: Vec<String> = [
        ("owner", r.owner),
        ("attendee", r.attendee),
        ("optional", r.optional),
        ("informational", r.informational),
        ("chair", r.chair),
        ("contact", r.contact),
    ]
    .into_iter()
    .filter(|(_, on)| *on)
    .map(|(n, _)| n.to_owned())
    .collect();
    let mut o = json!({"@type": "Participant", "calendarAddress": address, "kind": kind, "roles": flags(&roles), "participationStatus": word(&PARTICIPATION, p.status), "expectReply": p.expect_reply});
    if let Some(n) = name {
        o["name"] = json!(n);
    }
    Some(o)
}

fn override_patch(p: &AppointmentPatch, uid: &str, rid: &str, losses: &mut Vec<Loss>) -> Value {
    let mut o = Map::new();
    if let Some(t) = &p.title {
        o.insert("title".into(), json!(t));
    }
    if let Some((d, _)) = &p.description {
        o.insert("description".into(), json!(d));
    }
    if let Some(w) = &p.when {
        let mut m = Map::new();
        put_when(&mut m, w);
        if let Some(s) = m.get("start") {
            o.insert("start".into(), s.clone());
        }
    }
    if let Some(s) = p.status {
        o.insert("status".into(), json!(word(&STATUS, s)));
    }
    if let Some(s) = p.free_busy {
        o.insert("freeBusyStatus".into(), json!(word(&FREE_BUSY, s)));
    }
    if let Some(s) = p.privacy {
        o.insert("privacy".into(), json!(word(&PRIVACY, s)));
    }
    if let Some(n) = p.priority {
        o.insert("priority".into(), json!(n));
    }
    if p.participants.is_some()
        || p.alerts.is_some()
        || p.keywords.is_some()
        || p.categories.is_some()
        || p.locale.is_some()
        || p.color.is_some()
        || p.use_default_alerts.is_some()
    {
        losses.push(Loss { uid: Some(uid.to_owned()), field: format!("recurrenceOverrides/{rid}"), reason: "participants, alerts, keywords, categories, locale, colour of one occurrence are not written".into() });
    }
    Value::Object(o)
}

/// The event as JSON. `kept` is the whole recurrence part of the original (rule, `EXDATE`/overrides) of a series whose
/// rule is outside the engine's subset: it replaces what is built here and is written back as it was received.
#[must_use]
pub fn event_json(
    a: &Appointment,
    people: &People,
    kept: Option<&Map<String, Value>>,
    losses: &mut Vec<Loss>,
) -> Value {
    let mut o = Map::new();
    o.insert("@type".into(), json!("Event"));
    o.insert("version".into(), json!("2.0"));
    o.insert("uid".into(), json!(a.uid));
    o.insert("title".into(), json!(a.title));
    if !a.description.is_empty() {
        o.insert("description".into(), json!(a.description));
        if a.description_type == TextType::Html {
            o.insert("descriptionContentType".into(), json!("text/html"));
        }
    }
    if let Some(l) = &a.locale {
        o.insert("locale".into(), json!(l));
    }
    put_when(&mut o, &a.when);
    o.insert("status".into(), json!(word(&STATUS, a.status)));
    o.insert(
        "freeBusyStatus".into(),
        json!(word(&FREE_BUSY, a.free_busy)),
    );
    o.insert("privacy".into(), json!(word(&PRIVACY, a.privacy)));
    if a.priority != 0 {
        o.insert("priority".into(), json!(a.priority));
    }
    if let Some(c) = &a.color {
        o.insert("color".into(), json!(c));
    }
    if !a.keywords.is_empty() {
        o.insert("keywords".into(), flags(&a.keywords));
    }
    if !a.categories.is_empty() {
        o.insert("categories".into(), flags(&a.categories));
    }
    let ps: Map<String, Value> = a
        .participants
        .iter()
        .filter_map(|p| participant(p, people, &a.uid, losses).map(|v| (p.key.clone(), v)))
        .collect();
    if !ps.is_empty() {
        o.insert("participants".into(), Value::Object(ps));
    }
    if !a.alerts.is_empty() {
        o.insert(
            "alerts".into(),
            Value::Object(
                a.alerts
                    .iter()
                    .map(|x| (x.key.clone(), crate::jscal_parts::alert_json(x)))
                    .collect(),
            ),
        );
    }
    if a.use_default_alerts {
        o.insert("useDefaultAlerts".into(), json!(true));
    }
    if let [first, rest @ ..] = a.recurrence.as_slice() {
        o.insert("recurrenceRule".into(), rule_to_json(first));
        if !rest.is_empty() {
            losses.push(Loss {
                uid: Some(a.uid.clone()),
                field: "recurrence".into(),
                reason: format!("{} further rule(s): JSCalendar 2.0 allows one", rest.len()),
            });
        }
    }
    if !a.excluded_recurrence.is_empty() {
        losses.push(Loss {
            uid: Some(a.uid.clone()),
            field: "excludedRecurrenceRules".into(),
            reason: "not part of JSCalendar 2.0: left out".into(),
        });
    }
    let overrides: Map<String, Value> = a
        .overrides
        .iter()
        .map(|ov| {
            let rid = ov.recurrence_id.canonical();
            let v = match &ov.change {
                OverrideChange::Excluded => json!({"excluded": true}),
                OverrideChange::Changed(p) => override_patch(p, &a.uid, &rid, losses),
            };
            (rid, v)
        })
        .collect();
    if !overrides.is_empty() {
        o.insert("recurrenceOverrides".into(), Value::Object(overrides));
    }
    o.insert("created".into(), json!(format_utc(a.created)));
    o.insert("updated".into(), json!(format_utc(a.updated)));
    o.insert("sequence".into(), json!(a.sequence));
    for (k, n) in [
        ("locations", a.locations.len()),
        ("virtualLocations", a.virtual_locations.len()),
        ("links", a.links.len()),
    ] {
        if n > 0 {
            losses.push(Loss {
                uid: Some(a.uid.clone()),
                field: k.to_owned(),
                reason: "not stored, so not written".into(),
            });
        }
    }
    if let Some(kept) = kept {
        for k in RECURRENCE_KEYS {
            o.remove(k);
        }
        o.extend(kept.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    Value::Object(o)
}
