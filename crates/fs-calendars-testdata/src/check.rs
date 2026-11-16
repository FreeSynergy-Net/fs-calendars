//! Checks before anything is stored: nothing is written when one fails (`Invalid`, `UnknownZone`).
//!
//! Pattern: none (pure validation, collects all problems of one appointment).

use crate::draft::{Appointment, Error, Problem, Result, When};
use crate::recur::check as check_rule;
use crate::zone::{CustomZones, naive, naive_date, resolve};

fn problem(field: &str, reason: &str) -> Problem {
    Problem {
        field: field.to_owned(),
        reason: reason.to_owned(),
    }
}

/// Check one appointment as it would be stored.
///
/// # Errors
/// `UnknownZone` for a zone that is neither IANA nor registered (never guessed); `Invalid` with every problem found.
pub fn appointment(a: &Appointment, customs: &CustomZones) -> Result<()> {
    let mut problems = Vec::new();
    if a.title.trim().is_empty() {
        problems.push(problem("title", "must not be empty"));
    }
    match &a.when {
        When::Timed {
            start,
            zone,
            end_zone,
            ..
        } => {
            resolve(zone, customs)?;
            if let Some(z) = end_zone {
                resolve(z, customs)?;
            }
            if naive(*start).is_none() {
                problems.push(problem("when.start", "no such date or time"));
            }
        }
        When::Floating { start, .. } => {
            if naive(*start).is_none() {
                problems.push(problem("when.start", "no such date or time"));
            }
        }
        When::AllDay { first, days } => {
            if naive_date(*first).is_none() {
                problems.push(problem("when.first", "no such date"));
            }
            if *days == 0 {
                problems.push(problem(
                    "when.days",
                    "an all-day appointment lasts at least one day",
                ));
            }
        }
    }
    for r in a.recurrence.iter().chain(&a.excluded_recurrence) {
        problems.extend(check_rule(r));
    }
    if a.participants.iter().any(|p| p.key.is_empty()) {
        problems.push(problem("participants.key", "must not be empty"));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(Error::Invalid(problems))
    }
}
