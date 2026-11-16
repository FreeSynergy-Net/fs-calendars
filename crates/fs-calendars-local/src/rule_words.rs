//! The words of the rule tables (frequency, skip, weekday, relation) as stored.
//!
//! Pattern: Value Object (const word tables; the type is `words::Table`).

use fs_calendars_testdata::draft::{Frequency, Relation, Skip, Weekday};

pub const FREQ: [(Frequency, &str); 7] = [
    (Frequency::Yearly, "yearly"),
    (Frequency::Monthly, "monthly"),
    (Frequency::Weekly, "weekly"),
    (Frequency::Daily, "daily"),
    (Frequency::Hourly, "hourly"),
    (Frequency::Minutely, "minutely"),
    (Frequency::Secondly, "secondly"),
];
pub const SKIPS: [(Skip, &str); 3] = [
    (Skip::Omit, "omit"),
    (Skip::Backward, "backward"),
    (Skip::Forward, "forward"),
];
pub const DAYS: [(Weekday, &str); 7] = [
    (Weekday::Mo, "mo"),
    (Weekday::Tu, "tu"),
    (Weekday::We, "we"),
    (Weekday::Th, "th"),
    (Weekday::Fr, "fr"),
    (Weekday::Sa, "sa"),
    (Weekday::Su, "su"),
];
pub const RELATIONS: [(Relation, &str); 4] = [
    (Relation::First, "first"),
    (Relation::Next, "next"),
    (Relation::Child, "child"),
    (Relation::Parent, "parent"),
];
