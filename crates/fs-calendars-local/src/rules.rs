//! Recurrence rules and `related` as typed rows (`calendar_rule`, `calendar_rule_part`, `calendar_related`).
//!
//! Pattern: Adapter (rule values to rows and back); the order of a by-list is kept in `pos`.

use fs_calendars_testdata::draft::{
    ByMonth, NDay, ObjectId, Recurrence, RecurrenceRule, Related, Relation, Result, RuleEnd,
};
use turso::Value;

use crate::calendar_tables::{RELATED, RULE, RULE_ORIGINAL, RULE_PART};
use crate::codec::{
    boolean, damaged, get_bool, get_int, get_opt_int, get_opt_text, get_text, id_value, int,
    int_to_local, local_to_int, opt_int, opt_text, text,
};
pub use crate::rule_words::{DAYS, FREQ, RELATIONS, SKIPS};
use crate::store::Conn;
use crate::words::{Table as Words, from_word, word};

/// A rule with its position in the list (`seq`).
type Placed = Vec<(i64, Recurrence)>;

/// One by-part value: `(part, value, nth, leap)`.
type Part = (&'static str, i64, Option<i64>, bool);

fn parts(r: &RecurrenceRule) -> Vec<Part> {
    let mut out: Vec<Part> = Vec::new();
    out.extend(r.by_day.iter().map(|d| {
        (
            "day",
            i64::try_from(DAYS.iter().position(|(w, _)| *w == d.day).unwrap_or(0)).unwrap_or(0),
            d.nth.map(i64::from),
            false,
        )
    }));
    out.extend(
        r.by_month_day
            .iter()
            .map(|v| ("month_day", i64::from(*v), None, false)),
    );
    out.extend(
        r.by_month
            .iter()
            .map(|m| ("month", i64::from(m.month), None, m.leap)),
    );
    out.extend(
        r.by_year_day
            .iter()
            .map(|v| ("year_day", i64::from(*v), None, false)),
    );
    out.extend(
        r.by_week_no
            .iter()
            .map(|v| ("week_no", i64::from(*v), None, false)),
    );
    out.extend(
        r.by_hour
            .iter()
            .map(|v| ("hour", i64::from(*v), None, false)),
    );
    out.extend(
        r.by_minute
            .iter()
            .map(|v| ("minute", i64::from(*v), None, false)),
    );
    out.extend(
        r.by_second
            .iter()
            .map(|v| ("second", i64::from(*v), None, false)),
    );
    out.extend(
        r.by_set_position
            .iter()
            .map(|v| ("set_position", i64::from(*v), None, false)),
    );
    out
}

/// Replace the rules (`excluded` = the excluding rules) of an appointment: a typed rule in `calendar_rule`, a rule kept
/// as written (`Recurrence::Original`) in `calendar_rule_original`, both at their position `seq` of the list.
///
/// # Errors
/// `Invalid` naming `table.column`, or the engine's error.
pub async fn write_rules(
    c: &Conn<'_>,
    id: ObjectId,
    excluded: bool,
    rules: &[Recurrence],
) -> Result<()> {
    for t in ["calendar_rule", "calendar_rule_original"] {
        c.exec(
            &format!("DELETE FROM {t} WHERE appointment = ? AND excluded = ?"),
            vec![id_value(id), boolean(excluded)],
        )
        .await?;
    }
    for (seq, rule) in rules.iter().enumerate() {
        let seq = i64::try_from(seq).unwrap_or(0);
        let r = match rule {
            Recurrence::Rule(r) => r,
            Recurrence::Original(rrule) => {
                c.insert(
                    &RULE_ORIGINAL,
                    vec![id_value(id), boolean(excluded), int(seq), text(rrule)],
                )
                .await?;
                continue;
            }
        };
        let (count, until) = match r.end {
            RuleEnd::Never => (Value::Null, Value::Null),
            RuleEnd::Count(n) => (int(i64::from(n)), Value::Null),
            RuleEnd::Until(l) => (Value::Null, int(local_to_int(l)?)),
        };
        c.insert(
            &RULE,
            vec![
                id_value(id),
                boolean(excluded),
                int(seq),
                text(word(&FREQ, r.frequency)),
                int(i64::from(r.interval)),
                opt_text(r.rscale.as_deref()),
                text(word(&SKIPS, r.skip)),
                text(word(&DAYS, r.first_day_of_week)),
                count,
                until,
            ],
        )
        .await?;
        for (pos, (part, value, nth, leap)) in parts(r).into_iter().enumerate() {
            c.insert(
                &RULE_PART,
                vec![
                    id_value(id),
                    boolean(excluded),
                    int(seq),
                    text(part),
                    int(i64::try_from(pos).unwrap_or(0)),
                    int(value),
                    opt_int(nth),
                    boolean(leap),
                ],
            )
            .await?;
        }
    }
    Ok(())
}

fn day(v: i64, nth: Option<i64>) -> Result<NDay> {
    let w = DAYS
        .get(usize::try_from(v).map_err(|_| damaged("day"))?)
        .ok_or_else(|| damaged("day"))?
        .0;
    Ok(NDay {
        day: w,
        nth: nth
            .map(|n| i16::try_from(n).map_err(|_| damaged("nth")))
            .transpose()?,
    })
}

fn narrow<T: TryFrom<i64>>(v: i64) -> Result<T> {
    T::try_from(v).map_err(|_| damaged("rule value"))
}

/// The rules of an appointment: `(rules, excluded rules)`.
///
/// # Errors
/// `Unavailable` for a row that does not read back.
pub async fn read_rules(c: &Conn<'_>, id: ObjectId) -> Result<(Vec<Recurrence>, Vec<Recurrence>)> {
    let rows = c.query("SELECT excluded, seq, frequency, interval, rscale, skip, first_day_of_week, count, until FROM calendar_rule WHERE appointment = ? ORDER BY excluded, seq", vec![id_value(id)]).await?;
    let (mut keep, mut out): (Placed, Placed) = (Vec::new(), Vec::new());
    for r in &rows {
        let (ex, seq) = (get_bool(r, 0)?, get_int(r, 1)?);
        let end = match (get_opt_int(r, 7)?, get_opt_int(r, 8)?) {
            (Some(n), _) => RuleEnd::Count(narrow(n)?),
            (None, Some(u)) => RuleEnd::Until(int_to_local(u)?),
            _ => RuleEnd::Never,
        };
        let mut rule = RecurrenceRule {
            frequency: from_word(&FREQ, &get_text(r, 2)?)?,
            interval: narrow(get_int(r, 3)?)?,
            rscale: get_opt_text(r, 4)?,
            skip: from_word(&SKIPS, &get_text(r, 5)?)?,
            first_day_of_week: from_word(&DAYS, &get_text(r, 6)?)?,
            by_day: vec![],
            by_month_day: vec![],
            by_month: vec![],
            by_year_day: vec![],
            by_week_no: vec![],
            by_hour: vec![],
            by_minute: vec![],
            by_second: vec![],
            by_set_position: vec![],
            end,
        };
        let ps = c.query("SELECT part, value, nth, leap FROM calendar_rule_part WHERE appointment = ? AND excluded = ? AND seq = ? ORDER BY pos", vec![id_value(id), boolean(ex), int(seq)]).await?;
        for p in &ps {
            let (v, nth) = (get_int(p, 1)?, get_opt_int(p, 2)?);
            match get_text(p, 0)?.as_str() {
                "day" => rule.by_day.push(day(v, nth)?),
                "month_day" => rule.by_month_day.push(narrow(v)?),
                "month" => rule.by_month.push(ByMonth {
                    month: narrow(v)?,
                    leap: get_bool(p, 3)?,
                }),
                "year_day" => rule.by_year_day.push(narrow(v)?),
                "week_no" => rule.by_week_no.push(narrow(v)?),
                "hour" => rule.by_hour.push(narrow(v)?),
                "minute" => rule.by_minute.push(narrow(v)?),
                "second" => rule.by_second.push(narrow(v)?),
                "set_position" => rule.by_set_position.push(narrow(v)?),
                _ => return Err(damaged("rule part")),
            }
        }
        let rule = Recurrence::Rule(rule);
        if ex {
            out.push((seq, rule));
        } else {
            keep.push((seq, rule));
        }
    }
    let kept = c
        .query(
            "SELECT excluded, seq, rrule FROM calendar_rule_original WHERE appointment = ?",
            vec![id_value(id)],
        )
        .await?;
    for r in &kept {
        let (ex, seq) = (get_bool(r, 0)?, get_int(r, 1)?);
        let rule = Recurrence::Original(get_text(r, 2)?);
        if ex {
            out.push((seq, rule));
        } else {
            keep.push((seq, rule));
        }
    }
    keep.sort_by_key(|(seq, _)| *seq);
    out.sort_by_key(|(seq, _)| *seq);
    let strip = |v: Placed| v.into_iter().map(|(_, r)| r).collect();
    Ok((strip(keep), strip(out)))
}

/// Replace the `related` rows.
///
/// # Errors
/// `Invalid` or the engine's error.
pub async fn write_related(c: &Conn<'_>, id: ObjectId, related: &[Related]) -> Result<()> {
    c.exec(
        "DELETE FROM calendar_related WHERE appointment = ?",
        vec![id_value(id)],
    )
    .await?;
    let mut seen = std::collections::BTreeSet::new();
    for r in related {
        if seen.insert((r.uid.clone(), word(&RELATIONS, r.relation))) {
            c.insert(
                &RELATED,
                vec![
                    id_value(id),
                    text(&r.uid),
                    text(word(&RELATIONS, r.relation)),
                ],
            )
            .await?;
        }
    }
    Ok(())
}

/// The `related` rows, by uid.
///
/// # Errors
/// `Unavailable` for a row that does not read back.
pub async fn read_related(c: &Conn<'_>, id: ObjectId) -> Result<Vec<Related>> {
    let rows = c.query("SELECT uid, relation FROM calendar_related WHERE appointment = ? ORDER BY uid, relation", vec![id_value(id)]).await?;
    rows.iter()
        .map(|r| {
            Ok(Related {
                uid: get_text(r, 0)?,
                relation: from_word(&RELATIONS as &Words<Relation>, &get_text(r, 1)?)?,
            })
        })
        .collect()
}
