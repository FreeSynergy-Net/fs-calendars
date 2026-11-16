//! The reading half of the exchange: people through the directory, one transaction per `uid`, the report.
//!
//! Pattern: Adapter (second `impl` block of `LocalExchange`).

use fs_calendars_testdata::draft::{
    Attendee, BasicValue, ContactAddress, Error, FileRef, ImportContext, ImportReport, Loss,
    MediaType, ObjectId, OriginChannel, Participant, Reference, Result, Roles,
};
use fs_calendars_testdata::people::{Directory, ProposeEntry};
use serde_json::Value;

use crate::exchange::{ICS, LocalExchange};
use crate::jscal_in::{RawPerson, Read, read_event};
use crate::origin::NewOrigin;

impl LocalExchange {
    pub(crate) async fn participants(
        &self,
        raw: &[RawPerson],
        uid: &str,
        ics: bool,
        losses: &mut Vec<Loss>,
        batch: &mut Batch<'_>,
    ) -> Result<Vec<Participant>> {
        let mut out = Vec::new();
        for p in raw {
            let Some(addr) = &p.address else {
                losses.push(Loss {
                    uid: Some(uid.to_owned()),
                    field: format!("participants/{}", p.key),
                    reason: "no address: a person is never a free name, left out".into(),
                });
                continue;
            };
            let who = match p.kind.as_str() {
                "resource" | "location" => {
                    // An address that is no absolute IRI costs only this person, never the appointment.
                    let Ok(r) = Reference::parse(addr) else {
                        losses.push(Loss {
                            uid: Some(uid.to_owned()),
                            field: format!("participants/{}", p.key),
                            reason: "address.unsupported".into(),
                        });
                        continue;
                    };
                    if p.kind == "resource" {
                        Attendee::Resource(r)
                    } else {
                        Attendee::Location(r)
                    }
                }
                _ => {
                    let Ok(address) = ContactAddress::parse(addr) else {
                        losses.push(Loss {
                            uid: Some(uid.to_owned()),
                            field: format!("participants/{}", p.key),
                            reason: "address.unsupported".into(),
                        });
                        continue;
                    };
                    // The first hit of the directory is the person; an unknown address is a proposal in ONE batch of
                    // this import (origin = the file). A failure of the directory (also `Forbidden` for an AI)
                    // aborts the whole import: never silently an unknown address, never an import without people.
                    // Known limit: if the directory fails only AFTER the first proposals, the batch they opened stays
                    // in the directory (the `Directory` trait has no way to discard a batch); the proposals are only
                    // "proposed", nobody confirmed them, and the people can reject the batch there.
                    let found = self.people.find_by_address(&address).await?;
                    Attendee::Actor(if let Some(r) = found.into_iter().next() {
                        r
                    } else {
                        let entry = ProposeEntry {
                            display_hint: p.name.clone().unwrap_or_else(|| addr.clone()),
                            address: Some(address),
                            origin: Some(batch.origin.clone()),
                        };
                        let id = batch.open(self.people.as_ref()).await?;
                        self.people.propose(&entry, Some(id)).await?
                    })
                }
            };
            // RFC 5545: an ATTENDEE without ROLE is a required participant; the iCalendar writer omits that default.
            let roles = if ics && p.roles == Roles::default() {
                Roles {
                    attendee: true,
                    ..Roles::default()
                }
            } else {
                p.roles
            };
            out.push(Participant {
                key: p.key.clone(),
                who,
                roles,
                status: p.status,
                expect_reply: p.expect_reply,
                comment: None,
                send_to: Vec::new(),
                invited_by: None,
                delegated_to: Vec::new(),
                delegated_from: Vec::new(),
                schedule_sequence: 0,
                schedule_updated: None,
            });
        }
        Ok(out)
    }

    pub(crate) async fn import_events(
        &self,
        events: Vec<(Value, Vec<u8>, String)>,
        source: &FileRef,
        ctx: &ImportContext,
        media: &MediaType,
    ) -> Result<ImportReport> {
        let mut report = ImportReport::default();
        let mut batch = Batch::new(source.iri.clone());
        // First pass: read and resolve the people of EVERY event. A failure of the directory ends the import here,
        // before anything is written.
        let mut ready = Vec::new();
        for (event, original, importer) in events {
            let read: Read = match read_event(&event, ctx) {
                Ok(r) => r,
                Err(Error::Invalid(p)) => {
                    report.failed.extend(p);
                    continue;
                }
                Err(e) => return Err(e),
            };
            if self.local.uid_exists(&read.uid).await? {
                // Before the directory is asked: no proposal for an object that is not imported.
                report.skipped.push(read.uid.clone());
                continue;
            }
            let mut losses = read.losses.clone();
            let people = self
                .participants(
                    &read.people,
                    &read.uid,
                    media.0 == ICS,
                    &mut losses,
                    &mut batch,
                )
                .await?;
            ready.push((read, losses, people, original, importer));
        }
        report.people_proposal = batch.id;
        for (read, losses, people, original, importer) in ready {
            let origin = NewOrigin {
                channel: OriginChannel::File,
                source: source.iri.clone(),
                media_type: media.clone(),
                foreign_uid: read.uid.clone(),
                foreign_recurrence_id: None,
                foreign_etag: None,
                prod_id: read.prod_id.clone(),
                importer,
                original: Some((original, media.clone())),
            };
            match self
                .local
                .import_event(ctx.calendar, &read, people, &origin)
                .await
            {
                Ok(done) => {
                    report.created.push(done.appointment.id);
                    report.lost.extend(done.losses);
                    report
                        .lost
                        .extend(losses.into_iter().skip(read.losses.len()));
                    if read.zone_assumed {
                        report.zone_assumed.push(read.uid.clone());
                    }
                }
                Err(Error::Invalid(p)) if p.iter().any(|x| x.field == "uid") => {
                    report.skipped.push(read.uid.clone());
                }
                Err(Error::Invalid(p)) => report.failed.extend(p),
                Err(Error::Unsupported { capability, reason }) => {
                    report.failed.push(fs_calendars_testdata::draft::Problem {
                        field: capability,
                        reason,
                    });
                }
                Err(e) => return Err(e),
            }
        }
        Ok(report)
    }
}

/// The one batch of proposals of an import (opened at the first unknown address; origin = the file).
pub(crate) struct Batch<'a> {
    origin: Reference,
    id: Option<ObjectId>,
    _unused: std::marker::PhantomData<&'a ()>,
}

impl Batch<'_> {
    fn new(origin: Reference) -> Self {
        Self {
            origin,
            id: None,
            _unused: std::marker::PhantomData,
        }
    }

    async fn open(&mut self, people: &dyn Directory) -> Result<ObjectId> {
        if let Some(id) = self.id {
            return Ok(id);
        }
        let id = people.open_batch(&self.origin).await?;
        self.id = Some(id);
        Ok(id)
    }
}
