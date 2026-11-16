//! The in-memory state behind [`crate::FakeAppointments`].
//!
//! Pattern: Repository state (one value behind one lock); plain data, no behaviour but small helpers.

use std::collections::{BTreeMap, HashMap};

use crate::draft::{
    Appointment, AppointmentPatch, CalendarInfo, Change, Error, FieldChange, FreeBusy,
    HistoryEntry, HistoryKind, LocalDateTime, ObjectId, Override, PersonRef, Privacy,
    RecurrenceRule, Result, Revision, Status, TextType, Utc, When,
};
use crate::ids::{object_id, person};
use crate::zone::CustomZones;

/// A stored appointment and whether it is in the trash.
#[derive(Clone, Debug)]
pub struct Item {
    /// The master appointment (a series is one item).
    pub appt: Appointment,
    /// Set when deleted as a whole.
    pub deleted: Option<Utc>,
}

/// What a partial delete (`This`, `ThisAndFollowing`) took away, so `restore` can give it back.
#[derive(Clone, Debug)]
pub struct Cut {
    pub id: ObjectId,
    pub recurrence_id: LocalDateTime,
    /// Whether the cut was `ThisAndFollowing` (else `This`).
    pub following: bool,
    pub until: Utc,
    /// The rules before the cut (`ThisAndFollowing` only).
    pub recurrence: Vec<RecurrenceRule>,
    /// The overrides the cut removed (`ThisAndFollowing` only).
    pub overrides: Vec<Override>,
}

/// Everything the fake knows.
#[derive(Debug)]
pub struct State {
    pub calendars: Vec<CalendarInfo>,
    pub items: BTreeMap<ObjectId, Item>,
    pub next: u64,
    pub now: Utc,
    pub history: Vec<(ObjectId, HistoryEntry)>,
    pub changes: Vec<Change>,
    pub customs: CustomZones,
    pub damaged: HashMap<ObjectId, String>,
    /// Partial deletes that `restore` can still undo.
    pub cuts: Vec<Cut>,
    /// The person who asks and who acts (`actor` in the log, `my_status` in instances).
    pub viewer: PersonRef,
}

/// Milliseconds the trash keeps a deleted appointment (30 days, like Google).
pub const TRASH_MS: i64 = 30 * 24 * 3600 * 1000;

impl State {
    /// A state with two calendars and no appointments; "now" is 2026-11-16T08:00:00Z.
    #[must_use]
    pub fn new() -> Self {
        let owner = person(1);
        let cal = |n: u64, name: &str| CalendarInfo {
            id: object_id(n),
            name: name.to_owned(),
            color: None,
            default_zone: None,
            owner: owner.clone(),
            revision: Revision(1),
        };
        Self {
            calendars: vec![cal(1, "Persönlich"), cal(2, "Arbeit")],
            items: BTreeMap::new(),
            next: 100,
            now: Utc::from_millis(1_794_816_000_000),
            history: Vec::new(),
            changes: Vec::new(),
            customs: CustomZones::new(),
            damaged: HashMap::new(),
            cuts: Vec::new(),
            viewer: owner,
        }
    }

    /// The next free object id.
    pub fn fresh_id(&mut self) -> ObjectId {
        self.next += 1;
        object_id(self.next)
    }

    /// A live (not trashed) item, or `NotFound`.
    ///
    /// # Errors
    /// `NotFound` for an unknown or trashed id.
    pub fn live(&self, id: ObjectId) -> Result<&Item> {
        self.items
            .get(&id)
            .filter(|i| i.deleted.is_none())
            .ok_or(Error::NotFound(id))
    }

    /// Write one entry to the log and one to the change feed.
    pub fn record(&mut self, a: &Appointment, kind: HistoryKind, changes: Vec<FieldChange>) {
        let (at, actor) = (self.now, self.viewer.clone());
        self.history.push((
            a.id,
            HistoryEntry {
                at,
                actor,
                kind,
                changes,
            },
        ));
        self.changes.push(Change {
            appointment: a.id,
            calendar: a.calendar,
            kind,
            at,
        });
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

/// A new appointment with the plain defaults (confirmed, busy, public, no alerts).
#[must_use]
pub fn blank(
    id: ObjectId,
    calendar: ObjectId,
    uid: String,
    title: &str,
    when: When,
    now: Utc,
) -> Appointment {
    Appointment {
        id,
        calendar,
        uid,
        revision: Revision(1),
        sequence: 0,
        created: now,
        updated: now,
        title: title.to_owned(),
        description: String::new(),
        description_type: TextType::Plain,
        locale: None,
        when,
        status: Status::Confirmed,
        free_busy: FreeBusy::Busy,
        privacy: Privacy::Public,
        priority: 0,
        color: None,
        keywords: Vec::new(),
        categories: Vec::new(),
        locations: Vec::new(),
        virtual_locations: Vec::new(),
        links: Vec::new(),
        participants: Vec::new(),
        reply_to: Vec::new(),
        alerts: Vec::new(),
        use_default_alerts: false,
        recurrence: Vec::new(),
        excluded_recurrence: Vec::new(),
        overrides: Vec::new(),
        related: Vec::new(),
        localizations: Vec::new(),
        origin: None,
        hide_attendees: false,
        may_invite_self: false,
        may_invite_others: false,
    }
}

/// An empty patch (nothing changes).
#[must_use]
pub fn no_change() -> AppointmentPatch {
    AppointmentPatch::default()
}
