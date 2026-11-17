//! The search term of the standard `find` command: a case-insensitive look in title and description.
//!
//! Moved here verbatim from `fs-calendars-app/src/backend/find.rs` (`matches`, sorting run S2b, not compiled).
//!
//! Pattern: Specification — a pure predicate over one event.

use super::Event;

/// Whether `e` matches `term`; no term (or an empty one) matches every event.
pub fn matches(term: Option<&str>, e: &Event) -> bool {
    let Some(t) = term.map(str::to_lowercase).filter(|t| !t.is_empty()) else {
        return true;
    };
    [&e.title, &e.description]
        .iter()
        .any(|f| f.as_deref().is_some_and(|s| s.to_lowercase().contains(&t)))
}
