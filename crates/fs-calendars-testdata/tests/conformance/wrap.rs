//! A provider that forwards everything to a `FakeAppointments` and breaks ONE rule on purpose (a mutant), to show
//! that the conformance rules notice it.

use std::sync::Arc;

use async_trait::async_trait;
use fs_calendars_testdata::FakeAppointments;
use fs_calendars_testdata::draft::{
    Answer, Appointment, AppointmentDraft, AppointmentPatch, AppointmentQuery, AppointmentSummary,
    Appointments, Attendee, Availability, CalendarDeleted, CalendarInfo, CalendarPatch,
    Capabilities, Change, Deleted, Error, FreeBusyAnswer, HistoryEntry, Invitee, ObjectId, Page,
    PersonRef, Place, Reference, Refresh, Refreshed, Restorable, Result, Revision, SeriesScope,
    Trash, Utc, When, Window, Zone,
};

/// The rule that is broken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mutant {
    /// Nothing is broken: a plain forward.
    None,
    /// `purge` deletes what is kept (the retention does not count).
    PurgeIgnoresHold,
    /// `purge` answers `Ok` and deletes nothing.
    PurgeDoesNothing,
    /// A person who does not share is `Unknown` (tells nothing about a calendar, but breaks the rule).
    NotSharedAsUnknown,
    /// An unknown participant is `Busy` with nothing (free: a guess).
    UnknownGuessedFree,
    /// Writing into a calendar in the trash is `Invalid` instead of `NotFound(calendar)`.
    TrashedWriteInvalid,
    /// A subscription answers `Forbidden` instead of `ReadOnly`.
    ReadOnlyAsForbidden,
    /// The cloud-metadata address is accepted as a feed.
    FeedAcceptsMetadata,
    /// A retention with an end shows as `Manual` (the end is lost, K8).
    HeldEndLost,
    /// An AI as owner is `Invalid` instead of `Forbidden`.
    AiOwnerInvalid,
    /// `update_calendar` does not look at the revision.
    NoCompareAndSet,
    /// A hidden calendar is `Forbidden` for an AI instead of `NotFound`.
    HiddenForbidden,
    /// The feed address is the `Origin.source` of a subscribed appointment.
    FeedInOrigin,
    /// `delete_calendar` does not count the appointments it took along.
    CountLost,
}

/// The forwarding provider.
pub struct Wrap(pub Arc<FakeAppointments>, pub Mutant);

fn is(m: Mutant, want: Mutant) -> bool {
    m == want
}

#[async_trait]
impl Appointments for Wrap {
    fn capabilities(&self) -> Capabilities {
        self.0.capabilities()
    }
    async fn calendars(&self, trash: Trash) -> Result<Vec<CalendarInfo>> {
        self.0.calendars(trash).await
    }
    async fn create_calendar(
        &self,
        name: &str,
        color: Option<&str>,
        zone: Option<Zone>,
        owner: Option<PersonRef>,
    ) -> Result<CalendarInfo> {
        let r = self.0.create_calendar(name, color, zone, owner).await;
        match r {
            Err(Error::Forbidden { .. }) if is(self.1, Mutant::AiOwnerInvalid) => {
                Err(Error::Invalid(Vec::new()))
            }
            other => other,
        }
    }
    async fn update_calendar(
        &self,
        id: ObjectId,
        expected: Revision,
        change: CalendarPatch,
    ) -> Result<CalendarInfo> {
        if is(self.1, Mutant::NoCompareAndSet) {
            let all = self.0.calendars(Trash::Include).await?;
            let current = all
                .iter()
                .find(|c| c.id == id)
                .map_or(expected, |c| c.revision);
            return self.0.update_calendar(id, current, change).await;
        }
        let r = self.0.update_calendar(id, expected, change).await;
        match r {
            Err(Error::NotFound(_)) if is(self.1, Mutant::HiddenForbidden) => {
                Err(Error::Forbidden {
                    capability: "calendar.update".to_owned(),
                })
            }
            other => other,
        }
    }
    async fn delete_calendar(&self, id: ObjectId, expected: Revision) -> Result<CalendarDeleted> {
        let mut r = self.0.delete_calendar(id, expected).await;
        if let (Ok(d), true) = (&mut r, is(self.1, Mutant::CountLost)) {
            d.appointments = 0;
        }
        r
    }
    async fn restore_calendar(&self, id: ObjectId) -> Result<CalendarInfo> {
        self.0.restore_calendar(id).await
    }
    async fn subscribe_calendar(
        &self,
        name: &str,
        color: Option<&str>,
        url: &Reference,
        refresh: Refresh,
    ) -> Result<CalendarInfo> {
        if is(self.1, Mutant::FeedAcceptsMetadata) && format!("{url:?}").contains("169.254.169.254")
        {
            let safe =
                fs_calendars_testdata::zone::reference("https://kalender.example.org/safe.ics");
            return self.0.subscribe_calendar(name, color, &safe, refresh).await;
        }
        self.0.subscribe_calendar(name, color, url, refresh).await
    }
    async fn refresh_calendar(&self, id: ObjectId) -> Result<Refreshed> {
        self.0.refresh_calendar(id).await
    }
    async fn get(&self, id: ObjectId) -> Result<Option<Appointment>> {
        let mut r = self.0.get(id).await;
        if let (Ok(Some(a)), true) = (&mut r, is(self.1, Mutant::FeedInOrigin)) {
            if let Some(o) = &mut a.origin {
                o.source = fs_calendars_testdata::zone::reference(
                    "https://kalender.example.org/zzconf-feed.ics",
                );
            }
        }
        r
    }
    async fn get_by_uid(&self, uid: &str) -> Result<Option<Appointment>> {
        self.0.get_by_uid(uid).await
    }
    async fn list(
        &self,
        q: &AppointmentQuery,
        after: Option<&Place>,
        size: usize,
    ) -> Result<Page<AppointmentSummary>> {
        let mut r = self.0.list(q, after, size).await;
        if let (Ok(p), true) = (&mut r, is(self.1, Mutant::HeldEndLost)) {
            for s in &mut p.items {
                if matches!(s.trash, Some(Restorable::Held(Some(_)))) {
                    s.trash = Some(Restorable::Manual);
                }
            }
        }
        r
    }
    async fn window(
        &self,
        from: Utc,
        to: Utc,
        view_zone: &Zone,
        calendars: &[ObjectId],
    ) -> Result<Window> {
        self.0.window(from, to, view_zone, calendars).await
    }
    async fn search(
        &self,
        text: &str,
        after: Option<&Place>,
        size: usize,
    ) -> Result<Page<AppointmentSummary>> {
        self.0.search(text, after, size).await
    }
    async fn create(&self, draft: AppointmentDraft) -> Result<Appointment> {
        let r = self.0.create(draft).await;
        match r {
            Err(Error::NotFound(_)) if is(self.1, Mutant::TrashedWriteInvalid) => {
                Err(Error::Invalid(Vec::new()))
            }
            Err(Error::ReadOnly { .. }) if is(self.1, Mutant::ReadOnlyAsForbidden) => {
                Err(Error::Forbidden {
                    capability: "event.create".to_owned(),
                })
            }
            other => other,
        }
    }
    async fn update(
        &self,
        id: ObjectId,
        expected: Revision,
        change: AppointmentPatch,
        scope: SeriesScope,
    ) -> Result<Appointment> {
        let r = self.0.update(id, expected, change, scope).await;
        match r {
            Err(Error::NotFound(_)) if is(self.1, Mutant::TrashedWriteInvalid) => {
                Err(Error::Invalid(Vec::new()))
            }
            Err(Error::ReadOnly { .. }) if is(self.1, Mutant::ReadOnlyAsForbidden) => {
                Err(Error::Forbidden {
                    capability: "event.update".to_owned(),
                })
            }
            other => other,
        }
    }
    async fn delete(
        &self,
        id: ObjectId,
        expected: Revision,
        scope: SeriesScope,
    ) -> Result<Deleted> {
        self.0.delete(id, expected, scope).await
    }
    async fn restore(&self, id: ObjectId, scope: SeriesScope) -> Result<Appointment> {
        self.0.restore(id, scope).await
    }
    async fn purge(&self, id: ObjectId) -> Result<()> {
        match self.1 {
            Mutant::PurgeIgnoresHold => {
                self.0.release(id);
                self.0.purge(id).await
            }
            Mutant::PurgeDoesNothing => Ok(()),
            _ => self.0.purge(id).await,
        }
    }
    async fn invite(
        &self,
        id: ObjectId,
        expected: Revision,
        invitees: &[Invitee],
        scope: SeriesScope,
    ) -> Result<Appointment> {
        self.0.invite(id, expected, invitees, scope).await
    }
    async fn respond(
        &self,
        id: ObjectId,
        expected: Revision,
        who: &PersonRef,
        answer: Answer,
        comment: Option<&str>,
        proposal: Option<When>,
        scope: SeriesScope,
    ) -> Result<Appointment> {
        self.0
            .respond(id, expected, who, answer, comment, proposal, scope)
            .await
    }
    async fn free_busy(
        &self,
        who: &[Attendee],
        from: Utc,
        to: Utc,
        view_zone: &Zone,
    ) -> Result<Vec<Availability>> {
        let mut r = self.0.free_busy(who, from, to, view_zone).await;
        if let Ok(list) = &mut r {
            for a in list {
                match (&a.answer, self.1) {
                    (FreeBusyAnswer::NotShared, Mutant::NotSharedAsUnknown) => {
                        a.answer = FreeBusyAnswer::Unknown;
                    }
                    (FreeBusyAnswer::Unknown, Mutant::UnknownGuessedFree) => {
                        a.answer = FreeBusyAnswer::Busy(Vec::new());
                    }
                    _ => {}
                }
            }
        }
        r
    }
    async fn history(
        &self,
        id: ObjectId,
        after: Option<&Place>,
        size: usize,
    ) -> Result<Page<HistoryEntry>> {
        self.0.history(id, after, size).await
    }
    async fn changes(&self, since: Option<&Place>, size: usize) -> Result<Page<Change>> {
        self.0.changes(since, size).await
    }
}
