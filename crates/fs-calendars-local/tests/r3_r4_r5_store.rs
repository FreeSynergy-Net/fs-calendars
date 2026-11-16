//! R3 (one transaction), R4 (foreign keys, cascade, trash instead of delete), R5 (CHECK and partial unique indexes).

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

mod common;

use common::{Fx, fx, ms, utc};
use fs_calendars_testdata::draft::{
    Alert, AlertAction, AppointmentPatch, Appointments, Attendee, Error, Participant,
    ParticipationStatus, RelativeTo, Revision, Roles, SeriesScope, Trigger,
};
use fs_calendars_testdata::ids::person;
use turso::Value;

fn participant(key: &str, n: u64) -> Participant {
    Participant {
        key: key.to_owned(),
        who: Attendee::Actor(person(n)),
        roles: Roles {
            attendee: true,
            ..Roles::default()
        },
        status: ParticipationStatus::NeedsAction,
        expect_reply: true,
        comment: None,
        send_to: vec![],
        invited_by: None,
        delegated_to: vec![],
        delegated_from: vec![],
        schedule_sequence: 0,
        schedule_updated: None,
    }
}

fn alert(key: &str) -> Alert {
    Alert {
        key: key.to_owned(),
        trigger: Trigger::Offset {
            seconds: -600,
            relative_to: RelativeTo::Start,
        },
        action: AlertAction::Display,
        acknowledged: None,
    }
}

async fn full(f: &Fx) -> fs_calendars_testdata::draft::Appointment {
    let mut d = f.draft(
        "Mit allem",
        fs_calendars_testdata::sample::timed(
            fs_calendars_testdata::sample::at(2026, 12, 1, 9, 0),
            "Europe/Berlin",
            30,
        ),
    );
    d.rest = AppointmentPatch {
        participants: Some(vec![participant("a", 2), participant("b", 3)]),
        alerts: Some(vec![alert("x")]),
        keywords: Some(vec!["k".into()]),
        categories: Some(vec!["c".into()]),
        ..AppointmentPatch::default()
    };
    f.p.create(d).await.unwrap()
}

#[tokio::test]
async fn r3_a_failure_in_a_late_table_leaves_no_row_anywhere() {
    let f = fx().await;
    let before = [
        f.count("objects").await,
        f.count("calendar_appointment").await,
        f.count("activity").await,
        f.count("activity_change").await,
    ];
    let mut d = f.draft(
        "Doppelt",
        fs_calendars_testdata::sample::timed(
            fs_calendars_testdata::sample::at(2026, 12, 1, 9, 0),
            "Europe/Berlin",
            30,
        ),
    );
    d.rest.participants = Some(vec![participant("a", 2), participant("a", 3)]);
    let r = f.p.create(d).await;
    assert!(matches!(r, Err(Error::Invalid(_))), "{r:?}");
    let after = [
        f.count("objects").await,
        f.count("calendar_appointment").await,
        f.count("activity").await,
        f.count("activity_change").await,
    ];
    assert_eq!(
        before, after,
        "envelope, row and log of the failed create are all gone"
    );
    assert_eq!(f.count("calendar_participant").await, 0);
    f.make_berlin("danach", 2, 9).await;
}

#[tokio::test]
async fn r4_the_store_runs_with_foreign_keys_on() {
    let f = fx().await;
    let c = f.p.store().begin().await.unwrap();
    let r = c.exec("INSERT INTO calendar_keyword (appointment, value) VALUES ('01927f00-0000-7000-8000-0000000000aa', 'x')", Vec::new()).await;
    assert!(
        matches!(c.finish(r).await, Err(Error::Invalid(_))),
        "an orphan row is refused"
    );
}

#[tokio::test]
async fn r4_trash_keeps_everything_and_restore_brings_it_back() {
    let f = fx().await;
    let a = full(&f).await;
    let rows = [
        f.count("calendar_participant").await,
        f.count("activity").await,
        f.count("activity_change").await,
    ];
    let d =
        f.p.delete(a.id, a.revision, SeriesScope::Whole)
            .await
            .unwrap();
    assert_eq!(
        f.count("calendar_participant").await,
        rows[0],
        "no child row is removed by the trash"
    );
    assert_eq!(
        f.count("activity_change").await,
        rows[2],
        "the log of the trashed object stays"
    );
    assert_eq!(
        f.count("activity").await,
        rows[1] + 1,
        "the delete is logged"
    );
    assert!(f.p.get(a.id).await.unwrap().is_none());
    assert_eq!(
        d.restorable_until,
        Some(utc(ms(2026, 11, 16, 10, 0) + 30 * common::DAY))
    );
    let back = f.p.restore(a.id, SeriesScope::Whole).await.unwrap();
    assert_eq!(back.participants.len(), 2);
    assert_eq!(back.alerts.len(), 1);
}

#[tokio::test]
async fn r4_final_delete_cascades_through_every_table_and_log_purge_keeps_the_object() {
    let f = fx().await;
    let a = full(&f).await;
    let b = f.make_berlin("bleibt", 3, 9).await;
    let id = fs_calendars_local::codec::id_text(a.id);
    f.p.update(
        a.id,
        a.revision,
        AppointmentPatch {
            title: Some("Neu".into()),
            ..AppointmentPatch::default()
        },
        SeriesScope::Whole,
    )
    .await
    .unwrap();
    let log_before = f.count("activity").await;
    f.exec(&format!("DELETE FROM activity WHERE object = '{id}'"))
        .await
        .unwrap();
    assert_eq!(
        f.count("calendar_appointment").await,
        2,
        "the log purge removes only the log"
    );
    assert!(f.count("activity").await < log_before);
    f.exec(&format!("DELETE FROM objects WHERE id = '{id}'"))
        .await
        .unwrap();
    for t in [
        "calendar_appointment",
        "calendar_participant",
        "calendar_alert",
        "calendar_keyword",
        "calendar_category",
    ] {
        let n = f
            .rows(&format!(
                "SELECT count(*) FROM {t} WHERE {} = '{id}'",
                if t == "calendar_appointment" {
                    "id"
                } else {
                    "appointment"
                }
            ))
            .await;
        assert_eq!(n[0][0], Value::Integer(0), "{t}");
    }
    assert!(
        f.p.get(b.id).await.unwrap().is_some(),
        "the other appointment is untouched"
    );
}

#[tokio::test]
async fn r4_updates_never_wipe_the_history() {
    let f = fx().await;
    let a = f.make_berlin("eins", 2, 9).await;
    let mut rev = a.revision;
    for t in ["zwei", "drei"] {
        rev =
            f.p.update(
                a.id,
                rev,
                AppointmentPatch {
                    title: Some(t.into()),
                    ..AppointmentPatch::default()
                },
                SeriesScope::Whole,
            )
            .await
            .unwrap()
            .revision;
    }
    let h = f.p.history(a.id, None, 10).await.unwrap();
    assert_eq!(
        h.items.len(),
        3,
        "Create + two Update (no DELETE+INSERT, no REPLACE)"
    );
}

#[tokio::test]
async fn r5_check_and_partial_unique_indexes_refuse_violations() {
    let f = fx().await;
    let a = f.make_berlin("a", 2, 9).await;
    let b = f.make_berlin("b", 3, 9).await;
    let c = f.make_berlin("c", 4, 9).await;
    let (ia, ib, ic) = (
        fs_calendars_local::codec::id_text(a.id),
        fs_calendars_local::codec::id_text(b.id),
        fs_calendars_local::codec::id_text(c.id),
    );
    assert!(
        matches!(
            f.p.create({
                let mut d = f.draft("x", a.when.clone());
                d.uid = Some(a.uid.clone());
                d
            })
            .await,
            Err(Error::Invalid(_))
        ),
        "uid exists (provider)"
    );
    let r = f
        .exec(&format!(
            "UPDATE calendar_appointment SET uid = '{}' WHERE id = '{ib}'",
            a.uid
        ))
        .await;
    assert!(
        matches!(r, Err(Error::Invalid(_))),
        "UNIQUE (uid) WHERE master IS NULL in the engine: {r:?}"
    );
    let r = f
        .exec(&format!(
            "UPDATE calendar_appointment SET when_kind = 'all_day' WHERE id = '{ia}'"
        ))
        .await;
    assert!(
        matches!(r, Err(Error::Invalid(_))),
        "CHECK of the WHEN composite: {r:?}"
    );
    f.exec(&format!(
        "UPDATE calendar_appointment SET master = '{ia}', recurrence_id = 1000 WHERE id = '{ib}'"
    ))
    .await
    .unwrap();
    let r = f.exec(&format!("UPDATE calendar_appointment SET master = '{ia}', recurrence_id = 1000 WHERE id = '{ic}'")).await;
    assert!(
        matches!(r, Err(Error::Invalid(_))),
        "UNIQUE (master, recurrence_id) WHERE master IS NOT NULL: {r:?}"
    );
    let r = f.exec(&format!("UPDATE calendar_appointment SET when_len_days = 0, when_kind = 'all_day', when_local = NULL, when_date = 5, when_zone = NULL, when_len_seconds = 0 WHERE id = '{ic}'")).await;
    assert!(
        matches!(r, Err(Error::Invalid(_))),
        "all_day with len_days = 0: {r:?}"
    );
    let _ = Revision(0);
}
