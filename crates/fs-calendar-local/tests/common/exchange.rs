//! Helpers of the exchange tests: a store, a directory, memory files, the fixtures.
#![allow(dead_code)]

use std::sync::Arc;

use fs_calendar_local::exchange::{ICS, JSCAL, LocalExchange};
use fs_calendar_local::files::{FileStore, MemoryFiles};
use fs_calendar_testdata::draft::{
    Exchange, FileRef, ImportContext, ImportReport, MediaType, ZoneSource,
};
use fs_calendar_testdata::sample::{Sample, sample};
use fs_calendar_testdata::zone::iana;

use super::{Fx, fx};

pub struct Ex {
    pub fx: Fx,
    pub local: Arc<fs_calendar_local::LocalAppointments>,
    pub files: Arc<MemoryFiles>,
    pub people: Arc<fs_calendar_testdata::FakePeople>,
    pub ex: LocalExchange,
}

pub async fn ex() -> Ex {
    let f = fx().await;
    let clock = f.clock.clone();
    let local = Arc::new(
        fs_calendar_local::LocalAppointments::open(
            ":memory:",
            super::actor(),
            fs_calendar_local::Clock::from_counter(clock),
        )
        .await
        .unwrap(),
    );
    let cal = local
        .create_calendar("Import", None, None)
        .await
        .unwrap()
        .id;
    let files = Arc::new(MemoryFiles::default());
    let s: Sample = sample();
    let people = Arc::new(s.directory());
    let ex = LocalExchange::new(local.clone(), people.clone(), files.clone());
    let mut f = f;
    f.cal = cal;
    Ex {
        fx: f,
        local,
        files,
        people,
        ex,
    }
}

use fs_calendar_testdata::draft::Appointments;

pub fn ctx(e: &Ex) -> ImportContext {
    ImportContext {
        calendar: e.fx.cal,
        assume_zone: iana("Europe/Berlin"),
        assume_zone_source: ZoneSource::PersonSetting,
    }
}

pub fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

pub fn media(t: &str) -> MediaType {
    MediaType(t.to_owned())
}

pub async fn import_text(e: &Ex, text: &str, ty: &str) -> ImportReport {
    let f: FileRef = e.files.put(text.as_bytes(), &media(ty));
    e.ex.import(f, &media(ty), &ctx(e)).await.unwrap()
}

pub async fn export_text(
    e: &Ex,
    ids: &[fs_calendar_testdata::draft::ObjectId],
    ty: &str,
) -> (String, Vec<fs_calendar_testdata::draft::Loss>) {
    let r =
        e.ex.export(
            ids,
            &fs_calendar_testdata::draft::ExportTarget {
                media_type: media(ty),
                destination: None,
            },
        )
        .await
        .unwrap();
    let bytes = e.files.read(&r.written).await.unwrap();
    (String::from_utf8(bytes).unwrap(), r.lost)
}

pub const ICS_TYPE: &str = ICS;
pub const JSCAL_TYPE: &str = JSCAL;
