//! Fixture test of the step 1 -> step 2 migration (registry.rs, ADR 0086): a store file written at Stand 1 (the
//! domain with its first step only, a calendar in it) is opened by this version: the step is applied, the old rows
//! stay readable, the new tables work.

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

use fs_calendars_local::calendar_tables::{PREFIX, STEP1};
use fs_calendars_local::codec::id_value;
use fs_calendars_local::{Clock, LocalAppointments, Store};
use fs_calendars_testdata::draft::{Appointments, BasicValue, Trash};
use fs_calendars_testdata::ids::{object_id, person};
use fs_objects::v5::Step;
use fs_objects::v5::registry::Domain;
use turso::Value;

const STAND_1: Domain = Domain::new(PREFIX, "calendar, Stand 1", &STEP1, &[Step::Create(&STEP1)]);

fn remove(path: &std::path::Path) {
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
}

#[tokio::test]
async fn a_store_of_stand_1_is_migrated_to_stand_2_and_keeps_its_calendar() {
    let path = std::env::temp_dir().join(format!("fs-cal-local-mig2-{}.db", std::process::id()));
    let p = path.to_str().unwrap();
    let old = object_id(77);
    {
        let s = Store::open_bare(p, 1).await.unwrap();
        s.register_domain(&STAND_1, 1).await.unwrap();
        let c = s.begin().await.unwrap();
        let made = async {
            c.exec(
                "INSERT INTO objects (id, type, name, name_sort, published, updated, rev) VALUES (?, 'Calendar', 'Alt', 'alt', 1, 1, 1)",
                vec![id_value(old)],
            )
            .await?;
            c.exec(
                "INSERT INTO calendar_collection (id, name, owner_id, owner_art) VALUES (?, 'Alt', ?, 'profile')",
                vec![id_value(old), Value::Text(BasicValue::canonical(&person(1).id))],
            )
            .await
        }
        .await;
        c.finish(made).await.unwrap();
        let v = s.read().await;
        let rows = v
            .query(
                "SELECT version FROM fs_domain WHERE prefix = 'calendar'",
                Vec::new(),
            )
            .await
            .unwrap();
        assert_eq!(rows[0][0], Value::Integer(1), "Stand 1 on disk");
    }
    let local = LocalAppointments::open(
        p,
        person(1),
        Clock::from_counter(std::sync::Arc::new(1.into())),
    )
    .await
    .unwrap();
    let v = local.store().read().await;
    let rows = v
        .query(
            "SELECT version FROM fs_domain WHERE prefix = 'calendar'",
            Vec::new(),
        )
        .await
        .unwrap();
    assert_eq!(rows[0][0], Value::Integer(2), "step 2 applied");
    for t in ["calendar_rule_original", "calendar_cascade"] {
        let n = v
            .query(&format!("SELECT count(*) FROM {t}"), Vec::new())
            .await;
        assert!(n.is_ok(), "{t} exists after the migration");
    }
    drop(v);
    let cals = local.calendars(Trash::Include).await.unwrap();
    assert!(
        cals.iter().any(|c| c.id == old && c.name == "Alt"),
        "the calendar of Stand 1 is still there: {cals:?}"
    );
    drop(local);
    remove(&path);
}
