//! iCalendar (RFC 5545) at the border: fixtures in the dialects of Google, Outlook and Apple (import tolerance only).
//! The fixtures are SELF-WRITTEN after what those dialects are known to look like; they are not real exports (the
//! PRODIDs only imitate the producers). The checklist lines of Google, Outlook and Apple stay open until real files exist.
//!
//! normalization (zone, floating, people), the original kept, and the loss report.

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

mod common;

use common::exchange::{ICS_TYPE, ex, fixture, import_text};
use fs_calendars_testdata::draft::{Appointments, Attendee, When};

#[tokio::test]
async fn google_file_is_read_with_its_series_people_and_alert() {
    let e = ex().await;
    let r = import_text(&e, &fixture("google.ics"), ICS_TYPE).await;
    assert_eq!(
        (r.created.len(), r.failed.len(), r.skipped.len()),
        (1, 0, 0),
        "{r:?}"
    );
    let a = e.local.get(r.created[0]).await.unwrap().unwrap();
    assert_eq!(a.title, "Chorprobe");
    assert!(
        matches!(&a.when, When::Timed { zone, zone_assumed: false, .. } if zone.id() == "Europe/Berlin")
    );
    assert_eq!(a.recurrence.len(), 1);
    assert_eq!(a.overrides.len(), 1, "the EXDATE is an excluded occurrence");
    assert_eq!(a.participants.len(), 2);
    assert!(
        a.participants
            .iter()
            .all(|p| matches!(p.who, Attendee::Actor(_))),
        "people through the directory, no free names"
    );
    assert_eq!(a.alerts.len(), 1);
    let o = a.origin.as_ref().unwrap();
    assert_eq!(
        (o.foreign_uid.as_str(), o.media_type.0.as_str()),
        ("abc123google@google.com", "text/calendar")
    );
    assert!(
        o.original.is_some() && o.rev_at_import == a.revision,
        "unchanged since import is computed"
    );
    assert!(
        r.lost.iter().any(|l| l.field == "locations"),
        "the location is not stored yet: reported, not lost quietly: {:?}",
        r.lost
    );
    assert!(
        r.lost
            .iter()
            .all(|l| l.uid.as_deref() == Some("abc123google@google.com"))
    );
}

#[tokio::test]
async fn outlook_zone_name_and_all_day_event() {
    let e = ex().await;
    let r = import_text(&e, &fixture("outlook.ics"), ICS_TYPE).await;
    assert_eq!(r.created.len(), 2, "{r:?}");
    let mut found = Vec::new();
    for id in &r.created {
        found.push(e.local.get(*id).await.unwrap().unwrap());
    }
    let timed = found.iter().find(|a| a.title == "Planung Q1").unwrap();
    assert!(
        matches!(&timed.when, When::Timed { zone, zone_assumed: false, .. } if zone.id() == "Europe/Berlin"),
        "W. Europe Standard Time is Europe/Berlin: {:?}",
        timed.when
    );
    assert_eq!(timed.locale.as_deref(), Some("de"));
    let day = found.iter().find(|a| a.title == "Urlaub").unwrap();
    assert!(matches!(&day.when, When::AllDay { days: 3, .. }));
    assert_eq!(day.free_busy, fs_calendars_testdata::draft::FreeBusy::Free);
    assert!(
        r.lost
            .iter()
            .any(|l| l.field == "x-properties" && l.reason.contains("x-microsoft-cdo-busystatus")),
        "vendor properties are named in the report: {:?}",
        r.lost
    );
}

#[tokio::test]
async fn apple_file_absolute_alert_and_all_day() {
    let e = ex().await;
    let r = import_text(&e, &fixture("apple.ics"), ICS_TYPE).await;
    assert_eq!(r.created.len(), 2, "{r:?}");
    let a = e
        .local
        .get_by_uid("6F1B2C3D-0000-4000-8000-APPLE0000001")
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        a.alerts[0].trigger,
        fs_calendars_testdata::draft::Trigger::Absolute { .. }
    ));
    let d = e
        .local
        .get_by_uid("6F1B2C3D-0000-4000-8000-APPLE0000002")
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(d.when, When::AllDay { days: 1, .. }));
}

#[tokio::test]
async fn a_time_without_zone_is_assumed_and_reported_unless_the_file_says_floating() {
    let e = ex().await;
    let r = import_text(&e, &fixture("dialect.ics"), ICS_TYPE).await;
    let plain = e
        .local
        .get_by_uid("floating-1@example.org")
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(&plain.when, When::Timed { zone, zone_assumed: true, .. } if zone.id() == "Europe/Berlin"),
        "never Floating by itself: {:?}",
        plain.when
    );
    let marked = e
        .local
        .get_by_uid("floating-2@example.org")
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(marked.when, When::Floating { .. }),
        "X-FS-FLOATING: {:?}",
        marked.when
    );
    assert_eq!(r.zone_assumed, ["floating-1@example.org"]);
}

#[tokio::test]
async fn a_rule_outside_the_subset_is_kept_shown_once_and_reported() {
    let e = ex().await;
    let r = import_text(&e, &fixture("dialect.ics"), ICS_TYPE).await;
    let a = e
        .local
        .get_by_uid("yearday-1@example.org")
        .await
        .unwrap()
        .unwrap();
    assert!(
        a.recurrence.is_empty(),
        "no interpreted rule: the first occurrence shows"
    );
    let w = e
        .local
        .window(
            common::utc(common::ms(2026, 11, 1, 0, 0)),
            common::utc(common::ms(2026, 11, 2, 0, 0)),
            &fs_calendars_testdata::zone::iana("UTC"),
            &[],
        )
        .await
        .unwrap();
    assert!(w.instances.iter().any(|i| i.appointment == a.id));
    let loss = r
        .lost
        .iter()
        .find(|l| l.field == "recurrence")
        .expect("reported");
    assert!(
        loss.reason.contains("Serie nur teilweise darstellbar"),
        "{loss:?}"
    );
    let src = e.local.export_source(a.id).await.unwrap();
    assert_eq!(
        src.unread,
        ["recurrence"],
        "only the mark is stored, the rule stays in the original"
    );
    assert!(
        String::from_utf8(src.original.unwrap().0)
            .unwrap()
            .contains("BYYEARDAY=1,100,200")
    );
    let edit = fs_calendars_testdata::draft::AppointmentPatch {
        recurrence: Some(vec![fs_calendars_testdata::sample::rule(
            fs_calendars_testdata::draft::Frequency::Daily,
        )]),
        ..Default::default()
    };
    let u = e
        .local
        .update(
            a.id,
            a.revision,
            edit,
            fs_calendars_testdata::draft::SeriesScope::Whole,
        )
        .await;
    assert!(
        matches!(
            u,
            Err(fs_calendars_testdata::draft::Error::Unsupported { .. })
        ),
        "editing the rule is refused: {u:?}"
    );
}

#[tokio::test]
async fn importing_the_same_file_twice_adds_nothing() {
    let e = ex().await;
    let first = import_text(&e, &fixture("apple.ics"), ICS_TYPE).await;
    let again = import_text(&e, &fixture("apple.ics"), ICS_TYPE).await;
    assert_eq!(
        (
            first.created.len(),
            again.created.len(),
            again.skipped.len()
        ),
        (2, 0, 2)
    );
}
