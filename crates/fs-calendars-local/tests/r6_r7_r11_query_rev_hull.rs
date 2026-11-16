//! R6 (typed queries and keyset paging), R7 (revision compare-and-set), R11 (derived hull), draft section 4.

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

mod common;

use common::{fx, ms, utc};
use fs_calendars_testdata::draft::{
    AppointmentPatch, AppointmentQuery, Appointments, Attendee, Error, Page, Participant,
    ParticipationStatus, Roles, SeriesScope,
};
use fs_calendars_testdata::ids::person;
use fs_calendars_testdata::sample::{all_day, at, timed};
use fs_calendars_testdata::zone::iana;
use turso::Value;

fn title(t: &str) -> AppointmentPatch {
    AppointmentPatch {
        title: Some(t.to_owned()),
        ..AppointmentPatch::default()
    }
}

#[tokio::test]
async fn r6_a_thousand_rows_page_in_the_same_order_as_a_full_sort() {
    let f = fx().await;
    for i in 0..1000_u32 {
        let (d, h, m) = (1 + (i / 60) % 28, i % 24, (i * 7) % 60);
        f.make(
            &format!("T{i:04}"),
            timed(
                at(
                    2026,
                    12,
                    u8::try_from(d).unwrap(),
                    u8::try_from(h).unwrap(),
                    u8::try_from(m).unwrap(),
                ),
                "Europe/Berlin",
                30,
            ),
        )
        .await;
    }
    let q = AppointmentQuery::default();
    let mut all = Vec::new();
    let mut after = None;
    loop {
        let Page { items, next } = f.p.list(&q, after.as_ref(), 100).await.unwrap();
        all.extend(items);
        match next {
            Some(n) => after = Some(n),
            None => break,
        }
    }
    assert_eq!(all.len(), 1000);
    let sorted = f
        .rows("SELECT t.id FROM calendar_appointment t ORDER BY t.when_utc_min, t.id")
        .await;
    let ids: Vec<String> = sorted
        .iter()
        .map(|r| {
            if let Value::Text(t) = &r[0] {
                t.clone()
            } else {
                String::new()
            }
        })
        .collect();
    let got: Vec<String> = all
        .iter()
        .map(|s| fs_calendars_local::codec::id_text(s.id))
        .collect();
    assert_eq!(got, ids, "pages = one full sort");
}

#[tokio::test]
async fn r6_range_equality_and_text_queries_are_typed() {
    let f = fx().await;
    let a = f.make_berlin("Zahnarzt", 2, 9).await;
    f.make_berlin("Sport", 9, 18).await;
    let mut d = f.draft(
        "Treffen",
        timed(at(2026, 11, 20, 12, 0), "America/New_York", 60),
    );
    d.rest.participants = Some(vec![Participant {
        key: "p".into(),
        who: Attendee::Actor(person(7)),
        roles: Roles::default(),
        status: ParticipationStatus::NeedsAction,
        expect_reply: false,
        comment: None,
        send_to: vec![],
        invited_by: None,
        delegated_to: vec![],
        delegated_from: vec![],
        schedule_sequence: 0,
        schedule_updated: None,
    }]);
    f.p.create(d).await.unwrap();
    let titles = |p: Page<fs_calendars_testdata::draft::AppointmentSummary>| {
        p.items.into_iter().map(|s| s.title).collect::<Vec<_>>()
    };
    let range = AppointmentQuery {
        from: Some(utc(ms(2026, 11, 8, 0, 0))),
        to: Some(utc(ms(2026, 11, 21, 0, 0))),
        ..AppointmentQuery::default()
    };
    assert_eq!(
        titles(f.p.list(&range, None, 10).await.unwrap()),
        ["Sport", "Treffen"]
    );
    let who = AppointmentQuery {
        participant: Some(Attendee::Actor(person(7))),
        ..AppointmentQuery::default()
    };
    assert_eq!(
        titles(f.p.list(&who, None, 10).await.unwrap()),
        ["Treffen"],
        "equality on PERSON"
    );
    let text = AppointmentQuery {
        text: Some("ZAHN".into()),
        ..AppointmentQuery::default()
    };
    assert_eq!(
        titles(f.p.list(&text, None, 10).await.unwrap()),
        ["Zahnarzt"]
    );
    let none = AppointmentQuery {
        calendars: vec![fs_calendars_testdata::ids::object_id(99)],
        ..AppointmentQuery::default()
    };
    assert!(titles(f.p.list(&none, None, 10).await.unwrap()).is_empty());
    assert_eq!(f.p.get_by_uid(&a.uid).await.unwrap().unwrap().id, a.id);
}

#[tokio::test]
async fn r7_the_second_writer_gets_changed_and_a_no_op_writes_nothing() {
    let f = fx().await;
    let a = f.make_berlin("alt", 2, 9).await;
    let first =
        f.p.update(a.id, a.revision, title("neu"), SeriesScope::Whole)
            .await
            .unwrap();
    assert_eq!(first.revision.0, a.revision.0 + 1);
    let second =
        f.p.update(a.id, a.revision, title("anders"), SeriesScope::Whole)
            .await;
    assert!(
        matches!(second, Err(Error::Changed { current }) if current == first.revision),
        "{second:?}"
    );
    let log = f.count("activity").await;
    let same =
        f.p.update(a.id, first.revision, title("neu"), SeriesScope::Whole)
            .await
            .unwrap();
    assert_eq!(same.revision, first.revision, "no change, no new revision");
    assert_eq!(f.count("activity").await, log, "and no log entry");
    let d = f.p.delete(a.id, first.revision, SeriesScope::Whole).await;
    assert!(d.is_ok());
    assert!(
        matches!(
            f.p.update(a.id, first.revision, title("x"), SeriesScope::Whole)
                .await,
            Err(Error::NotFound(_))
        ),
        "no write into the trash"
    );
}

#[tokio::test]
async fn r11_the_hull_is_exact_for_timed_and_widened_for_floating_and_all_day() {
    let f = fx().await;
    let t = f
        .make("timed", timed(at(2026, 11, 20, 10, 0), "Europe/Berlin", 60))
        .await;
    let d = f.make("allday", all_day(2026, 11, 20, 2)).await;
    let hull = |id| {
        format!(
            "SELECT when_utc_min, when_utc_max FROM calendar_appointment WHERE id = '{}'",
            fs_calendars_local::codec::id_text(id)
        )
    };
    let h = f.rows(&hull(t.id)).await;
    assert_eq!(
        h[0],
        vec![
            Value::Integer(ms(2026, 11, 20, 9, 0)),
            Value::Integer(ms(2026, 11, 20, 10, 0))
        ]
    );
    let h = f.rows(&hull(d.id)).await;
    let pad = 16 * 3_600_000;
    assert_eq!(
        h[0],
        vec![
            Value::Integer(ms(2026, 11, 20, 0, 0) - pad),
            Value::Integer(ms(2026, 11, 22, 0, 0) + pad)
        ]
    );
    let _ = iana("UTC");
}
