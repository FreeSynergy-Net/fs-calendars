//! The reading half of the exchange: people through the directory, one transaction per `uid`, the report.
//!
//! Pattern: Adapter (second `impl` block of `LocalExchange`).

use fs_calendar_testdata::draft::{
    Attendee, BasicValue, Error, FileRef, ImportContext, ImportReport, Loss, MediaType,
    OriginChannel, Participant, Reference, Result, Roles,
};
use serde_json::Value;

use crate::codec::invalid;
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
                "resource" => Attendee::Resource(
                    Reference::parse(addr)
                        .map_err(|_| invalid("participants", "not an address"))?,
                ),
                "location" => Attendee::Location(
                    Reference::parse(addr)
                        .map_err(|_| invalid("participants", "not an address"))?,
                ),
                _ => Attendee::Actor(match self.people.find_by_address(addr).await? {
                    Some(r) => r,
                    None => {
                        self.people
                            .propose(p.name.as_deref().unwrap_or(addr), addr)
                            .await?
                    }
                }),
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
                .participants(&read.people, &read.uid, media.0 == ICS, &mut losses)
                .await?;
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
                    report.failed.push(fs_calendar_testdata::draft::Problem {
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
