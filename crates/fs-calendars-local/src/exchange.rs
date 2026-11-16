//! The border: `Exchange` for iCalendar (RFC 5545) and JSCalendar (RFC 8984), both ways (open formats).
//!
//! Pattern: Adapter. Reading: split into one part per `uid`, convert (`fs-jscalendar`), normalize (`jscal_in`),
//! resolve people through the directory, write through `LocalAppointments` in one transaction per `uid`; the received
//! part is kept as the original. Writing: unchanged since import and same type = the original byte for byte (ADR
//! 0085); otherwise the typed values as JSON, then iCalendar or JSCalendar, with the losses listed.

use std::sync::Arc;

use async_trait::async_trait;
use fs_calendars_testdata::draft::{
    Attendee, BasicValue, Error, Exchange, ExportReport, ExportTarget, FileRef, FormatSupport,
    ImportContext, ImportReport, Level, MediaType, ObjectId, Result,
};
use fs_calendars_testdata::people::Directory;
use fs_jscalendar::ical::{Component, parse_entries, split_ics, write_ics};
use serde_json::Value;

use crate::codec::invalid;
use crate::files::FileStore;
use crate::jscal_out::{People, event_json};
use crate::provider::LocalAppointments;

pub const ICS: &str = "text/calendar";
pub const JSCAL: &str = "application/jscalendar+json";
const PROD_ID: &str = "-//FreeSynergy//fs-calendars//EN";

/// The exchange of one local store.
pub struct LocalExchange {
    pub(crate) local: Arc<LocalAppointments>,
    pub(crate) people: Arc<dyn Directory>,
    pub(crate) files: Arc<dyn FileStore>,
}

impl LocalExchange {
    #[must_use]
    pub fn new(
        local: Arc<LocalAppointments>,
        people: Arc<dyn Directory>,
        files: Arc<dyn FileStore>,
    ) -> Self {
        Self {
            local,
            people,
            files,
        }
    }

    async fn people_of(
        &self,
        ids: &[ObjectId],
    ) -> Result<Vec<(crate::provider_admin::ExportSource, People)>> {
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

fn event_original(
    src: &crate::provider_admin::ExportSource,
    target: &MediaType,
) -> Option<Vec<u8>> {
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
        let importer = format!("fs-calendars-local {}", env!("CARGO_PKG_VERSION"));
        let mut events: Vec<(Value, Vec<u8>, String)> = Vec::new();
        let mut report = ImportReport::default();
        match as_type.0.as_str() {
            ICS => {
                report.lost.extend(tasks_in(&text));
                for part in split_ics(&text, Component::Event) {
                    match parse_entries(&part.text, "Event") {
                        Ok((entries, _notes)) => events.extend(entries.into_iter().map(|e| {
                            (
                                e,
                                part.text.clone().into_bytes(),
                                format!("{importer} (ics)"),
                            )
                        })),
                        Err(e) => report.failed.push(fs_calendars_testdata::draft::Problem {
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
                for e in &list {
                    if e.get("@type").and_then(Value::as_str) == Some("Task") {
                        report
                            .lost
                            .push(task_loss(e.get("uid").and_then(Value::as_str), "Task"));
                    }
                }
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
        report.people_proposal = more.people_proposal;
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
                let kept = crate::exchange_export::kept_recurrence(s);
                lost.extend(crate::exchange_export::only_in_original(s));
                if kept.is_none() && s.unread.iter().any(|f| f == "recurrence") {
                    lost.push(fs_calendars_testdata::draft::Loss {
                        uid: Some(s.appointment.uid.clone()),
                        field: "recurrence".into(),
                        reason:
                            "the received original is gone: the series is written without its rule"
                                .into(),
                    });
                }
                event_json(&s.appointment, people, kept.as_ref(), &mut lost)
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

fn task_loss(uid: Option<&str>, kind: &str) -> fs_calendars_testdata::draft::Loss {
    fs_calendars_testdata::draft::Loss {
        uid: uid.map(str::to_owned),
        field: kind.to_owned(),
        reason: "a task belongs to fs-tasks: not imported here, not forwarded yet".into(),
    }
}

/// The `VTODO` components of an iCalendar text (the event reader leaves them out): one loss each.
fn tasks_in(text: &str) -> Vec<fs_calendars_testdata::draft::Loss> {
    let (mut out, mut inside, mut uid) = (Vec::new(), false, None::<String>);
    for line in fs_jscalendar::ical::unfolded_lines(text) {
        match line.as_str() {
            "BEGIN:VTODO" => (inside, uid) = (true, None),
            "END:VTODO" => {
                out.push(task_loss(uid.take().as_deref(), "VTODO"));
                inside = false;
            }
            l if inside && l.starts_with("UID:") => uid = Some(l[4..].to_owned()),
            _ => {}
        }
    }
    out
}
