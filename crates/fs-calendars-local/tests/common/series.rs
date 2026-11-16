//! Series helpers shared by the series tests.
#![allow(dead_code)]

use super::{Fx, ms, utc};
use fs_calendars_testdata::draft::{
    Appointment, AppointmentPatch, Appointments, Frequency, LocalDateTime, RuleEnd, SeriesScope,
    Window,
};
use fs_calendars_testdata::sample::{at, rule, timed};
use fs_calendars_testdata::zone::iana;

pub fn rid(day: u8) -> LocalDateTime {
    at(2026, 11, day, 9, 0)
}
pub fn this(day: u8) -> SeriesScope {
    SeriesScope::This {
        recurrence_id: rid(day),
    }
}
pub fn following(day: u8) -> SeriesScope {
    SeriesScope::ThisAndFollowing {
        recurrence_id: rid(day),
    }
}
pub fn title(t: &str) -> AppointmentPatch {
    AppointmentPatch {
        title: Some(t.to_owned()),
        ..AppointmentPatch::default()
    }
}

/// Weekly on Monday 09:00 Berlin from 2026-11-02, five times (2, 9, 16, 23, 30 November).
pub async fn weekly(f: &Fx) -> Appointment {
    let mut d = f.draft("Montag", timed(rid(2), "Europe/Berlin", 60));
    let mut r = rule(Frequency::Weekly);
    r.end = RuleEnd::Count(5);
    d.rest.recurrence = Some(vec![r]);
    f.p.create(d).await.unwrap()
}

pub async fn days(f: &Fx) -> Window {
    f.p.window(
        utc(ms(2026, 11, 1, 0, 0)),
        utc(ms(2026, 12, 15, 0, 0)),
        &iana("Europe/Berlin"),
        &[],
    )
    .await
    .unwrap()
}

pub fn starts(w: &Window) -> Vec<String> {
    w.instances
        .iter()
        .map(|i| {
            format!(
                "{}",
                (i.start.millis() - ms(2026, 11, 1, 0, 0)) / 86_400_000
            )
        })
        .collect()
}
