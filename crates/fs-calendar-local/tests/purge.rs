//! Review points K6 (purge API, trash period) and the period run (ADR 0073).

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

mod common;

use common::fx;
use fs_calendar_testdata::draft::{Appointments, Error, SeriesScope};
use fs_calendar_testdata::sample::{at, timed};
use turso::Value;

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

#[tokio::test]
async fn the_period_run_forgets_expired_cuts_and_their_hidden_exceptions() {
    use common::series::{days, following, this, title, weekly};
    let f = fx().await;
    let a = weekly(&f).await;
    let u =
        f.p.update(a.id, a.revision, title("Sonder"), this(23))
            .await
            .unwrap();
    let d = f.p.delete(a.id, u.revision, following(16)).await.unwrap();
    let other = weekly(&f).await;
    assert_eq!(f.count("calendar_cut").await, 1);
    assert_eq!(
        f.count("calendar_appointment").await,
        3,
        "two masters, the exception and nothing else yet"
    );
    f.set_now(d.restorable_until.unwrap().millis() - 1);
    assert_eq!(
        f.p.sweep_expired().await.unwrap(),
        fs_calendar_local::sweep::Swept::default(),
        "not over yet"
    );
    f.set_now(d.restorable_until.unwrap().millis());
    let swept = f.p.sweep_expired().await.unwrap();
    assert_eq!(
        (swept.cuts, swept.hidden),
        (1, 1),
        "the bound is closed: `until <= now`"
    );
    assert_eq!(
        f.count("calendar_cut").await + f.count("calendar_cut_rule").await,
        0
    );
    assert_eq!(
        f.count("calendar_appointment").await,
        2,
        "the hidden exception is gone"
    );
    assert_eq!(f.count("calendar_override_field").await, 0);
    assert!(f.p.get(other.id).await.unwrap().is_some());
    let _ = days(&f).await;
    assert_eq!(f.p.sweep_expired().await.unwrap().cuts, 0);
}

#[tokio::test]
async fn the_final_delete_forgets_the_marks_of_the_object() {
    let f = fx().await;
    let a = f.make_berlin("a", 2, 9).await;
    f.p.purge_log(a.id).await.unwrap();
    f.p.mark_unlogged(a.id).await.unwrap();
    f.p.delete(a.id, a.revision, SeriesScope::Whole)
        .await
        .unwrap();
    f.p.delete_final(a.id).await.unwrap();
    assert_eq!(
        f.count("purged_objects").await + f.count("unlogged_ids").await,
        0
    );
}

#[tokio::test]
async fn the_period_run_removes_the_fields_of_a_deleted_changed_occurrence() {
    use common::series::{days, this, title, weekly};
    let f = fx().await;
    let a = weekly(&f).await;
    let u =
        f.p.update(a.id, a.revision, title("Geheim"), this(9))
            .await
            .unwrap();
    let d = f.p.delete(a.id, u.revision, this(9)).await.unwrap();
    assert_eq!(f.count("calendar_override_field").await, 1);
    let ids_before = f.rows("SELECT id FROM objects ORDER BY id").await;
    f.set_now(d.restorable_until.unwrap().millis());
    let swept = f.p.sweep_expired().await.unwrap();
    assert_eq!((swept.cuts, swept.scrubbed), (1, 1));
    assert_eq!(f.count("calendar_override_field").await, 0);
    assert_eq!(
        f.rows("SELECT id FROM objects ORDER BY id").await,
        ids_before,
        "changed in place: no new envelope, no lost one"
    );
    let rows = f
        .rows("SELECT title FROM calendar_appointment WHERE master IS NOT NULL")
        .await;
    assert_eq!(
        rows,
        [[turso::Value::Text("Montag".into())]],
        "the changed title is gone from the row"
    );
    assert_eq!(
        days(&f).await.instances.len(),
        4,
        "the occurrence stays deleted"
    );
    assert_eq!(f.p.get(a.id).await.unwrap().unwrap().overrides.len(), 1);
}
