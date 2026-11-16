//! The operations of `LocalAppointments` that are not part of the trait: purge, period run, import, export source.
//!
//! Pattern: Adapter (second `impl` block; the trait impl stays in `provider`).

use fs_calendars_testdata::draft::{Appointment, ObjectId, Result};

use crate::codec::id_value;
use crate::envelope;
use crate::provider::LocalAppointments;
use crate::read::load;

/// What an export reads of one object.
pub struct ExportSource {
    pub appointment: Appointment,
    /// Parts this program cannot interpret (they stay in the original).
    pub unread: Vec<String>,
    /// The received file of this object, when it is kept.
    pub original: Option<(Vec<u8>, fs_calendars_testdata::draft::MediaType)>,
}

impl LocalAppointments {
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

    /// Import one object (one `uid`) in one transaction: appointment, exceptions, origin and original (the border,
    /// `Exchange`). Not part of the trait.
    ///
    /// # Errors
    /// `Invalid` when the `uid` exists, `NotFound` for the calendar, or the engine's error; nothing is written then.
    pub async fn import_event(
        &self,
        calendar: ObjectId,
        read: &crate::jscal_in::Read,
        participants: Vec<fs_calendars_testdata::draft::Participant>,
        origin: &crate::origin::NewOrigin,
    ) -> Result<crate::import_ops::Imported> {
        let c = self.store.begin().await?;
        let r = crate::import_ops::import_one(
            &c,
            &self.actor,
            self.clock.now(),
            calendar,
            read,
            participants,
            origin,
        )
        .await;
        c.finish(r).await
    }

    /// What an export needs of one object: the appointment, the parts kept as written and the received original.
    ///
    /// # Errors
    /// `NotFound` for an unknown or trashed object.
    pub async fn export_source(&self, id: ObjectId) -> Result<ExportSource> {
        let c = self.store.read().await;
        let l = load(
            &c,
            "t.id = ? AND t.master IS NULL AND o.deleted IS NULL",
            vec![id_value(id)],
        )
        .await?
        .ok_or(fs_calendars_testdata::draft::Error::NotFound(id))?;
        let unread = crate::origin::read_unread(&c, id).await?;
        let original = crate::origin::original_of(&c, id).await?;
        Ok(ExportSource {
            appointment: l.appt,
            unread,
            original,
        })
    }
}
