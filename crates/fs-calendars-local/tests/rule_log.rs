//! ADR 0086: the log of a rule change is typed and complete (every part of the rule), no debug text.

#![allow(
    clippy::many_single_char_names,
    clippy::assert_is_empty,
    clippy::too_many_lines,
    clippy::type_complexity,
    clippy::semicolon_if_nothing_returned
)]

mod common;

use common::fx;
use common::series::{title, weekly};
use fs_calendars_local::fields::{diff, flatten};
use fs_calendars_testdata::draft::Recurrence;
use fs_calendars_testdata::draft::{
    AppointmentPatch, Appointments, ByMonth, Frequency, NDay, RecurrenceRule, RuleEnd, SeriesScope,
    Skip, Value, Weekday,
};
use fs_calendars_testdata::sample::rule;
use fs_calendars_testdata::state::blank;

fn with(f: impl FnOnce(&mut RecurrenceRule)) -> AppointmentPatch {
    let mut r = rule(Frequency::Weekly);
    r.end = RuleEnd::Count(5);
    f(&mut r);
    AppointmentPatch {
        recurrence: Some(vec![Recurrence::Rule(r)]),
        ..AppointmentPatch::default()
    }
}

#[tokio::test]
async fn every_part_of_a_rule_shows_up_typed_when_it_changes() {
    let f = fx().await;
    let a = weekly(&f).await;
    let parts: Vec<(&str, Box<dyn FnOnce(&mut RecurrenceRule)>, Value)> = vec![
        (
            "rule.by_hour",
            Box::new(|r| r.by_hour = vec![9]),
            Value::Int(9),
        ),
        (
            "rule.by_minute",
            Box::new(|r| r.by_minute = vec![30]),
            Value::Int(30),
        ),
        (
            "rule.by_second",
            Box::new(|r| r.by_second = vec![15]),
            Value::Int(15),
        ),
        (
            "rule.by_year_day",
            Box::new(|r| r.by_year_day = vec![100]),
            Value::Int(100),
        ),
        (
            "rule.by_week_no",
            Box::new(|r| r.by_week_no = vec![-1]),
            Value::Int(-1),
        ),
        (
            "rule.by_month_day",
            Box::new(|r| r.by_month_day = vec![15]),
            Value::Int(15),
        ),
        (
            "rule.by_set_position",
            Box::new(|r| r.by_set_position = vec![-1]),
            Value::Int(-1),
        ),
        (
            "rule.by_month",
            Box::new(|r| {
                r.by_month = vec![ByMonth {
                    month: 6,
                    leap: false,
                }]
            }),
            Value::Int(6),
        ),
        (
            "rule.by_day",
            Box::new(|r| {
                r.by_day = vec![NDay {
                    day: Weekday::Fr,
                    nth: Some(-1),
                }]
            }),
            Value::Text("fr".into()),
        ),
        (
            "rule.by_day_nth",
            Box::new(|r| {
                r.by_day = vec![NDay {
                    day: Weekday::Fr,
                    nth: Some(-1),
                }]
            }),
            Value::Int(-1),
        ),
        (
            "rule.skip",
            Box::new(|r| r.skip = Skip::Forward),
            Value::Text("forward".into()),
        ),
        (
            "rule.first_day_of_week",
            Box::new(|r| r.first_day_of_week = Weekday::Su),
            Value::Text("su".into()),
        ),
        (
            "rule.rscale",
            Box::new(|r| r.rscale = Some("gregorian".into())),
            Value::Text("gregorian".into()),
        ),
        (
            "rule.frequency",
            Box::new(|r| r.frequency = Frequency::Daily),
            Value::Text("daily".into()),
        ),
        ("rule.interval", Box::new(|r| r.interval = 2), Value::Int(2)),
    ];
    let base = |change: Box<dyn FnOnce(&mut RecurrenceRule)>| {
        let mut x = blank(
            fs_calendars_testdata::ids::object_id(1),
            fs_calendars_testdata::ids::object_id(2),
            "u".into(),
            "t",
            a.when.clone(),
            fs_calendars_testdata::draft::Utc::from_millis(0),
        );
        x.recurrence = with(change).recurrence.unwrap();
        x
    };
    let plain = base(Box::new(|_| {}));
    let mut texts = Vec::new();
    for (field, change, expect) in parts {
        let changes = diff(&flatten(&plain), &flatten(&base(change)));
        let hit = changes
            .iter()
            .find(|c| c.field == field)
            .unwrap_or_else(|| panic!("{field} missing: {changes:?}"));
        assert_eq!(hit.new, Some(expect), "{field}");
        assert!(
            hit.row_key.as_deref().is_some_and(|k| k.starts_with("r0")),
            "{field}"
        );
        texts.extend(
            changes
                .iter()
                .flat_map(|c| [&c.old, &c.new])
                .filter_map(|v| {
                    if let Some(Value::Text(t)) = v {
                        Some(t.clone())
                    } else {
                        None
                    }
                }),
        );
    }
    assert!(
        texts
            .iter()
            .all(|t| !t.contains('[') && !t.contains('{') && !t.contains("Weekly")),
        "no debug text: {texts:?}"
    );
    // End to end through the trait: a change of the rule is in the history of the series.
    let next =
        f.p.update(
            a.id,
            a.revision,
            with(|r| r.interval = 2),
            SeriesScope::Whole,
        )
        .await
        .unwrap();
    let h = f.p.history(next.id, None, 10).await.unwrap().items;
    assert!(h[1].changes.iter().any(|c| c.field == "rule.interval"
        && c.new == Some(Value::Int(2))
        && c.old == Some(Value::Int(1))));
    let _ = title("x");
}
