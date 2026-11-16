//! `fs-types` is stricter than the stand-ins were (review L3): an address that is no absolute IRI costs only its person
//! (left out and reported as a loss), never the appointment and never the whole import.

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

mod common;

use common::exchange::{ICS_TYPE, ex, import_text};
use fs_calendars_testdata::draft::Appointments;

fn event(uid: &str, attendee: &str) -> String {
    format!(
        "BEGIN:VEVENT\r\nUID:{uid}@example.org\r\nDTSTAMP:20261116T080000Z\r\nDTSTART;TZID=Europe/Berlin:20261201T090000\r\nDTEND;TZID=Europe/Berlin:20261201T100000\r\nSUMMARY:{uid}\r\n{attendee}END:VEVENT\r\n"
    )
}

#[tokio::test]
async fn an_address_that_is_no_iri_costs_only_its_person() {
    let e = ex().await;
    let text = format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//self-written//strict//EN\r\n{}{}{}{}END:VCALENDAR\r\n",
        event("bad1", "ATTENDEE;CUTYPE=RESOURCE:mailto:a b\r\n"),
        event(
            "good",
            "ATTENDEE;PARTSTAT=ACCEPTED:mailto:ok@example.org\r\n"
        ),
        event("bad2", "ATTENDEE;CUTYPE=ROOM:mailto:c d\r\n"),
        event("plain", ""),
    );
    let r = import_text(&e, &text, ICS_TYPE).await;
    assert_eq!(r.created.len(), 4, "all four are created: {r:?}");
    assert!(
        r.failed.is_empty(),
        "an address costs only its person: {r:?}"
    );
    let left_out: Vec<&str> = r
        .lost
        .iter()
        .filter(|l| l.reason == "address.unsupported")
        .filter_map(|l| l.uid.as_deref())
        .collect();
    assert_eq!(
        left_out,
        ["bad1@example.org", "bad2@example.org"],
        "the person with a bad address is left out and reported, never dropped quietly: {r:?}"
    );
    for id in &r.created {
        assert!(e.local.get(*id).await.unwrap().is_some());
    }
}
