//! Review points K1 (cancelled futures), K4 (custom zones), K5 (window faults, Unicode text), K6 (purge API).

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

mod common;

use std::time::Duration;

use common::{fx, ms, utc};
use fs_calendar_testdata::draft::{AppointmentQuery, Appointments, Error, SeriesScope, Span, When};
use fs_calendar_testdata::sample::{at, timed};
use fs_calendar_testdata::zone::{custom, iana};
use turso::Value;

#[tokio::test]
async fn a_cancelled_future_leaves_no_open_transaction_behind() {
    let f = fx().await;
    let id = "01927f00-0000-7000-8000-0000000000ee";
    let r = tokio::time::timeout(Duration::from_millis(50), async {
        let c = f.p.store().begin().await.unwrap();
        c.exec(
            &format!("INSERT INTO purged_objects (object_id) VALUES ('{id}')"),
            Vec::new(),
        )
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_secs(30)).await;
        c.finish(Ok(())).await
    })
    .await;
    assert!(r.is_err(), "the future was cancelled");
    assert_eq!(
        f.count("purged_objects").await,
        0,
        "a read after the cancel sees no half write"
    );
    let a = f.make_berlin("danach", 2, 9).await;
    assert_eq!(f.count("purged_objects").await, 0);
    assert_eq!(
        f.p.get(a.id).await.unwrap().unwrap().title,
        "danach",
        "the next write works"
    );
    let dropped = f.p.store().begin().await.unwrap();
    drop(dropped);
    f.make_berlin("und noch eins", 3, 9).await;
}

#[tokio::test]
async fn custom_zones_are_refused_as_unsupported_everywhere() {
    let f = fx().await;
    let when = When::Timed {
        start: at(2026, 11, 20, 9, 0),
        zone: custom("/Custom/X"),
        length: Span {
            days: 0,
            seconds: 60,
        },
        end_zone: None,
        zone_assumed: false,
    };
    let r = f.p.create(f.draft("x", when)).await;
    assert!(
        matches!(r, Err(Error::Unsupported { ref capability, .. }) if capability == "event.create"),
        "{r:?}"
    );
    let c =
        f.p.create_calendar("K", None, Some(custom("/Custom/Y")))
            .await;
    assert!(
        matches!(c, Err(Error::Unsupported { ref capability, .. }) if capability == "calendar.create"),
        "{c:?}"
    );
    assert_eq!(f.p.calendars().await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_length_zero_appointment_at_the_window_start_belongs_to_it() {
    let f = fx().await;
    let w0 = When::Timed {
        start: at(2026, 11, 20, 9, 0),
        zone: iana("UTC"),
        length: Span {
            days: 0,
            seconds: 0,
        },
        end_zone: None,
        zone_assumed: false,
    };
    let a = f.make("Moment", w0).await;
    let from = utc(ms(2026, 11, 20, 9, 0));
    let w =
        f.p.window(from, utc(ms(2026, 11, 20, 10, 0)), &iana("UTC"), &[])
            .await
            .unwrap();
    assert_eq!(
        w.instances
            .iter()
            .map(|i| i.appointment)
            .collect::<Vec<_>>(),
        [a.id]
    );
    let before =
        f.p.window(
            utc(ms(2026, 11, 20, 9, 1)),
            utc(ms(2026, 11, 20, 10, 0)),
            &iana("UTC"),
            &[],
        )
        .await
        .unwrap();
    assert_eq!(before.instances.len(), 0);
}

#[tokio::test]
async fn a_damaged_row_costs_only_itself_in_the_window() {
    let f = fx().await;
    let good = f.make_berlin("gut", 20, 9).await;
    let bad = f.make_berlin("kaputt", 20, 10).await;
    f.exec(&format!(
        "UPDATE calendar_appointment SET when_zone = 'Mars/Base' WHERE id = '{}'",
        fs_calendar_local::codec::id_text(bad.id)
    ))
    .await
    .unwrap();
    f.exec(&format!(
        "UPDATE calendar_appointment SET status = 'weird' WHERE id = '{}'",
        fs_calendar_local::codec::id_text(good.id)
    ))
    .await
    .unwrap_err();
    let w =
        f.p.window(
            utc(ms(2026, 11, 20, 0, 0)),
            utc(ms(2026, 11, 21, 0, 0)),
            &iana("UTC"),
            &[],
        )
        .await
        .unwrap();
    assert_eq!(
        w.instances
            .iter()
            .map(|i| i.appointment)
            .collect::<Vec<_>>(),
        [good.id]
    );
    assert_eq!(w.damaged.len(), 1);
    assert_eq!(w.damaged[0].appointment, bad.id);
}

#[tokio::test]
async fn text_search_lower_cases_the_same_way_on_both_sides() {
    let f = fx().await;
    f.make_berlin("Ärzte-Treffen", 2, 9).await;
    f.make_berlin("Straße", 3, 9).await;
    for needle in ["ärzte", "ÄRZTE", "Ärz"] {
        let q = AppointmentQuery {
            text: Some(needle.to_owned()),
            ..AppointmentQuery::default()
        };
        assert_eq!(
            f.p.list(&q, None, 10).await.unwrap().items.len(),
            1,
            "{needle}"
        );
    }
}

#[tokio::test]
async fn purge_functions_keep_and_remove_what_the_adr_says() {
    let f = fx().await;
    let a = f.make_berlin("a", 2, 9).await;
    let id = fs_calendar_local::codec::id_text(a.id);
    let high = f
        .rows("SELECT value FROM meta WHERE key = 'activity_high'")
        .await;
    assert!(matches!(high[0][0], Value::Text(_)));
    assert_eq!(
        f.rows("SELECT key FROM meta WHERE key = 'store_id'")
            .await
            .len(),
        1
    );
    f.p.purge_log(a.id).await.unwrap();
    assert_eq!(
        f.count("activity").await,
        1,
        "only the log of the calendar is left"
    );
    assert_eq!(
        f.p.get(a.id).await.unwrap().unwrap().title,
        "a",
        "the object stays"
    );
    assert_eq!(
        f.rows(&format!(
            "SELECT 1 FROM purged_objects WHERE object_id = '{id}'"
        ))
        .await
        .len(),
        1
    );
    f.p.purge_log(a.id).await.unwrap();
    assert!(
        matches!(f.p.delete_final(a.id).await, Err(Error::Invalid(_))),
        "only from the trash"
    );
    f.p.delete(a.id, a.revision, SeriesScope::Whole)
        .await
        .unwrap();
    f.p.delete_final(a.id).await.unwrap();
    assert_eq!(f.count("calendar_appointment").await, 0);
    assert_eq!(
        f.rows(&format!("SELECT 1 FROM objects WHERE id = '{id}'"))
            .await
            .len(),
        0
    );
    assert!(matches!(
        f.p.delete_final(a.id).await,
        Err(Error::NotFound(_))
    ));
    let b = f.make_berlin("b", 3, 9).await;
    f.p.mark_unlogged(b.id).await.unwrap();
    f.p.update(
        b.id,
        b.revision,
        fs_calendar_testdata::draft::AppointmentPatch {
            title: Some("c".into()),
            ..Default::default()
        },
        SeriesScope::Whole,
    )
    .await
    .unwrap();
    assert_eq!(
        f.p.history(b.id, None, 10).await.unwrap().items.len(),
        1,
        "later writes are not logged"
    );
}

#[tokio::test]
async fn the_trash_period_must_be_positive_and_series_go_with_their_exceptions() {
    let f = fx().await;
    assert!(matches!(
        f.exec("INSERT INTO setting (key, int_value) VALUES ('trash_days', 0)")
            .await,
        Err(Error::Invalid(_))
    ));
    assert!(
        f.exec("INSERT INTO setting (key, int_value) VALUES ('other', 0)")
            .await
            .is_ok()
    );
    let mut d = f.draft("S", timed(at(2026, 11, 2, 9, 0), "Europe/Berlin", 60));
    let mut r = fs_calendar_testdata::sample::rule(fs_calendar_testdata::draft::Frequency::Weekly);
    r.end = fs_calendar_testdata::draft::RuleEnd::Count(3);
    d.rest.recurrence = Some(vec![r]);
    let s = f.p.create(d).await.unwrap();
    let scope = SeriesScope::This {
        recurrence_id: at(2026, 11, 9, 9, 0),
    };
    let u =
        f.p.update(
            s.id,
            s.revision,
            fs_calendar_testdata::draft::AppointmentPatch {
                title: Some("x".into()),
                ..Default::default()
            },
            scope,
        )
        .await
        .unwrap();
    let before = f.count("objects").await;
    f.p.delete(s.id, u.revision, SeriesScope::Whole)
        .await
        .unwrap();
    f.p.delete_final(s.id).await.unwrap();
    assert_eq!(
        f.count("objects").await,
        before - 2,
        "master and exception envelope"
    );
    assert_eq!(f.count("calendar_appointment").await, 0);
    assert_eq!(
        f.count("calendar_rule").await + f.count("calendar_override_field").await,
        0
    );
}
