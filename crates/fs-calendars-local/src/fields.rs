//! An appointment as a flat list of typed fields, and the difference of two such lists (the typed log, R10).
//!
//! Pattern: Visitor-free flattening (one function lists every field of the slice) plus a pure `diff`.

use std::collections::BTreeMap;

use fs_calendars_testdata::draft::{
    Alert, Appointment, AppointmentPatch, Attendee, FieldChange, OverrideChange, Participant,
    Trigger, Value as Val, When,
};

use crate::words::{
    ALERT_ACTION, FREE_BUSY, PARTICIPATION, PRIVACY, RELATIVE_TO, STATUS, TEXT_TYPE, word,
};

/// `(field, row key)` to value.
pub type Flat = BTreeMap<(String, Option<String>), Val>;

pub(crate) fn put(f: &mut Flat, field: &str, key: Option<&str>, v: Val) {
    f.insert((field.to_owned(), key.map(str::to_owned)), v);
}

fn text(s: &str) -> Val {
    Val::Text(s.to_owned())
}

/// Every stored field of an appointment (not `updated`, which is a stamp).
#[must_use]
pub fn flatten(a: &Appointment) -> Flat {
    let mut f = Flat::new();
    put(&mut f, "title", None, text(&a.title));
    put(&mut f, "description", None, text(&a.description));
    put(
        &mut f,
        "description_type",
        None,
        text(word(&TEXT_TYPE, a.description_type)),
    );
    if let Some(l) = &a.locale {
        put(&mut f, "locale", None, text(l));
    }
    if let Some(c) = &a.color {
        put(&mut f, "color", None, text(c));
    }
    put(&mut f, "status", None, text(word(&STATUS, a.status)));
    put(
        &mut f,
        "free_busy",
        None,
        text(word(&FREE_BUSY, a.free_busy)),
    );
    put(&mut f, "privacy", None, text(word(&PRIVACY, a.privacy)));
    put(&mut f, "priority", None, Val::Int(i64::from(a.priority)));
    put(&mut f, "sequence", None, Val::Int(i64::from(a.sequence)));
    for (n, v) in [
        ("use_default_alerts", a.use_default_alerts),
        ("hide_attendees", a.hide_attendees),
        ("may_invite_self", a.may_invite_self),
        ("may_invite_others", a.may_invite_others),
    ] {
        put(&mut f, n, None, Val::Bool(v));
    }
    when(&mut f, &a.when);
    for k in &a.keywords {
        put(&mut f, "keyword", Some(k), text(k));
    }
    for k in &a.categories {
        put(&mut f, "category", Some(k), text(k));
    }
    a.participants.iter().for_each(|p| participant(&mut f, p));
    a.alerts.iter().for_each(|x| alert(&mut f, x));
    for (ex, rules) in [("r", &a.recurrence), ("x", &a.excluded_recurrence)] {
        rules
            .iter()
            .enumerate()
            .for_each(|(i, r)| crate::fields_rule::rule(&mut f, &format!("{ex}{i}"), r));
    }
    for r in &a.related {
        put(
            &mut f,
            "related",
            Some(&r.uid),
            text(word(&crate::rules::RELATIONS, r.relation)),
        );
    }
    f
}

fn when(f: &mut Flat, w: &When) {
    match w {
        When::Timed {
            start,
            zone,
            length,
            end_zone,
            zone_assumed,
        } => {
            put(f, "when_kind", None, text("timed"));
            put(f, "when_local", None, Val::Local(*start));
            put(f, "when_zone", None, Val::Zone(zone.clone()));
            if let Some(z) = end_zone {
                put(f, "when_end_zone", None, Val::Zone(z.clone()));
            }
            put(f, "when_zone_assumed", None, Val::Bool(*zone_assumed));
            put(f, "when_len_days", None, Val::Int(i64::from(length.days)));
            put(
                f,
                "when_len_seconds",
                None,
                Val::Int(i64::from(length.seconds)),
            );
        }
        When::Floating { start, length } => {
            put(f, "when_kind", None, text("floating"));
            put(f, "when_local", None, Val::Local(*start));
            put(f, "when_len_days", None, Val::Int(i64::from(length.days)));
            put(
                f,
                "when_len_seconds",
                None,
                Val::Int(i64::from(length.seconds)),
            );
        }
        When::AllDay { first, days } => {
            put(f, "when_kind", None, text("all_day"));
            put(f, "when_date", None, Val::Date(*first));
            put(f, "when_len_days", None, Val::Int(i64::from(*days)));
        }
    }
}

fn participant(f: &mut Flat, p: &Participant) {
    let k = Some(p.key.as_str());
    put(f, "participant.who", k, Val::Attendee(p.who.clone()));
    for (n, v) in [
        ("owner", p.roles.owner),
        ("attendee", p.roles.attendee),
        ("optional", p.roles.optional),
        ("informational", p.roles.informational),
        ("chair", p.roles.chair),
        ("contact", p.roles.contact),
    ] {
        put(f, &format!("participant.role_{n}"), k, Val::Bool(v));
    }
    put(
        f,
        "participant.status",
        k,
        text(word(&PARTICIPATION, p.status)),
    );
    put(f, "participant.expect_reply", k, Val::Bool(p.expect_reply));
    if let Some(c) = &p.comment {
        put(f, "participant.comment", k, text(c));
    }
    if let Some(c) = &p.invited_by {
        put(f, "participant.invited_by", k, text(c));
    }
    put(
        f,
        "participant.schedule_sequence",
        k,
        Val::Int(i64::from(p.schedule_sequence)),
    );
    if let Some(u) = p.schedule_updated {
        put(f, "participant.schedule_updated", k, Val::Utc(u));
    }
}

fn alert(f: &mut Flat, a: &Alert) {
    let k = Some(a.key.as_str());
    match a.trigger {
        Trigger::Offset {
            seconds,
            relative_to,
        } => {
            put(f, "alert.trigger", k, text("offset"));
            put(f, "alert.offset_seconds", k, Val::Int(seconds));
            put(
                f,
                "alert.relative_to",
                k,
                text(word(&RELATIVE_TO, relative_to)),
            );
        }
        Trigger::Absolute { at } => {
            put(f, "alert.trigger", k, text("absolute"));
            put(f, "alert.at", k, Val::Utc(at));
        }
    }
    put(f, "alert.action", k, text(word(&ALERT_ACTION, a.action)));
    if let Some(u) = a.acknowledged {
        put(f, "alert.acknowledged", k, Val::Utc(u));
    }
}

/// The field changes from `old` to `new` (absent = `None`); empty when nothing differs.
#[must_use]
pub fn diff(old: &Flat, new: &Flat) -> Vec<FieldChange> {
    let mut keys: Vec<&(String, Option<String>)> = old.keys().chain(new.keys()).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter(|k| old.get(*k) != new.get(*k))
        .map(|k| FieldChange {
            field: k.0.clone(),
            row_key: k.1.clone(),
            old: old.get(k).cloned(),
            new: new.get(k).cloned(),
        })
        .collect()
}

/// Whether the slice stores everything of this appointment; the first thing it cannot is named.
#[must_use]
pub fn unsupported(a: &Appointment) -> Option<&'static str> {
    let moved = |f: &dyn Fn(&AppointmentPatch) -> bool| {
        a.overrides
            .iter()
            .any(|o| matches!(&o.change, OverrideChange::Changed(p) if f(p)))
    };
    let rest: [(&'static str, bool); 8] = [
        (
            "locations",
            !a.locations.is_empty()
                || moved(&|p| p.locations.as_ref().is_some_and(|l| !l.is_empty())),
        ),
        (
            "virtual_locations",
            !a.virtual_locations.is_empty()
                || moved(&|p| p.virtual_locations.as_ref().is_some_and(|l| !l.is_empty())),
        ),
        (
            "links",
            !a.links.is_empty() || moved(&|p| p.links.as_ref().is_some_and(|l| !l.is_empty())),
        ),
        ("reply_to", !a.reply_to.is_empty()),
        (
            "localizations",
            !a.localizations.is_empty()
                || moved(&|p| p.localizations.as_ref().is_some_and(|l| !l.is_empty())),
        ),
        (
            "participants.send_to_or_delegation",
            a.participants.iter().any(|p| {
                !p.send_to.is_empty() || !p.delegated_to.is_empty() || !p.delegated_from.is_empty()
            }),
        ),
        (
            "overrides.recurrence",
            moved(&|p| p.recurrence.is_some() || p.excluded_recurrence.is_some()),
        ),
        (
            "overrides.participants.send_to_or_delegation",
            moved(&|p| {
                p.participants.as_ref().is_some_and(|l| {
                    l.iter().any(|x| {
                        !x.send_to.is_empty()
                            || !x.delegated_to.is_empty()
                            || !x.delegated_from.is_empty()
                    })
                })
            }),
        ),
    ];
    rest.iter().find(|(_, set)| *set).map(|(n, _)| *n)
}

/// Whether a participant is a person (for "my status").
#[must_use]
pub fn is_actor(p: &Participant, who: &Attendee) -> bool {
    &p.who == who
}
