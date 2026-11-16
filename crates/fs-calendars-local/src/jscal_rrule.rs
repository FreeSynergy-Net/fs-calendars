//! The RRULE text of a JSCalendar recurrence rule object, for a rule the program cannot interpret
//! (`Recurrence::Original`, ADR 0030).
//!
//! Pattern: Adapter (JSON members to RRULE parts). The exact bytes of what was received stay in the kept original
//! file (`object_original`); this text is what the typed store carries so that a series is never silently lost.
//!
//! Known limit (parts.rs asks for the received text byte for byte): a rule that came as JSON is rebuilt here from its
//! members, so the stored `Original` text is NOT byte for byte what was received (the order and spelling of the parts
//! are ours). The export is right all the same: it takes the received file from `object_original`, never this text.

use serde_json::Value;

fn list(o: &serde_json::Map<String, Value>, key: &str, name: &str, parts: &mut Vec<String>) {
    let Some(a) = o.get(key).and_then(Value::as_array) else {
        return;
    };
    let items: Vec<String> = a
        .iter()
        .filter_map(|x| {
            x.as_i64()
                .map(|n| n.to_string())
                .or_else(|| x.as_str().map(str::to_owned))
        })
        .collect();
    if !items.is_empty() {
        parts.push(format!("{name}={}", items.join(",")));
    }
}

/// The RRULE text of a recurrence rule object (`FREQ=...;INTERVAL=...;BYDAY=...`), best effort for the members the
/// subset does not have.
#[must_use]
pub fn rrule_of_json(v: &Value) -> String {
    let Some(o) = v.as_object() else {
        return String::new();
    };
    let text = |k: &str| o.get(k).and_then(Value::as_str);
    let mut parts = vec![format!(
        "FREQ={}",
        text("frequency").unwrap_or("yearly").to_uppercase()
    )];
    if let Some(i) = o
        .get("interval")
        .and_then(Value::as_u64)
        .filter(|i| *i != 1)
    {
        parts.push(format!("INTERVAL={i}"));
    }
    if let Some(r) = text("rscale") {
        parts.push(format!("RSCALE={}", r.to_uppercase()));
    }
    if let Some(s) = text("skip") {
        parts.push(format!("SKIP={}", s.to_uppercase()));
    }
    if let Some(w) = text("firstDayOfWeek") {
        parts.push(format!("WKST={}", w.to_uppercase()));
    }
    if let Some(days) = o.get("byDay").and_then(Value::as_array) {
        let days: Vec<String> = days
            .iter()
            .filter_map(|d| {
                let day = d.get("day")?.as_str()?.to_uppercase();
                let nth = d
                    .get("nthOfPeriod")
                    .and_then(Value::as_i64)
                    .map(|n| n.to_string());
                Some(format!("{}{day}", nth.unwrap_or_default()))
            })
            .collect();
        if !days.is_empty() {
            parts.push(format!("BYDAY={}", days.join(",")));
        }
    }
    for (key, name) in [
        ("byMonthDay", "BYMONTHDAY"),
        ("byMonth", "BYMONTH"),
        ("byYearDay", "BYYEARDAY"),
        ("byWeekNo", "BYWEEKNO"),
        ("byHour", "BYHOUR"),
        ("byMinute", "BYMINUTE"),
        ("bySecond", "BYSECOND"),
        ("bySetPosition", "BYSETPOS"),
    ] {
        list(o, key, name, &mut parts);
    }
    if let Some(n) = o.get("count").and_then(Value::as_u64) {
        parts.push(format!("COUNT={n}"));
    }
    if let Some(u) = text("until") {
        parts.push(format!("UNTIL={}", u.replace(['-', ':'], "")));
    }
    parts.join(";")
}
