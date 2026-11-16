//! K1: a unit of work that is cancelled (a timeout, a client that goes away) leaves no open transaction behind.
//! `finish` has one suspension point before the COMMIT (see `store.rs`); the second test cancels exactly there.

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

mod common;

use std::time::Duration;

use common::fx;
use fs_calendars_testdata::draft::Appointments;

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

/// Poll a future exactly once with a waker that does nothing, then drop it: a cancel between two awaits.
fn poll_once_and_drop<F: std::future::Future>(f: F) -> bool {
    let mut f = std::pin::pin!(f);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    f.as_mut().poll(&mut cx).is_ready()
}

#[tokio::test]
async fn a_future_cancelled_inside_finish_still_rolls_back_at_the_next_access() {
    let f = fx().await;
    let id = "01927f00-0000-7000-8000-0000000000dd";
    let c = f.p.store().begin().await.unwrap();
    c.exec(
        &format!("INSERT INTO purged_objects (object_id) VALUES ('{id}')"),
        Vec::new(),
    )
    .await
    .unwrap();
    let ready = poll_once_and_drop(c.finish(Ok(())));
    assert!(
        !ready,
        "finish must have a suspension point before the COMMIT, or this test proves nothing"
    );
    assert_eq!(
        f.count("purged_objects").await,
        0,
        "the half finished unit of work is rolled back, not committed"
    );
    f.make_berlin("danach", 2, 9).await;
}
