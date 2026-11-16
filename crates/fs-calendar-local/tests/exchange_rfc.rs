//! The expected occurrences come from RFC 5545 section 3.8.5.3 (fixed lists), not from the engine of the fake.

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

mod common;

use common::exchange::{ICS_TYPE, ex, import_text};
use fs_calendar_testdata::draft::{Appointments, BasicValue};

#[tokio::test]
async fn expected_occurrences_come_from_rfc_5545_section_3_8_5_3() {
    let e = ex().await;
    let cases: [(&str, &str, &[&str]); 2] = [
        (
            "RRULE:FREQ=DAILY;COUNT=10",
            "rfc-daily",
            &[
                "1997-09-02",
                "1997-09-03",
                "1997-09-04",
                "1997-09-05",
                "1997-09-06",
                "1997-09-07",
                "1997-09-08",
                "1997-09-09",
                "1997-09-10",
                "1997-09-11",
            ],
        ),
        (
            "RRULE:FREQ=WEEKLY;COUNT=10",
            "rfc-weekly",
            &[
                "1997-09-02",
                "1997-09-09",
                "1997-09-16",
                "1997-09-23",
                "1997-09-30",
                "1997-10-07",
                "1997-10-14",
                "1997-10-21",
                "1997-10-28",
                "1997-11-04",
            ],
        ),
    ];
    for (line, uid, days) in cases {
        let text = format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//RFC//5545//EN\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:19970901T000000Z\r\nDTSTART;TZID=America/New_York:19970902T090000\r\nDTEND;TZID=America/New_York:19970902T100000\r\nSUMMARY:RFC\r\n{line}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
        );
        let r = import_text(&e, &text, ICS_TYPE).await;
        assert_eq!(r.created.len(), 1, "{uid}: {r:?}");
        let ny = fs_calendar_testdata::zone::iana("America/New_York");
        let w = e
            .local
            .window(
                common::utc(common::ms(1997, 8, 1, 0, 0)),
                common::utc(common::ms(1998, 3, 1, 0, 0)),
                &ny,
                &[],
            )
            .await
            .unwrap();
        let got: Vec<String> = w
            .instances
            .iter()
            .filter(|i| i.appointment == r.created[0])
            .map(|i| i.recurrence_id.unwrap().canonical()[..10].to_owned())
            .collect();
        assert_eq!(got, days, "{uid}");
    }
}
