//! Helpers shared by the tests of `LocalAppointments`.
#![allow(dead_code)]

pub mod exchange;
pub mod series;

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use fs_calendars_local::{Clock, LocalAppointments};
use fs_calendars_testdata::draft::{
    Appointment, AppointmentDraft, AppointmentPatch, Appointments, Error, ObjectId, PersonRef, Utc,
    When,
};
use fs_calendars_testdata::ids::person;
use fs_calendars_testdata::sample::{at, timed};
use turso::Value;

pub const DAY: i64 = 86_400_000;

/// Milliseconds of a UTC time.
pub fn ms(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> i64 {
    chrono::NaiveDate::from_ymd_opt(y, mo, d)
        .unwrap()
        .and_hms_opt(h, mi, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis()
}

pub struct Fx {
    pub p: LocalAppointments,
    pub clock: Arc<AtomicI64>,
    pub cal: ObjectId,
}

pub fn actor() -> PersonRef {
    person(1)
}

/// An in-memory provider, the clock at 2026-11-16 10:00 UTC, one calendar.
pub async fn fx() -> Fx {
    let clock = Arc::new(AtomicI64::new(ms(2026, 11, 16, 10, 0)));
    let p = LocalAppointments::open(":memory:", actor(), Clock::from_counter(clock.clone()))
        .await
        .unwrap();
    let cal = p
        .create_calendar("Persönlich", Some("#336699"), None, None)
        .await
        .unwrap()
        .id;
    Fx { p, clock, cal }
}

impl Fx {
    pub fn set_now(&self, ms: i64) {
        self.clock.store(ms, Ordering::SeqCst);
    }
    pub fn draft(&self, title: &str, when: When) -> AppointmentDraft {
        AppointmentDraft {
            calendar: self.cal,
            uid: None,
            title: title.to_owned(),
            when,
            rest: AppointmentPatch::default(),
        }
    }
    pub async fn make(&self, title: &str, when: When) -> Appointment {
        self.p.create(self.draft(title, when)).await.unwrap()
    }
    pub async fn make_berlin(&self, title: &str, day: u8, hour: u8) -> Appointment {
        self.make(
            title,
            timed(at(2026, 11, day, hour, 0), "Europe/Berlin", 60),
        )
        .await
    }
    /// All rows of a query on the store.
    pub async fn rows(&self, sql: &str) -> Vec<Vec<Value>> {
        self.p
            .store()
            .read()
            .await
            .query(sql, Vec::new())
            .await
            .unwrap()
    }
    pub async fn count(&self, table: &str) -> i64 {
        match self.rows(&format!("SELECT count(*) FROM {table}")).await[0][0] {
            Value::Integer(i) => i,
            ref other => panic!("not a count: {other:?}"),
        }
    }
    pub async fn exec(&self, sql: &str) -> Result<u64, Error> {
        let c = self.p.store().begin().await?;
        let r = c.exec(sql, Vec::new()).await;
        c.finish(r).await
    }
}

pub fn utc(ms: i64) -> Utc {
    Utc::from_millis(ms)
}
