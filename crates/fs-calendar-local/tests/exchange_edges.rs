//! Edges of the border: people are asked only for objects that are imported, tasks and unread members are reported,
//! a broken event costs only itself, a file is checked against its checksum.

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

mod common;

use common::exchange::{ICS_TYPE, JSCAL_TYPE, ex, import_text, media};
use fs_calendar_local::files::{FileStore, MemoryFiles};
use fs_calendar_testdata::draft::Error;

const ONE: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//self-written//edge//EN\r\nBEGIN:VEVENT\r\nUID:edge-1@example.org\r\nDTSTAMP:20261116T080000Z\r\nDTSTART;TZID=Europe/Berlin:20261201T090000\r\nDTEND;TZID=Europe/Berlin:20261201T100000\r\nSUMMARY:Mit Fremden\r\nATTENDEE;CN=Neu Person;PARTSTAT=ACCEPTED:mailto:neu.person@example.org\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

#[tokio::test]
async fn a_second_import_asks_the_directory_for_nobody() {
    let e = ex().await;
    let first = import_text(&e, ONE, ICS_TYPE).await;
    assert_eq!(first.created.len(), 1, "{first:?}");
    assert_eq!(
        e.people.proposed().len(),
        1,
        "one proposal for the unknown person"
    );
    let again = import_text(&e, ONE, ICS_TYPE).await;
    assert_eq!((again.created.len(), again.skipped.len()), (0, 1));
    assert_eq!(
        e.people.proposed().len(),
        1,
        "no second proposal for an object that is skipped"
    );
}

#[tokio::test]
async fn tasks_sequence_and_organizer_are_reported_not_dropped() {
    let e = ex().await;
    let text = ONE.replace("END:VCALENDAR", "BEGIN:VTODO\r\nUID:todo-1@example.org\r\nDTSTAMP:20261116T080000Z\r\nSUMMARY:Aufgabe\r\nEND:VTODO\r\nEND:VCALENDAR").replace("SUMMARY:Mit Fremden", "SUMMARY:Mit Fremden\r\nSEQUENCE:3\r\nORGANIZER:mailto:chef@example.org");
    let r = import_text(&e, &text, ICS_TYPE).await;
    let find = |f: &str| r.lost.iter().find(|l| l.field == f);
    assert_eq!(
        find("VTODO").and_then(|l| l.uid.as_deref()),
        Some("todo-1@example.org"),
        "{:?}",
        r.lost
    );
    assert!(find("sequence").is_some(), "{:?}", r.lost);
    let js = r#"{"@type":"Group","entries":[{"@type":"Task","uid":"t-2","title":"x"},{"@type":"Event","uid":"e-2","title":"y","start":"2026-12-01T10:00:00","timeZone":"UTC"}]}"#;
    let org = r#"{"@type":"Event","uid":"e-3","title":"z","start":"2026-12-01T10:00:00","timeZone":"UTC","organizerCalendarAddress":"mailto:chef@example.org"}"#;
    let o = import_text(&e, org, JSCAL_TYPE).await;
    assert!(
        o.lost.iter().any(|l| l.field == "organizerCalendarAddress"),
        "{:?}",
        o.lost
    );
    let j = import_text(&e, js, JSCAL_TYPE).await;
    assert_eq!(j.created.len(), 1);
    assert!(
        j.lost
            .iter()
            .any(|l| l.field == "Task" && l.uid.as_deref() == Some("t-2")),
        "{:?}",
        j.lost
    );
}

#[tokio::test]
async fn a_broken_event_beside_a_good_one_fails_alone() {
    let e = ex().await;
    let text = ONE.replace("END:VCALENDAR", "BEGIN:VEVENT\r\nUID:broken@example.org\r\nDTSTAMP:20261116T080000Z\r\nSUMMARY:Ohne Start\r\nEND:VEVENT\r\nEND:VCALENDAR");
    let r = import_text(&e, &text, ICS_TYPE).await;
    assert_eq!((r.created.len(), r.failed.len()), (1, 1), "{r:?}");
    assert!(
        r.failed.iter().any(|p| p.field.contains("start")),
        "{:?}",
        r.failed
    );
}

#[tokio::test]
async fn a_file_is_read_only_with_the_checksum_it_was_promised() {
    let files = MemoryFiles::default();
    let mut f = files.put(b"hello", &media("text/plain"));
    assert_eq!(files.read(&f).await.unwrap(), b"hello");
    f.sha256[0] ^= 1;
    assert!(
        matches!(files.read(&f).await, Err(Error::Invalid(_))),
        "a wrong checksum is refused"
    );
}

#[tokio::test]
async fn the_start_of_a_series_that_is_not_read_cannot_be_moved_and_an_edited_export_names_what_it_leaves()
 {
    use common::exchange::{export_text, fixture};
    use fs_calendar_testdata::draft::{AppointmentPatch, Appointments, SeriesScope};
    use fs_calendar_testdata::sample::{at, timed};
    let e = ex().await;
    let r = import_text(&e, &fixture("yearday_series.ics"), ICS_TYPE).await;
    let a = e.local.get(r.created[0]).await.unwrap().unwrap();
    let mv = AppointmentPatch {
        when: Some(timed(at(2027, 1, 2, 9, 0), "Europe/Berlin", 60)),
        ..Default::default()
    };
    let u = e
        .local
        .update(a.id, a.revision, mv, SeriesScope::Whole)
        .await;
    assert!(
        matches!(u, Err(Error::Unsupported { .. })),
        "the kept EXDATE and exceptions point at the old start: {u:?}"
    );
    let g = import_text(&e, &fixture("google.ics"), ICS_TYPE).await;
    let id = g.created[0];
    let ga = e.local.get(id).await.unwrap().unwrap();
    e.local
        .update(
            id,
            ga.revision,
            AppointmentPatch {
                title: Some("Neu".into()),
                ..Default::default()
            },
            SeriesScope::Whole,
        )
        .await
        .unwrap();
    let (_, lost) = export_text(&e, &[id], ICS_TYPE).await;
    let fields: Vec<&str> = lost.iter().map(|l| l.field.as_str()).collect();
    assert!(
        fields.contains(&"sequence") && fields.contains(&"organizerCalendarAddress"),
        "{fields:?}"
    );
}
