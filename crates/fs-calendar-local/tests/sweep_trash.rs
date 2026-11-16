//! The period run and the trash: an object whose trash period is over is removed for good, with everything that hangs
//! on it (review L3: `Swept::objects`), also a collection with appointments (the foreign key has no `CASCADE`).

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

mod common;

use common::fx;
use fs_calendar_local::codec::id_text;
use fs_calendar_testdata::draft::{Appointments, Error, SeriesScope};

#[tokio::test]
async fn a_trashed_object_goes_for_good_exactly_when_its_period_ends() {
    let f = fx().await;
    let a = f.make_berlin("a", 2, 9).await;
    let other = f.make_berlin("b", 3, 9).await;
    let d =
        f.p.delete(a.id, a.revision, SeriesScope::Whole)
            .await
            .unwrap();
    let until = d.restorable_until.unwrap().millis();
    f.set_now(until - 1);
    let early = f.p.sweep_expired().await.unwrap();
    assert_eq!(early.objects, 0, "nothing goes before the promised time");
    assert_eq!(f.count("calendar_appointment").await, 2);
    f.set_now(until);
    let swept = f.p.sweep_expired().await.unwrap();
    assert_eq!(swept.objects, 1, "the bound is closed: `until <= now`");
    assert_eq!(f.count("calendar_appointment").await, 1);
    assert!(f.p.get(other.id).await.unwrap().is_some());
    assert!(matches!(
        f.p.restore(a.id, SeriesScope::Whole).await,
        Err(Error::NotFound(_))
    ));
    let id = id_text(a.id);
    assert_eq!(
        f.rows(&format!("SELECT 1 FROM activity WHERE object = '{id}'"))
            .await
            .len(),
        0,
        "no log entry of the gone object is left"
    );
    assert_eq!(f.p.sweep_expired().await.unwrap().objects, 0, "once only");
}

#[tokio::test]
async fn a_collection_with_appointments_that_ended_its_trash_period_does_not_block_the_run() {
    let f = fx().await;
    let a = f.make_berlin("a", 2, 9).await;
    let keep = f.p.create_calendar("Zweiter", None, None).await.unwrap().id;
    let id = id_text(f.cal);
    // No program function deletes a collection yet; the layer's trash is set the way it would.
    f.exec(&format!(
        "UPDATE objects SET deleted = 1, purge_after = 5 WHERE id = '{id}'"
    ))
    .await
    .unwrap();
    f.set_now(10);
    let swept = f.p.sweep_expired().await.unwrap();
    assert_eq!(swept.objects, 1, "the collection");
    assert_eq!(f.count("calendar_collection").await, 1, "the other stays");
    assert_eq!(
        f.count("calendar_appointment").await,
        0,
        "its appointments went with it"
    );
    assert!(f.p.get(a.id).await.unwrap().is_none());
    assert_eq!(f.count("objects WHERE type = 'Calendar'").await, 1);
    let _ = keep;
}
