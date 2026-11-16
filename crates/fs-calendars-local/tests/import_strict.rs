//! `fs-types` is stricter than the stand-ins of the trait (review L3): an address the stand-in takes and the layer's
//! type refuses must cost only its own appointment, not the whole import.

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
async fn an_address_that_only_the_stand_in_accepts_costs_only_its_appointment() {
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
    assert_eq!(r.created.len(), 2, "good and plain are created: {r:?}");
    assert_eq!(r.failed.len(), 2, "one report per bad appointment: {r:?}");
    assert!(
        r.failed
            .iter()
            .all(|p| p.field == "calendar_participant.who_id"),
        "the layer refuses the address, the appointment is reported as failed: {r:?}"
    );
    for id in &r.created {
        assert!(e.local.get(*id).await.unwrap().is_some());
    }
}
