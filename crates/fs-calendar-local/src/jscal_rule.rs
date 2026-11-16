//! A recurrence rule as JSCalendar JSON (`recurrenceRule`) and back.
//!
//! Pattern: Adapter (typed rule to JSON and back). A rule with a part the engine does not expand is read as
//! `None`: the caller keeps the JSON as written (`calendar_unread`) and reports a loss.

use fs_calendar_testdata::draft::{BasicValue, LocalDateTime};
use fs_calendar_testdata::draft::{ByMonth, NDay, RecurrenceRule, RuleEnd};
use serde_json::{Map, Value, json};

use crate::rules::{DAYS, FREQ, SKIPS};
use crate::words::{from_word, word};

fn ints<T: TryFrom<i64>>(v: Option<&Value>) -> Option<Vec<T>> {
    let Some(a) = v else { return Some(Vec::new()) };
    a.as_array()?
        .iter()
        .map(|x| T::try_from(x.as_i64()?).ok())
        .collect()
}

/// The rule of a JSON object; `None` when it has a part this program does not interpret (the engine's subset).
#[must_use]
pub fn rule_from_json(v: &Value) -> Option<RecurrenceRule> {
    let o = v.as_object()?;
    let known = [
        "@type",
        "frequency",
        "interval",
        "rscale",
        "skip",
        "firstDayOfWeek",
        "byDay",
        "byMonthDay",
        "byMonth",
        "bySetPosition",
        "count",
        "until",
    ];
    if o.keys().any(|k| !known.contains(&k.as_str())) {
        return None;
    }
    if o.get("rscale")
        .and_then(Value::as_str)
        .is_some_and(|r| !r.eq_ignore_ascii_case("gregorian"))
    {
        return None;
    }
    let by_day = o
        .get("byDay")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|d| {
                    Some(NDay {
                        day: from_word(&DAYS, d.get("day")?.as_str()?).ok()?,
                        nth: d
                            .get("nthOfPeriod")
                            .and_then(Value::as_i64)
                            .map(|n| i16::try_from(n).ok())
                            .map(|n| n.unwrap_or(0)),
                    })
                })
                .collect::<Option<Vec<_>>>()
        })
        .unwrap_or_else(|| Some(Vec::new()))?;
    let by_month = o
        .get("byMonth")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|m| {
                    let t = m.as_str()?;
                    Some(ByMonth {
                        month: t.trim_end_matches('L').parse().ok()?,
                        leap: t.ends_with('L'),
                    })
                })
                .collect::<Option<Vec<_>>>()
        })
        .unwrap_or_else(|| Some(Vec::new()))?;
    let end = match (
        o.get("count").and_then(Value::as_u64),
        o.get("until").and_then(Value::as_str),
    ) {
        (Some(n), _) => RuleEnd::Count(u32::try_from(n).ok()?),
        (None, Some(u)) => RuleEnd::Until(LocalDateTime::parse(u.trim_end_matches('Z')).ok()?),
        _ => RuleEnd::Never,
    };
    Some(RecurrenceRule {
        frequency: from_word(&FREQ, o.get("frequency")?.as_str()?).ok()?,
        interval: u32::try_from(o.get("interval").and_then(Value::as_u64).unwrap_or(1)).ok()?,
        rscale: None,
        skip: o
            .get("skip")
            .and_then(Value::as_str)
            .map_or(Some(fs_calendar_testdata::draft::Skip::Omit), |s| {
                from_word(&SKIPS, s).ok()
            })?,
        first_day_of_week: o
            .get("firstDayOfWeek")
            .and_then(Value::as_str)
            .map_or(Some(fs_calendar_testdata::draft::Weekday::Mo), |s| {
                from_word(&DAYS, s).ok()
            })?,
        by_day,
        by_month_day: ints(o.get("byMonthDay"))?,
        by_month,
        by_year_day: Vec::new(),
        by_week_no: Vec::new(),
        by_hour: Vec::new(),
        by_minute: Vec::new(),
        by_second: Vec::new(),
        by_set_position: ints(o.get("bySetPosition"))?,
        end,
    })
}

/// The JSON object of a rule.
#[must_use]
pub fn rule_to_json(r: &RecurrenceRule) -> Value {
    let mut o = Map::new();
    o.insert("@type".into(), json!("RecurrenceRule"));
    o.insert("frequency".into(), json!(word(&FREQ, r.frequency)));
    if r.interval != 1 {
        o.insert("interval".into(), json!(r.interval));
    }
    if let Some(s) = &r.rscale {
        o.insert("rscale".into(), json!(s));
    }
    if r.skip != fs_calendar_testdata::draft::Skip::Omit {
        o.insert("skip".into(), json!(word(&SKIPS, r.skip)));
    }
    if r.first_day_of_week != fs_calendar_testdata::draft::Weekday::Mo {
        o.insert(
            "firstDayOfWeek".into(),
            json!(word(&DAYS, r.first_day_of_week)),
        );
    }
    if !r.by_day.is_empty() {
        let days: Vec<Value> = r
            .by_day
            .iter()
            .map(|d| {
                let mut m = json!({"@type": "NDay", "day": word(&DAYS, d.day)});
                if let Some(n) = d.nth {
                    m["nthOfPeriod"] = json!(n);
                }
                m
            })
            .collect();
        o.insert("byDay".into(), json!(days));
    }
    let lists: [(&str, Vec<i64>); 6] = [
        (
            "byMonthDay",
            r.by_month_day.iter().map(|v| i64::from(*v)).collect(),
        ),
        (
            "byYearDay",
            r.by_year_day.iter().map(|v| i64::from(*v)).collect(),
        ),
        (
            "byWeekNo",
            r.by_week_no.iter().map(|v| i64::from(*v)).collect(),
        ),
        ("byHour", r.by_hour.iter().map(|v| i64::from(*v)).collect()),
        (
            "byMinute",
            r.by_minute.iter().map(|v| i64::from(*v)).collect(),
        ),
        (
            "bySecond",
            r.by_second.iter().map(|v| i64::from(*v)).collect(),
        ),
    ];
    for (k, l) in lists {
        if !l.is_empty() {
            o.insert(k.into(), json!(l));
        }
    }
    if !r.by_month.is_empty() {
        o.insert(
            "byMonth".into(),
            json!(
                r.by_month
                    .iter()
                    .map(|m| format!("{}{}", m.month, if m.leap { "L" } else { "" }))
                    .collect::<Vec<_>>()
            ),
        );
    }
    if !r.by_set_position.is_empty() {
        o.insert("bySetPosition".into(), json!(r.by_set_position));
    }
    match r.end {
        RuleEnd::Never => {}
        RuleEnd::Count(n) => {
            o.insert("count".into(), json!(n));
        }
        RuleEnd::Until(l) => {
            o.insert("until".into(), json!(l.canonical()));
        }
    }
    Value::Object(o)
}
