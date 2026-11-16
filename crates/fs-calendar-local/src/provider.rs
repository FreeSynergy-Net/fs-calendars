//! `LocalAppointments`: the trait impl; one line per method, the work is in `ops`, `query` and `history`.
//!
//! Pattern: Adapter (trait to the typed store). Not offered in this slice (default `Unsupported`): `search`,
//! `invite`, `respond`, `free_busy`, `changes`; series scopes other than `Whole`.

use async_trait::async_trait;
use fs_calendar_testdata::draft::{
    Appointment, AppointmentDraft, AppointmentPatch, AppointmentQuery, AppointmentSummary,
    Appointments, CAPABILITIES, CalendarInfo, Capabilities, Deleted, HistoryEntry, ObjectId, Page,
    PersonRef, Place, Result, Revision, SeriesScope, Utc, Window, Zone,
};

use crate::clock::Clock;
use crate::codec::{id_value, text};
use crate::read::load;
use crate::store::Store;
use crate::{history, ops, query};

/// The local provider: the typed v5 tables of one store file; `actor` is the asking person.
pub struct LocalAppointments {
    pub(crate) store: Store,
    pub(crate) actor: PersonRef,
    pub(crate) clock: Clock,
}

/// The capabilities this slice offers.
const OFFERED: [&str; 7] = [
    "calendar.read",
    "calendar.create",
    "event.create",
    "event.update",
    "event.delete",
    "event.restore",
    "event.history",
];

impl LocalAppointments {
    /// Open the store at `path` (`:memory:` is volatile) and register the `calendar_` domain.
    ///
    /// # Errors
    /// `Unavailable` when the engine cannot open the file; `Invalid` on a clash in the registry.
    pub async fn open(path: &str, actor: PersonRef, clock: Clock) -> Result<Self> {
        let now = clock.now();
        let store = Store::open(path, now).await?;
        Ok(Self {
            store,
            actor,
            clock,
        })
    }

    /// Whether an appointment with this `uid` exists (a trashed one too: `uid` is unique).
    ///
    /// # Errors
    /// The engine's error.
    pub async fn uid_exists(&self, uid: &str) -> Result<bool> {
        let c = self.store.read().await;
        let rows = c
            .query(
                "SELECT 1 FROM calendar_appointment WHERE uid = ? AND master IS NULL",
                vec![text(uid)],
            )
            .await?;
        Ok(!rows.is_empty())
    }

    /// The acting person.
    #[must_use]
    pub fn actor(&self) -> &PersonRef {
        &self.actor
    }

    /// The store (the tests look at the rows).
    #[must_use]
    pub fn store(&self) -> &Store {
        &self.store
    }
}

#[async_trait]
impl Appointments for LocalAppointments {
    fn capabilities(&self) -> Capabilities {
        Capabilities(
            CAPABILITIES
                .iter()
                .map(|c| ((*c).to_owned(), OFFERED.contains(c)))
                .collect(),
        )
    }
    async fn calendars(&self) -> Result<Vec<CalendarInfo>> {
        ops::calendars(&self.store.read().await).await
    }
    async fn create_calendar(
        &self,
        name: &str,
        color: Option<&str>,
        default_zone: Option<Zone>,
    ) -> Result<CalendarInfo> {
        ops::create_calendar(
            self.store.begin().await?,
            &self.actor,
            self.clock.now(),
            name,
            color,
            default_zone,
        )
        .await
    }
    async fn get(&self, id: ObjectId) -> Result<Option<Appointment>> {
        let c = self.store.read().await;
        Ok(load(
            &c,
            "t.id = ? AND t.master IS NULL AND o.deleted IS NULL",
            vec![id_value(id)],
        )
        .await?
        .map(|l| l.appt))
    }
    async fn get_by_uid(&self, uid: &str) -> Result<Option<Appointment>> {
        let c = self.store.read().await;
        Ok(load(
            &c,
            "t.uid = ? AND t.master IS NULL AND o.deleted IS NULL",
            vec![text(uid)],
        )
        .await?
        .map(|l| l.appt))
    }
    async fn list(
        &self,
        q: &AppointmentQuery,
        after: Option<&Place>,
        size: usize,
    ) -> Result<Page<AppointmentSummary>> {
        query::list(&self.store.read().await, q, after, size).await
    }
    async fn window(
        &self,
        from: Utc,
        to: Utc,
        view_zone: &Zone,
        calendars: &[ObjectId],
    ) -> Result<Window> {
        query::window(
            &self.store.read().await,
            from,
            to,
            view_zone,
            calendars,
            &self.actor,
        )
        .await
    }
    async fn create(&self, draft: AppointmentDraft) -> Result<Appointment> {
        ops::create(
            self.store.begin().await?,
            &self.actor,
            self.clock.now(),
            draft,
        )
        .await
    }
    async fn update(
        &self,
        id: ObjectId,
        expected: Revision,
        change: AppointmentPatch,
        scope: SeriesScope,
    ) -> Result<Appointment> {
        ops::update(
            self.store.begin().await?,
            &self.actor,
            self.clock.now(),
            id,
            expected,
            change,
            scope,
        )
        .await
    }
    async fn delete(
        &self,
        id: ObjectId,
        expected: Revision,
        scope: SeriesScope,
    ) -> Result<Deleted> {
        ops::delete(
            self.store.begin().await?,
            &self.actor,
            self.clock.now(),
            id,
            expected,
            scope,
        )
        .await
    }
    async fn restore(&self, id: ObjectId, scope: SeriesScope) -> Result<Appointment> {
        ops::restore(
            self.store.begin().await?,
            &self.actor,
            self.clock.now(),
            id,
            scope,
        )
        .await
    }
    async fn history(
        &self,
        id: ObjectId,
        after: Option<&Place>,
        size: usize,
    ) -> Result<Page<HistoryEntry>> {
        history::history(&self.store.read().await, id, after, size).await
    }
}
