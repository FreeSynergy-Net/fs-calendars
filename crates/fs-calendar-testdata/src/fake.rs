//! `FakeAppointments`: the in-memory provider every other part is tested against first (plan step K0/F0).
//!
//! Pattern: Fake (test double with working behaviour) behind the `Appointments` Repository trait. It answers
//! every method except `free_busy` (`Unsupported`, the one expected `event.freebusy`), keeps a log and a change
//! feed, and has a trash. Where the draft trait is silent the fake decides and says so in `DECISIONS`.

use std::sync::{Mutex, MutexGuard};

use crate::draft::{Appointment, ObjectId, PersonRef, Utc, Zone};
use crate::state::{Item, State};

/// What the fake decided where the draft trait does not say (findings for the trait review).
pub const DECISIONS: [&str; 8] = [
    "get/get_by_uid do not return a trashed appointment (None); list with include_deleted does.",
    "All-day and floating appointments are placed in the view zone passed to window; list, search and sorting use UTC as the view zone.",
    "A custom zone ('/...') is a fixed offset registered with with_custom_zone; its rules are not interpreted.",
    "delete with This or ThisAndFollowing returns restorable_until = None: the trait has no way to undo a partial delete.",
    "Only daily, weekly, monthly and yearly rules (by_day, by_month_day, by_month, by_set_position, count, until) are expanded; others are Invalid.",
    "respond has no `expected`: it writes against the current revision.",
    "The acting person (log actor, my_status) is a property of the provider (`viewer`), not an argument.",
    "ThisAndFollowing on the first occurrence acts as Whole; otherwise the series splits (new uid, First/Next relations).",
];

/// In-memory `Appointments`.
#[derive(Debug)]
pub struct FakeAppointments {
    state: Mutex<State>,
}

/// A fake with no appointments (kept as a name for the plan's `EmptyAppointments`; Z1).
pub type EmptyAppointments = FakeAppointments;

impl FakeAppointments {
    /// Two calendars ("Persönlich", "Arbeit"), no appointments.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State::new()),
        }
    }

    /// Same as [`FakeAppointments::new`]: the empty case of the plan (Z1).
    #[must_use]
    pub fn empty() -> Self {
        Self::new()
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Register a custom zone as a fixed offset (seconds east of UTC).
    #[must_use]
    pub fn with_custom_zone(self, name: &str, offset_seconds: i32) -> Self {
        self.lock().customs.insert(name.to_owned(), offset_seconds);
        self
    }

    /// The person who acts and asks.
    #[must_use]
    pub fn with_viewer(self, viewer: PersonRef) -> Self {
        self.lock().viewer = viewer;
        self
    }

    /// Put an appointment in as it is (no checks, no log): for loading the sample.
    pub fn insert_raw(&self, appt: Appointment) {
        let mut s = self.lock();
        s.next = s.next.max(crate::ids::object_number(appt.id));
        s.items.insert(
            appt.id,
            Item {
                appt,
                deleted: None,
            },
        );
    }

    /// Replace the calendars (loading the sample).
    pub fn set_calendars(&self, calendars: Vec<crate::draft::CalendarInfo>) {
        self.lock().calendars = calendars;
    }

    /// Set "now" (UTC ms): creation time, log time, trash deadline.
    pub fn set_now(&self, now: Utc) {
        self.lock().now = now;
    }

    /// Let a window report this appointment as damaged (Z3) instead of its instances.
    pub fn mark_damaged(&self, id: ObjectId, reason: &str) {
        self.lock().damaged.insert(id, reason.to_owned());
    }

    /// The zone a test names, for readability.
    #[must_use]
    pub fn iana(name: &str) -> Zone {
        Zone::Iana(name.to_owned())
    }
}

impl Default for FakeAppointments {
    fn default() -> Self {
        Self::new()
    }
}
