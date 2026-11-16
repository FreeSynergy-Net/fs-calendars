//! `LocalAppointments`: the trait impl; one line per method, the work is in `ops`, `calendar_ops`, `query` and
//! `history`.
//!
//! Pattern: Adapter (trait to the typed store). Not offered in this slice (`Unsupported`, said in `capabilities`):
//! `search`, `invite`, `respond`, `free_busy`, `changes`, `subscribe_calendar`, `refresh_calendar`; series scopes other
//! than the ones in `scope_ops`/`scope_delete`. An asking AI is not known to the local provider yet: the AI rules of
//! the trait are not enforced here (the conformance rules skip them).

use async_trait::async_trait;
use fs_calendars_testdata::draft::{
    Answer, Appointment, AppointmentDraft, AppointmentPatch, AppointmentQuery, AppointmentSummary,
    Appointments, Attendee, Availability, CAPABILITIES, CalendarDeleted, CalendarInfo,
    CalendarPatch, Capabilities, Change, Deleted, HistoryEntry, Invitee, ObjectId, Page, PersonRef,
    Place, Reference, Refresh, Refreshed, Result, Revision, SeriesScope, Trash, Utc, When, Window,
    Zone, not_possible,
};

use crate::clock::Clock;
use crate::codec::{id_value, text};
use crate::read::load;
use crate::store::Store;
use crate::{calendar_ops, calendar_trash, calendar_update, history, ops, query};

/// The local provider: the typed v5 tables of one store file; `actor` is the asking person.
pub struct LocalAppointments {
    pub(crate) store: Store,
    pub(crate) actor: PersonRef,
    pub(crate) clock: Clock,
}

/// The capabilities this slice offers.
const OFFERED: [&str; 12] = [
    "calendar.read",
    "calendar.create",
    "calendar.update",
    "calendar.delete",
    "calendar.restore",
    "event.read",
    "event.create",
    "event.update",
    "event.delete",
    "event.restore",
    "event.history",
    "trash.purge",
];

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
    async fn calendars(&self, trash: Trash) -> Result<Vec<CalendarInfo>> {
        calendar_ops::calendars(&self.store.read().await, trash, self.clock.now()).await
    }
    async fn create_calendar(
        &self,
        name: &str,
        color: Option<&str>,
        default_zone: Option<Zone>,
        owner: Option<PersonRef>,
    ) -> Result<CalendarInfo> {
        calendar_ops::create_calendar(
            self.store.begin().await?,
            &self.actor,
            self.clock.now(),
            (name, color, default_zone, owner),
        )
        .await
    }
    async fn update_calendar(
        &self,
        id: ObjectId,
        expected: Revision,
        change: CalendarPatch,
    ) -> Result<CalendarInfo> {
        calendar_update::update_calendar(
            self.store.begin().await?,
            &self.actor,
            self.clock.now(),
            id,
            expected,
            &change,
        )
        .await
    }
    async fn delete_calendar(&self, id: ObjectId, expected: Revision) -> Result<CalendarDeleted> {
        calendar_trash::delete_calendar(
            self.store.begin().await?,
            &self.actor,
            self.clock.now(),
            id,
            expected,
        )
        .await
    }
    async fn restore_calendar(&self, id: ObjectId) -> Result<CalendarInfo> {
        calendar_trash::restore_calendar(
            self.store.begin().await?,
            &self.actor,
            self.clock.now(),
            id,
        )
        .await
    }
    async fn subscribe_calendar(
        &self,
        _: &str,
        _: Option<&str>,
        _: &Reference,
        _: Refresh,
    ) -> Result<CalendarInfo> {
        not_possible("calendar.subscribe")
    }
    async fn refresh_calendar(&self, _: ObjectId) -> Result<Refreshed> {
        not_possible("calendar.refresh")
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
        query::list(&self.store.read().await, q, after, size, self.clock.now()).await
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
    async fn search(
        &self,
        _: &str,
        _: Option<&Place>,
        _: usize,
    ) -> Result<Page<AppointmentSummary>> {
        not_possible("event.search")
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
    async fn purge(&self, id: ObjectId) -> Result<()> {
        calendar_trash::purge(self.store.begin().await?, self.clock.now(), id).await
    }
    async fn invite(
        &self,
        _: ObjectId,
        _: Revision,
        _: &[Invitee],
        _: SeriesScope,
    ) -> Result<Appointment> {
        not_possible("invite.create")
    }
    async fn respond(
        &self,
        _: ObjectId,
        _: Revision,
        _: &PersonRef,
        _: Answer,
        _: Option<&str>,
        _: Option<When>,
        _: SeriesScope,
    ) -> Result<Appointment> {
        not_possible("invite.answer")
    }
    async fn free_busy(
        &self,
        _: &[Attendee],
        _: Utc,
        _: Utc,
        _: &Zone,
    ) -> Result<Vec<Availability>> {
        not_possible("event.freebusy")
    }
    async fn history(
        &self,
        id: ObjectId,
        after: Option<&Place>,
        size: usize,
    ) -> Result<Page<HistoryEntry>> {
        history::history(&self.store.read().await, id, after, size).await
    }
    async fn changes(&self, _: Option<&Place>, _: usize) -> Result<Page<Change>> {
        not_possible("event.read")
    }
}
