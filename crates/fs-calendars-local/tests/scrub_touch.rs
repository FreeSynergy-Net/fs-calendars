//! B3 (kern-3c) on the shared layer: a period-run hook that changes the data of an object carries the envelope along
//! with the layer's `touch` (revision, `updated`, `name`, `name_sort`); the old title must not stay behind.

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

mod common;

use common::fx;
use fs_calendars_testdata::draft::Appointments;
use fs_calendars_testdata::ids::end_of;
use turso::Value;

#[tokio::test]
async fn the_scrub_of_an_expired_cut_touches_the_envelope_of_the_exception() {
    use common::series::{this, title, weekly};
    let f = fx().await;
    let a = weekly(&f).await;
    let u =
        f.p.update(a.id, a.revision, title("Geheim"), this(9))
            .await
            .unwrap();
    let d = f.p.delete(a.id, u.revision, this(9)).await.unwrap();
    let env = "SELECT name, name_sort, rev, updated FROM objects WHERE type = 'EventException'";
    let before = f.rows(env).await;
    assert_eq!(before[0][0], Value::Text("Geheim".into()));
    f.set_now(end_of(d.restorable).unwrap().millis());
    f.p.sweep_expired().await.unwrap();
    let after = f.rows(env).await;
    assert_eq!(after.len(), 1, "changed in place");
    assert_eq!(after[0][0], Value::Text("Montag".into()), "name");
    assert_eq!(after[0][1], Value::Text("montag".into()), "name_sort");
    let (Value::Integer(r0), Value::Integer(r1)) = (&before[0][2], &after[0][2]) else {
        panic!("rev is an integer");
    };
    assert_eq!(*r1, r0 + 1, "the scrub is a revision of the envelope");
    assert_eq!(
        after[0][3],
        Value::Integer(end_of(d.restorable).unwrap().millis()),
        "updated is the time of the run"
    );
}
