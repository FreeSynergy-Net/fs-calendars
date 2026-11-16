//! The stored words of the trait's enums (RFC 8984 names, AS2 names for the log).
//!
//! Pattern: Value Object (const lookup tables); two generic functions read them in both directions.

use fs_calendars_testdata::draft::{
    AlertAction, FreeBusy, ParticipationStatus, Privacy, RelativeTo, Result, Status, TextType,
};

use crate::codec::damaged;

pub type Table<T> = [(T, &'static str)];

/// The word of a value.
pub fn word<T: Copy + PartialEq>(table: &Table<T>, v: T) -> &'static str {
    table.iter().find(|(x, _)| *x == v).map_or("", |(_, w)| w)
}

/// The value of a word; a word that is not in the table is a damaged row.
pub fn from_word<T: Copy>(table: &Table<T>, w: &str) -> Result<T> {
    table
        .iter()
        .find(|(_, x)| *x == w)
        .map(|(v, _)| *v)
        .ok_or_else(|| damaged(w))
}

pub const STATUS: [(Status, &str); 3] = [
    (Status::Confirmed, "confirmed"),
    (Status::Tentative, "tentative"),
    (Status::Cancelled, "cancelled"),
];
pub const FREE_BUSY: [(FreeBusy, &str); 2] = [(FreeBusy::Busy, "busy"), (FreeBusy::Free, "free")];
pub const PRIVACY: [(Privacy, &str); 3] = [
    (Privacy::Public, "public"),
    (Privacy::Private, "private"),
    (Privacy::Secret, "secret"),
];
pub const TEXT_TYPE: [(TextType, &str); 2] = [(TextType::Plain, "plain"), (TextType::Html, "html")];
pub const PARTICIPATION: [(ParticipationStatus, &str); 5] = [
    (ParticipationStatus::NeedsAction, "needs-action"),
    (ParticipationStatus::Accepted, "accepted"),
    (ParticipationStatus::Declined, "declined"),
    (ParticipationStatus::Tentative, "tentative"),
    (ParticipationStatus::Delegated, "delegated"),
];
pub const RELATIVE_TO: [(RelativeTo, &str); 2] =
    [(RelativeTo::Start, "start"), (RelativeTo::End, "end")];
pub const ALERT_ACTION: [(AlertAction, &str); 2] = [
    (AlertAction::Display, "display"),
    (AlertAction::Email, "email"),
];
