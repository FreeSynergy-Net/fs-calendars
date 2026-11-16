//! R1 (a program registers a domain) and R2 (column types are checked on write), draft section 4.

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

mod common;

use common::{Fx, fx};
use fs_calendar_local::Store;
use fs_calendar_local::spec::{
    Col,
    Logical::{Bool, Date, Enum, Int, Iri, Local, Real, Ref, Text, Utc, Zone},
    Table,
};
use fs_calendar_testdata::draft::{BasicValue, Date as D, Error};
use turso::Value;

const ONE: Table = Table {
    name: "tasks_task",
    cols: &[Col::req("id", Ref), Col::req("title", Text)],
    constraints: &["PRIMARY KEY (id)"],
    indexes: &[],
};
const OTHER: Table = Table {
    name: "tasks_label",
    cols: &[Col::req("id", Ref)],
    constraints: &["PRIMARY KEY (id)"],
    indexes: &[],
};
const OUTSIDE: Table = Table {
    name: "other_thing",
    cols: &[Col::req("id", Ref)],
    constraints: &["PRIMARY KEY (id)"],
    indexes: &[],
};

fn is_invalid(r: &Result<(), Error>, field: &str) -> bool {
    matches!(r, Err(Error::Invalid(p)) if p.iter().any(|x| x.field.contains(field)))
}

#[tokio::test]
async fn r1_two_domains_idempotent_and_clashes_refused() {
    let s = Store::open(":memory:", 0).await.unwrap();
    s.register_domain("tasks", 1, "tasks", &[ONE], 5)
        .await
        .unwrap();
    s.register_domain(
        "calendar",
        1,
        "calendar",
        &fs_calendar_local::calendar_tables::ALL,
        5,
    )
    .await
    .unwrap();
    s.register_domain("tasks", 1, "tasks", &[ONE], 6)
        .await
        .unwrap();
    let c = s.read().await;
    let doms = c
        .query(
            "SELECT prefix, version FROM fs_domain ORDER BY prefix",
            Vec::new(),
        )
        .await
        .unwrap();
    assert_eq!(doms.len(), 3, "calendar, fs_, tasks");
    let cols = c.query("SELECT logical, derived FROM fs_column WHERE domain = 'calendar' AND col = 'when_utc_min'", Vec::new()).await.unwrap();
    assert_eq!(
        cols[0],
        vec![Value::Text("utc".into()), Value::Integer(1)],
        "the registry knows the hull is derived"
    );
    drop(c);
    assert!(
        is_invalid(
            &s.register_domain("tasks", 1, "x", &[ONE, OTHER], 7).await,
            "prefix"
        ),
        "other table list under a used prefix"
    );
    assert!(
        is_invalid(
            &s.register_domain("tasks", 2, "x", &[ONE], 7).await,
            "prefix"
        ),
        "other version"
    );
    for reserved in ["object", "activity", "person", "fs"] {
        assert!(
            is_invalid(
                &s.register_domain(reserved, 1, "x", &[ONE], 7).await,
                "prefix"
            ),
            "{reserved}"
        );
    }
    assert!(
        is_invalid(
            &s.register_domain("shop", 1, "x", &[OUTSIDE], 7).await,
            "tables"
        ),
        "a table outside the prefix"
    );
}

#[tokio::test]
async fn r1_opening_the_same_file_twice_is_idempotent() {
    let path = std::env::temp_dir().join(format!("fs-cal-local-r1-{}.db", std::process::id()));
    let p = path.to_str().unwrap();
    for _ in 0..2 {
        let s = Store::open(p, 1).await.unwrap();
        s.register_domain(
            "calendar",
            1,
            "calendar",
            &fs_calendar_local::calendar_tables::ALL,
            1,
        )
        .await
        .unwrap();
    }
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{p}{ext}"));
    }
}

const TYPED: Table = Table {
    name: "tasks_typed",
    cols: &[
        Col::req("id", Ref),
        Col::req("u", Utc),
        Col::req("l", Local),
        Col::req("d", Date),
        Col::req("z", Zone),
        Col::req("e", Enum(&["a", "b"])),
        Col::req("i", Iri),
        Col::opt("r", Real),
        Col::req("b", Bool),
        Col::req("n", Int),
    ],
    constraints: &["PRIMARY KEY (id)"],
    indexes: &[],
};
const ID: &str = "01927f00-0000-7000-8000-000000000001";

fn good() -> Vec<Value> {
    vec![
        ID.into(),
        Value::Integer(1_000),
        Value::Integer(86_400),
        Value::Integer(20_000),
        "Europe/Berlin".into(),
        "a".into(),
        "urn:uuid:x".into(),
        Value::Real(1.5),
        Value::Integer(1),
        Value::Integer(-7),
    ]
}

async fn put(f: &Fx, row: Vec<Value>) -> Result<(), Error> {
    let c = f.p.store().begin().await?;
    let r = c.insert(&TYPED, row).await;
    c.finish(r).await
}

#[tokio::test]
async fn r2_each_type_stores_a_good_value_and_refuses_a_bad_one_naming_the_field() {
    let f = fx().await;
    f.p.store()
        .register_domain("tasks", 1, "t", &[TYPED], 0)
        .await
        .unwrap();
    put(&f, good()).await.unwrap();
    let back = f
        .rows("SELECT u, l, d, z, e, i, r, b, n FROM tasks_typed")
        .await;
    assert_eq!(
        back[0][..4],
        [
            Value::Integer(1_000),
            Value::Integer(86_400),
            Value::Integer(20_000),
            Value::Text("Europe/Berlin".into())
        ]
    );
    assert_eq!(back[0][6], Value::Real(1.5));
    let bad: [(usize, Value, &str); 13] = [
        (0, "not-a-uuid".into(), "id"),
        (2, Value::Integer(i64::MAX), "l"),
        (3, Value::Integer(i64::MAX), "d"),
        (4, "".into(), "z"),
        (4, "/Custom/x".into(), "z"),
        (5, "c".into(), "e"),
        (6, "no scheme".into(), "i"),
        (7, Value::Real(f64::NAN), "r"),
        (7, Value::Real(f64::INFINITY), "r"),
        (8, Value::Integer(2), "b"),
        (9, Value::Null, "n"),
        (1, "text".into(), "u"),
        (3, "x".into(), "d"),
    ];
    for (i, v, field) in bad {
        let mut row = good();
        row[0] = format!("01927f00-0000-7000-8000-0000000000{:02}", i + 10)
            .as_str()
            .into();
        row[i] = v.clone();
        let r = put(&f, row).await;
        assert!(is_invalid(&r, field), "{field} {v:?}: {r:?}");
    }
    let mut zone = good();
    zone[0] = "01927f00-0000-7000-8000-000000000099".into();
    zone[4] = "Mars/Olympus_Mons".into();
    assert!(
        matches!(put(&f, zone).await, Err(Error::UnknownZone(_))),
        "a zone that does not exist is UnknownZone, never guessed"
    );
    assert_eq!(
        f.count("tasks_typed").await,
        1,
        "nothing of the refused rows was stored"
    );
    assert!(D::parse("2026-02-30").is_err(), "30 February is not a date");
}
