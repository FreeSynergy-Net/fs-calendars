//! `SlowAppointments`: any provider, every call delayed (Z2: the grid shows its loading state).
//!
//! Pattern: Decorator over `Appointments`; the delay is awaited with `tokio::time::sleep`, so a test can use
//! paused time.

use std::time::Duration;

use async_trait::async_trait;

use crate::draft::{
    Answer, Appointment, AppointmentDraft, AppointmentPatch, AppointmentQuery, AppointmentSummary,
    Appointments, Busy, CalendarInfo, Capabilities, Change, Deleted, HistoryEntry, Invitee,
    ObjectId, Page, PersonRef, Place, Result, Revision, SeriesScope, Utc, When, Window, Zone,
};

/// `inner` with `delay` before every answer.
#[derive(Debug)]
pub struct SlowAppointments<A> {
    inner: A,
    delay: Duration,
}

impl<A: Appointments> SlowAppointments<A> {
    /// Wrap `inner`; the plan's Z2 uses three seconds.
    #[must_use]
    pub fn new(inner: A, delay: Duration) -> Self {
        Self { inner, delay }
    }

    async fn wait(&self) {
        tokio::time::sleep(self.delay).await;
    }
}

#[async_trait]
impl<A: Appointments> Appointments for SlowAppointments<A> {
    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }
    async fn calendars(&self) -> Result<Vec<CalendarInfo>> {
        self.wait().await;
        self.inner.calendars().await
    }
    async fn create_calendar(
        &self,
        name: &str,
        color: Option<&str>,
        zone: Option<Zone>,
    ) -> Result<CalendarInfo> {
        self.wait().await;
        self.inner.create_calendar(name, color, zone).await
    }
    async fn get(&self, id: ObjectId) -> Result<Option<Appointment>> {
        self.wait().await;
        self.inner.get(id).await
    }
    async fn get_by_uid(&self, uid: &str) -> Result<Option<Appointment>> {
        self.wait().await;
        self.inner.get_by_uid(uid).await
    }
    async fn list(
        &self,
        q: &AppointmentQuery,
        after: Option<&Place>,
        size: usize,
    ) -> Result<Page<AppointmentSummary>> {
        self.wait().await;
        self.inner.list(q, after, size).await
    }
    async fn window(
        &self,
        from: Utc,
        to: Utc,
        view_zone: &Zone,
        calendars: &[ObjectId],
    ) -> Result<Window> {
        self.wait().await;
        self.inner.window(from, to, view_zone, calendars).await
    }
    async fn search(
        &self,
        text: &str,
        after: Option<&Place>,
        size: usize,
    ) -> Result<Page<AppointmentSummary>> {
        self.wait().await;
        self.inner.search(text, after, size).await
    }
    async fn create(&self, draft: AppointmentDraft) -> Result<Appointment> {
        self.wait().await;
        self.inner.create(draft).await
    }
    async fn update(
        &self,
        id: ObjectId,
        expected: Revision,
        change: AppointmentPatch,
        scope: SeriesScope,
    ) -> Result<Appointment> {
        self.wait().await;
        self.inner.update(id, expected, change, scope).await
    }
    async fn delete(
        &self,
        id: ObjectId,
        expected: Revision,
        scope: SeriesScope,
    ) -> Result<Deleted> {
        self.wait().await;
        self.inner.delete(id, expected, scope).await
    }
    async fn restore(&self, id: ObjectId) -> Result<Appointment> {
        self.wait().await;
        self.inner.restore(id).await
    }
    async fn invite(
        &self,
        id: ObjectId,
        expected: Revision,
        invitees: &[Invitee],
        scope: SeriesScope,
    ) -> Result<Appointment> {
        self.wait().await;
        self.inner.invite(id, expected, invitees, scope).await
    }
    async fn respond(
        &self,
        id: ObjectId,
        who: &PersonRef,
        answer: Answer,
        comment: Option<&str>,
        proposal: Option<When>,
        scope: SeriesScope,
    ) -> Result<Appointment> {
        self.wait().await;
        self.inner
            .respond(id, who, answer, comment, proposal, scope)
            .await
    }
    async fn free_busy(&self, people: &[PersonRef], from: Utc, to: Utc) -> Result<Vec<Busy>> {
        self.wait().await;
        self.inner.free_busy(people, from, to).await
    }
    async fn history(
        &self,
        id: ObjectId,
        after: Option<&Place>,
        size: usize,
    ) -> Result<Page<HistoryEntry>> {
        self.wait().await;
        self.inner.history(id, after, size).await
    }
    async fn changes(&self, since: Option<&Place>, size: usize) -> Result<Page<Change>> {
        self.wait().await;
        self.inner.changes(since, size).await
    }
}
