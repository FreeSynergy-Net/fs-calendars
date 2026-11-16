//! Creating calendars and appointments, changing, deleting and restoring (thin over [`crate::scope`]).
//!
//! Pattern: Facade over the command executor in `scope.rs`; this file only builds and checks the new thing.

use crate::check;
use crate::draft::{
    Appointment, AppointmentDraft, AppointmentPatch, CalendarInfo, Deleted, Error, HistoryKind,
    ObjectId, Problem, Result, Revision, SeriesScope, Zone,
};
use crate::fake::FakeAppointments;
use crate::patch::{apply, describe};
use crate::scope;
use crate::state::{Item, blank};
use crate::trash;

impl FakeAppointments {
    pub(crate) fn create_calendar_impl(
        &self,
        name: &str,
        color: Option<&str>,
        default_zone: Option<Zone>,
    ) -> Result<CalendarInfo> {
        if name.trim().is_empty() {
            return Err(Error::Invalid(vec![Problem {
                field: "name".to_owned(),
                reason: "must not be empty".to_owned(),
            }]));
        }
        let mut s = self.lock();
        if let Some(z) = &default_zone {
            crate::zone::resolve(z, &s.customs)?;
        }
        let info = CalendarInfo {
            id: s.fresh_id(),
            name: name.to_owned(),
            color: color.map(str::to_owned),
            default_zone,
            owner: s.viewer.clone(),
            revision: Revision(1),
        };
        s.calendars.push(info.clone());
        Ok(info)
    }

    pub(crate) fn create_impl(&self, draft: AppointmentDraft) -> Result<Appointment> {
        let mut s = self.lock();
        if !s.calendars.iter().any(|c| c.id == draft.calendar) {
            return Err(Error::NotFound(draft.calendar));
        }
        let id = s.fresh_id();
        let uid = draft.uid.unwrap_or_else(|| {
            format!(
                "urn:uuid:00000000-0000-7000-9000-{:012x}",
                crate::ids::object_number(id)
            )
        });
        if s.items.values().any(|i| i.appt.uid == uid) {
            return Err(Error::Invalid(vec![Problem {
                field: "uid".to_owned(),
                reason: "already exists".to_owned(),
            }]));
        }
        let mut a = blank(id, draft.calendar, uid, &draft.title, draft.when, s.now);
        apply(&mut a, &draft.rest);
        check::appointment(&a, &s.customs)?;
        let changes = describe(
            &blank(id, draft.calendar, a.uid.clone(), "", a.when.clone(), s.now),
            &AppointmentPatch {
                title: Some(a.title.clone()),
                ..draft.rest
            },
        );
        s.record(&a, HistoryKind::Create, changes);
        s.items.insert(
            id,
            Item {
                appt: a.clone(),
                deleted: None,
            },
        );
        Ok(a)
    }

    pub(crate) fn update_impl(
        &self,
        id: ObjectId,
        expected: Revision,
        change: &AppointmentPatch,
        scope: SeriesScope,
    ) -> Result<Appointment> {
        scope::write(
            &mut self.lock(),
            id,
            Some(expected),
            change,
            scope,
            HistoryKind::Update,
            Vec::new(),
        )
    }

    pub(crate) fn delete_impl(
        &self,
        id: ObjectId,
        expected: Revision,
        scope: SeriesScope,
    ) -> Result<Deleted> {
        trash::delete(&mut self.lock(), id, expected, scope)
    }

    pub(crate) fn restore_impl(&self, id: ObjectId, scope: SeriesScope) -> Result<Appointment> {
        trash::restore(&mut self.lock(), id, scope)
    }
}
