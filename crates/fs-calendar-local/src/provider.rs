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

use crate::calendar_tables::{ALL, PREFIX, VERSION};
use crate::clock::Clock;
use crate::codec::{id_value, text};
use crate::read::load;
use crate::store::Store;
use crate::{envelope, history, ops, query};

/// The local provider: the typed v5 tables of one store file; `actor` is the asking person.
pub struct LocalAppointments {
    store: Store,
    actor: PersonRef,
    clock: Clock,
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
        store
            .register_domain(
                PREFIX,
                VERSION,
                "calendar: collections, appointments, participants, alerts",
                &ALL,
                now,
            )
            .await?;
        Ok(Self {
            store,
            actor,
            clock,
        })
    }

    /// Log purge (ADR 0029): the log of the object goes, the object stays; not part of the trait.
    ///
    /// # Errors
    /// `NotFound` when there is no such object.
    pub async fn purge_log(&self, id: ObjectId) -> Result<()> {
        let c = self.store.begin().await?;
        let r = envelope::purge_log(&c, id).await;
        c.finish(r).await
    }

    /// Final delete of an object in the trash (ADR 0073), with everything that hangs on it; not part of the trait.
    ///
    /// # Errors
    /// `NotFound`, or `Invalid` when the object is not in the trash.
    pub async fn delete_final(&self, id: ObjectId) -> Result<()> {
        let c = self.store.begin().await?;
        let r = envelope::delete_final(&c, id).await;
        c.finish(r).await
    }

    /// The writes of this object are no longer logged (as v4 `unlogged_ids`); not part of the trait.
    ///
    /// # Errors
    /// `NotFound` when there is no such object.
    pub async fn mark_unlogged(&self, id: ObjectId) -> Result<()> {
        let c = self.store.begin().await?;
        let r = envelope::mark_unlogged(&c, id).await;
        c.finish(r).await
    }

    /// The period run (ADR 0073): forget the cuts whose restore period is over and the exceptions they hid.
    /// Not part of the trait; the program calls it from its own schedule.
    ///
    /// # Errors
    /// The engine's error.
    pub async fn sweep_expired(&self) -> Result<crate::sweep::Swept> {
        let c = self.store.begin().await?;
        let r = crate::sweep::sweep_expired(&c, self.clock.now(), &self.actor).await;
        c.finish(r).await
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
