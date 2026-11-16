//! R1 (the calendar registers its domain with the shared layer) and the zone check of R2 that stays with the calendar.
//! The rest of R1/R2 (reserved prefixes, clashes, every column type, steps) is the layer's and has its tests in
//! `fs-objects` (`v5_registry`, `v5_steps`); they are not repeated here.

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

use fs_calendars_local::Store;
use fs_calendars_local::typed::check_zones;
use fs_calendars_testdata::draft::Error;
use fs_objects::v5::Step;
use fs_objects::v5::registry::Domain;
use fs_objects::v5::spec::{
    Col,
    Logical::{Ref, Text, Zone},
    Table,
};
use turso::Value;

const ONE: Table = Table {
    name: "tasks_task",
    cols: &[Col::req("id", Ref), Col::req("title", Text)],
    constraints: &["PRIMARY KEY (id)"],
    indexes: &[],
};
const TASKS: Domain = Domain {
    prefix: "tasks",
    about: "tasks",
    tables: &[ONE],
    channels: &[],
    children: &[],
    expiries: &[],
    steps: &[Step::Create(&[ONE])],
};

#[tokio::test]
async fn r1_the_calendar_domain_is_registered_and_the_hull_is_derived() {
    let s = Store::open(":memory:", 0).await.unwrap();
    let c = s.read().await;
    let doms = c
        .query(
            "SELECT prefix, version FROM fs_domain ORDER BY prefix",
            Vec::new(),
        )
        .await
        .unwrap();
    assert_eq!(doms.len(), 2, "calendar and fs_");
    assert_eq!(doms[0][0], Value::Text("calendar".into()));
    assert_eq!(doms[0][1], Value::Integer(1), "one step: Create");
    let cols = c.query("SELECT logical, derived FROM fs_column WHERE domain = 'calendar' AND col = 'when_utc_min'", Vec::new()).await.unwrap();
    assert_eq!(
        cols[0],
        vec![Value::Text("utc".into()), Value::Integer(1)],
        "the registry knows the hull is derived"
    );
}

#[tokio::test]
async fn r1_a_second_program_cannot_share_the_calendar_store() {
    let s = Store::open(":memory:", 0).await.unwrap();
    let r = s.register_domain(&TASKS, 1).await;
    assert!(
        matches!(&r, Err(Error::Unavailable(m)) if m.contains("cannot join")),
        "LocalTasks needs a store file of its own: {r:?}"
    );
}

#[tokio::test]
async fn r1_opening_the_same_file_twice_is_idempotent() {
    let path = std::env::temp_dir().join(format!("fs-cal-local-r1-{}.db", std::process::id()));
    let p = path.to_str().unwrap();
    for _ in 0..2 {
        Store::open(p, 1).await.unwrap();
    }
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{p}{ext}"));
    }
}

const ZONED: Table = Table {
    name: "calendar_zoned",
    cols: &[Col::req("z", Zone)],
    constraints: &[],
    indexes: &[],
};

#[test]
fn r2_a_zone_that_does_not_exist_is_unknown_zone_never_guessed() {
    assert!(check_zones(&ZONED, &["Europe/Berlin".into()]).is_ok());
    assert!(matches!(
        check_zones(&ZONED, &["Mars/Olympus_Mons".into()]),
        Err(Error::UnknownZone(_))
    ));
    assert!(
        check_zones(&ZONED, &["/Custom/x".into()]).is_ok(),
        "a custom zone is the layer's to refuse (Invalid), not a zone lookup"
    );
}
