//! The expected occurrences come from RFC 5545 section 3.8.5.3 as fixed lists (date and wall-clock time, and the UTC
//! instant across the change of the clock), not from the engine of the fake. New York, 09:00 local.

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

mod common;

use common::exchange::{ICS_TYPE, ex, import_text};
use fs_calendar_testdata::draft::{Appointments, BasicValue};

/// `(rule line, DTSTART date, expected local dates)`; every occurrence is at 09:00 local time.
type Case = (&'static str, &'static str, &'static [&'static str]);

const CASES: [Case; 6] = [
    (
        "FREQ=DAILY;COUNT=10",
        "19970902",
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
        "FREQ=WEEKLY;COUNT=10",
        "19970902",
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
    // "Every other day": INTERVAL (the RFC example is endless; five occurrences are cut by COUNT here).
    (
        "FREQ=DAILY;INTERVAL=2;COUNT=5",
        "19970902",
        &[
            "1997-09-02",
            "1997-09-04",
            "1997-09-06",
            "1997-09-08",
            "1997-09-10",
        ],
    ),
    // "Weekly on Tuesday and Thursday for five weeks": UNTIL and BYDAY.
    (
        "FREQ=WEEKLY;UNTIL=19971007T000000Z;WKST=SU;BYDAY=TU,TH",
        "19970902",
        &[
            "1997-09-02",
            "1997-09-04",
            "1997-09-09",
            "1997-09-11",
            "1997-09-16",
            "1997-09-18",
            "1997-09-23",
            "1997-09-25",
            "1997-09-30",
            "1997-10-02",
        ],
    ),
    // "Monthly on the first Friday for 10 occurrences": BYDAY with a position.
    (
        "FREQ=MONTHLY;COUNT=10;BYDAY=1FR",
        "19970905",
        &[
            "1997-09-05",
            "1997-10-03",
            "1997-11-07",
            "1997-12-05",
            "1998-01-02",
            "1998-02-06",
            "1998-03-06",
            "1998-04-03",
            "1998-05-01",
            "1998-06-05",
        ],
    ),
    // "The third instance into the month of one of Tuesday, Wednesday or Thursday, for the next 3 months": BYSETPOS.
    (
        "FREQ=MONTHLY;COUNT=3;BYDAY=TU,WE,TH;BYSETPOS=3",
        "19970904",
        &["1997-09-04", "1997-10-07", "1997-11-06"],
    ),
];

#[tokio::test]
async fn expected_occurrences_come_from_rfc_5545_section_3_8_5_3() {
    let e = ex().await;
    let ny = fs_calendar_testdata::zone::iana("America/New_York");
    for (n, (rule, start, days)) in CASES.iter().enumerate() {
        let uid = format!("rfc-{n}");
        let text = format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//self-written//RFC 5545 example//EN\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:19970901T000000Z\r\nDTSTART;TZID=America/New_York:{start}T090000\r\nDTEND;TZID=America/New_York:{start}T100000\r\nSUMMARY:RFC\r\nRRULE:{rule}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
        );
        let r = import_text(&e, &text, ICS_TYPE).await;
        assert_eq!(r.created.len(), 1, "{rule}: {r:?}");
        let w = e
            .local
            .window(
                common::utc(common::ms(1997, 8, 1, 0, 0)),
                common::utc(common::ms(1998, 8, 1, 0, 0)),
                &ny,
                &[],
            )
            .await
            .unwrap();
        let mine: Vec<_> = w
            .instances
            .iter()
            .filter(|i| i.appointment == r.created[0])
            .collect();
        let got: Vec<String> = mine
            .iter()
            .map(|i| i.recurrence_id.unwrap().canonical())
            .collect();
        let want: Vec<String> = days.iter().map(|d| format!("{d}T09:00:00")).collect();
        assert_eq!(got, want, "{rule}: date and wall-clock time");
        // 09:00 in New York is 13:00 UTC in summer time (until 26 October 1997) and 14:00 UTC after it (and from 5 April 1998 again 13:00).
        for i in &mine {
            let day = &i.recurrence_id.unwrap().canonical()[..10];
            let winter = ("1997-10-26".."1998-04-05").contains(&day);
            let (y, m, d) = (
                day[..4].parse().unwrap(),
                day[5..7].parse().unwrap(),
                day[8..].parse().unwrap(),
            );
            assert_eq!(
                i.start.millis(),
                common::ms(y, m, d, if winter { 14 } else { 13 }, 0),
                "{rule}: the instant of {day} follows the clock change"
            );
        }
    }
}
