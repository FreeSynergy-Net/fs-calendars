//! The border: `Exchange` for iCalendar (RFC 5545) and JSCalendar (RFC 8984), both ways (open formats).
//!
//! Pattern: Adapter. Reading: split into one part per `uid`, convert (`fs-jscalendar`), normalize (`jscal_in`),
//! resolve people through the directory, write through `LocalAppointments` in one transaction per `uid`; the received
//! part is kept as the original. Writing: unchanged since import and same type = the original byte for byte (ADR
//! 0085); otherwise the typed values as JSON, then iCalendar or JSCalendar, with the losses listed.

use std::sync::Arc;

use async_trait::async_trait;
use fs_calendar_testdata::draft::{
    Attendee, BasicValue, Error, Exchange, ExportReport, ExportTarget, FileRef, FormatSupport,
    ImportContext, ImportReport, Level, Loss, MediaType, ObjectId, OriginChannel, Participant,
    PersonDirectory, Reference, Result,
};
use fs_jscalendar::ical::{Component, parse_entries, split_ics, write_ics};
use serde_json::Value;

use crate::codec::invalid;
use crate::files::FileStore;
use crate::jscal_in::{RawPerson, Read, read_event};
use crate::jscal_out::{People, event_json};
use crate::origin::NewOrigin;
use crate::provider::LocalAppointments;

pub const ICS: &str = "text/calendar";
pub const JSCAL: &str = "application/jscalendar+json";
const PROD_ID: &str = "-//FreeSynergy//fs-calendar//EN";

/// The exchange of one local store.
pub struct LocalExchange {
    local: Arc<LocalAppointments>,
    people: Arc<dyn PersonDirectory>,
    files: Arc<dyn FileStore>,
}

impl LocalExchange {
    #[must_use]
    pub fn new(
        local: Arc<LocalAppointments>,
        people: Arc<dyn PersonDirectory>,
        files: Arc<dyn FileStore>,
    ) -> Self {
        Self {
            local,
            people,
            files,
        }
    }

    async fn participants(
        &self,
        raw: &[RawPerson],
        uid: &str,
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
            out.push(Participant {
                key: p.key.clone(),
                who,
                roles: p.roles,
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

    async fn import_events(
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
            let mut losses = read.losses.clone();
            let people = self
                .participants(&read.people, &read.uid, &mut losses)
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
                    report.skipped.push(read.uid.clone())
                }
                Err(Error::Invalid(p)) => report.failed.extend(p),
                Err(Error::Unsupported { capability, reason }) => {
                    report.failed.push(fs_calendar_testdata::draft::Problem {
                        field: capability,
                        reason,
                    })
                }
                Err(e) => return Err(e),
            }
        }
        Ok(report)
    }

    async fn people_of(
        &self,
        ids: &[ObjectId],
    ) -> Result<Vec<(crate::provider::ExportSource, People)>> {
        let mut out = Vec::new();
        for id in ids {
            let src = self.local.export_source(*id).await?;
            let mut people = People::new();
            for p in &src.appointment.participants {
                if let Attendee::Actor(r) = &p.who {
                    if let Some((name, addr)) = self.people.display(r).await? {
                        people.insert(p.key.clone(), (Some(name), addr));
                    }
                } else if let Attendee::Resource(r) | Attendee::Location(r) = &p.who {
                    people.insert(
                        p.key.clone(),
                        (
                            None,
                            Some(r.canonical()).filter(|a| a.starts_with("mailto:")),
                        ),
                    );
                }
            }
            out.push((src, people));
        }
        Ok(out)
    }
}

fn event_original(src: &crate::provider::ExportSource, target: &MediaType) -> Option<Vec<u8>> {
    let unchanged = src
        .appointment
        .origin
        .as_ref()
        .is_some_and(|o| o.rev_at_import == src.appointment.revision);
    src.original
        .as_ref()
        .filter(|(_, m)| unchanged && m == target)
        .map(|(b, _)| b.clone())
}

#[async_trait]
impl Exchange for LocalExchange {
    fn formats(&self) -> Vec<FormatSupport> {
        let partial = |m: &[&str]| Level::Partial {
            missing: m.iter().map(|s| (*s).to_owned()).collect(),
        };
        let miss = [
            "locations",
            "virtualLocations",
            "links",
            "localizations",
            "replyTo",
            "custom time zones",
            "rules outside the expanded subset (kept as written)",
        ];
        vec![
            FormatSupport {
                media_type: MediaType(ICS.into()),
                import: partial(&miss),
                export: partial(&miss),
            },
            FormatSupport {
                media_type: MediaType(JSCAL.into()),
                import: partial(&miss),
                export: partial(&miss),
            },
        ]
    }

    async fn import(
        &self,
        source: FileRef,
        as_type: &MediaType,
        ctx: &ImportContext,
    ) -> Result<ImportReport> {
        let bytes = self.files.read(&source).await?;
        let text = String::from_utf8(bytes).map_err(|_| invalid("source", "not UTF-8 text"))?;
        let importer = format!("fs-calendar-local {}", env!("CARGO_PKG_VERSION"));
        let mut events: Vec<(Value, Vec<u8>, String)> = Vec::new();
        let mut report = ImportReport::default();
        match as_type.0.as_str() {
            ICS => {
                for part in split_ics(&text, Component::Event) {
                    match parse_entries(&part.text, "Event") {
                        Ok((entries, _notes)) => events.extend(entries.into_iter().map(|e| {
                            (
                                e,
                                part.text.clone().into_bytes(),
                                format!("{importer} (ics)"),
                            )
                        })),
                        Err(e) => report.failed.push(fs_calendar_testdata::draft::Problem {
                            field: part.uid,
                            reason: e.to_string(),
                        }),
                    }
                }
            }
            JSCAL => {
                let v: Value = serde_json::from_str(&text)
                    .map_err(|e| invalid("source", &format!("not JSON: {e}")))?;
                let up = fs_jscalendar::upgrade::upgrade(v);
                let list: Vec<Value> = match up.value.get("entries").and_then(Value::as_array) {
                    Some(a) => a.clone(),
                    None => vec![up.value.clone()],
                };
                let single = list.len() == 1;
                for e in list
                    .into_iter()
                    .filter(|e| e.get("@type").and_then(Value::as_str) == Some("Event"))
                {
                    let original = if single {
                        text.clone().into_bytes()
                    } else {
                        serde_json::to_vec(&e).unwrap_or_default()
                    };
                    events.push((e, original, format!("{importer} (jscalendar)")));
                }
            }
            other => {
                return Err(Error::Unsupported {
                    capability: "exchange.import".into(),
                    reason: format!("{other} is not a format of this provider"),
                });
            }
        }
        let more = self.import_events(events, &source, ctx, as_type).await?;
        report.created.extend(more.created);
        report.skipped.extend(more.skipped);
        report.failed.extend(more.failed);
        report.lost.extend(more.lost);
        report.zone_assumed.extend(more.zone_assumed);
        Ok(report)
    }

    async fn export(&self, ids: &[ObjectId], to: &ExportTarget) -> Result<ExportReport> {
        let target = &to.media_type;
        if target.0 != ICS && target.0 != JSCAL {
            return Err(Error::Unsupported {
                capability: "exchange.export".into(),
                reason: format!("{} is not a format of this provider", target.0),
            });
        }
        let sources = self.people_of(ids).await?;
        if let [(src, _)] = sources.as_slice() {
            if let Some(bytes) = event_original(src, target) {
                return Ok(ExportReport {
                    written: self.files.write(bytes, target).await?,
                    lost: Vec::new(),
                });
            }
        }
        let mut lost = Vec::new();
        let events: Vec<Value> = sources
            .iter()
            .map(|(s, people)| {
                let rule = s
                    .unread
                    .iter()
                    .find(|(f, _)| f == "rrule")
                    .map(|(_, t)| t.as_str());
                event_json(&s.appointment, people, rule, &mut lost)
            })
            .collect();
        let bytes = if target.0 == ICS {
            write_ics(events, PROD_ID)
                .map_err(|e| Error::Unavailable(e.to_string()))?
                .into_bytes()
        } else if let [one] = events.as_slice() {
            serde_json::to_vec_pretty(one).unwrap_or_default()
        } else {
            serde_json::to_vec_pretty(&serde_json::json!({"@type": "Group", "entries": events}))
                .unwrap_or_default()
        };
        Ok(ExportReport {
            written: self.files.write(bytes, target).await?,
            lost,
        })
    }
}
