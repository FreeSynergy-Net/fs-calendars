//! A recurrence rule as typed log fields (ADR 0086): one entry per scalar and one per by-part value, no debug text.
//!
//! Pattern: Visitor-free flattening (as `fields`); the row key is the rule (`r0`, `x0` for an excluding rule) and, for
//! a by-part value, `rule:position` so that a change of one value is one entry.

use fs_calendar_testdata::draft::{RecurrenceRule, RuleEnd, Value as Val};

use crate::fields::{Flat, put};
use crate::rules::{DAYS, FREQ, SKIPS};
use crate::words::word;

fn int(v: impl Into<i64>) -> Val {
    Val::Int(v.into())
}

/// Every stored part of a rule: frequency, interval, `rscale`, `skip`, `first_day_of_week` (wkst), the end, and all
/// by-parts (`by_day` with its `nth`, month with the leap mark, month day, year day, week number, hour, minute,
/// second, set position).
pub fn rule(f: &mut Flat, k: &str, r: &RecurrenceRule) {
    let key = Some(k);
    put(
        f,
        "rule.frequency",
        key,
        Val::Text(word(&FREQ, r.frequency).to_owned()),
    );
    put(f, "rule.interval", key, int(r.interval));
    if let Some(s) = &r.rscale {
        put(f, "rule.rscale", key, Val::Text(s.clone()));
    }
    put(
        f,
        "rule.skip",
        key,
        Val::Text(word(&SKIPS, r.skip).to_owned()),
    );
    put(
        f,
        "rule.first_day_of_week",
        key,
        Val::Text(word(&DAYS, r.first_day_of_week).to_owned()),
    );
    match r.end {
        RuleEnd::Never => {}
        RuleEnd::Count(n) => put(f, "rule.count", key, int(n)),
        RuleEnd::Until(l) => put(f, "rule.until", key, Val::Local(l)),
    }
    let at = |i: usize| Some(format!("{k}:{i}"));
    let mut part = |field: &str, i: usize, v: Val| put(f, field, at(i).as_deref(), v);
    for (i, d) in r.by_day.iter().enumerate() {
        part("rule.by_day", i, Val::Text(word(&DAYS, d.day).to_owned()));
        if let Some(n) = d.nth {
            part("rule.by_day_nth", i, int(n));
        }
    }
    for (i, m) in r.by_month.iter().enumerate() {
        part("rule.by_month", i, int(m.month));
        part("rule.by_month_leap", i, Val::Bool(m.leap));
    }
    let lists: [(&str, Vec<i64>); 7] = [
        (
            "rule.by_month_day",
            r.by_month_day.iter().map(|v| i64::from(*v)).collect(),
        ),
        (
            "rule.by_year_day",
            r.by_year_day.iter().map(|v| i64::from(*v)).collect(),
        ),
        (
            "rule.by_week_no",
            r.by_week_no.iter().map(|v| i64::from(*v)).collect(),
        ),
        (
            "rule.by_hour",
            r.by_hour.iter().map(|v| i64::from(*v)).collect(),
        ),
        (
            "rule.by_minute",
            r.by_minute.iter().map(|v| i64::from(*v)).collect(),
        ),
        (
            "rule.by_second",
            r.by_second.iter().map(|v| i64::from(*v)).collect(),
        ),
        (
            "rule.by_set_position",
            r.by_set_position.iter().map(|v| i64::from(*v)).collect(),
        ),
    ];
    for (field, values) in lists {
        for (i, v) in values.into_iter().enumerate() {
            part(field, i, int(v));
        }
    }
}
