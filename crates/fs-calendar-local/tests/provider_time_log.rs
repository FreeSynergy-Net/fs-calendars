//! The window in the viewer's zone, the trash with its period, and the typed history through the trait.

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

mod common;

use common::{DAY, fx, ms, utc};
use fs_calendar_testdata::draft::{
    AppointmentPatch, AppointmentQuery, Appointments, Attendee, BasicValue, Error, HistoryKind,
    Participant, ParticipationStatus, Roles, SeriesScope, Span, Value, When,
};
use fs_calendar_testdata::ids::person;
use fs_calendar_testdata::sample::{all_day, at, timed};
use fs_calendar_testdata::zone::{custom, iana};

#[tokio::test]
async fn the_window_places_floating_and_all_day_per_viewer_and_timed_exactly() {
    let f = fx().await;
    let berlin = f
        .make(
            "Berlin 09:00",
            timed(at(2026, 11, 20, 9, 0), "Europe/Berlin", 60),
        )
        .await;
    let floating = f
        .make(
            "schwebend 08:00",
            When::Floating {
                start: at(2026, 11, 20, 8, 0),
                length: Span {
                    days: 0,
                    seconds: 3600,
                },
            },
        )
        .await;
    let day = f.make("ganztägig", all_day(2026, 11, 20, 1)).await;
    let (from, to) = (utc(ms(2026, 11, 19, 0, 0)), utc(ms(2026, 11, 22, 0, 0)));
    let in_zone = |z: &'static str| {
        let f = &f;
        async move { f.p.window(from, to, &iana(z), &[]).await.unwrap() }
    };
    let b = in_zone("Europe/Berlin").await;
    let n = in_zone("America/New_York").await;
    let start = |w: &fs_calendar_testdata::draft::Window, id| {
        w.instances
            .iter()
            .find(|i| i.appointment == id)
            .unwrap()
            .start
            .millis()
    };
    assert_eq!(
        start(&b, berlin.id),
        ms(2026, 11, 20, 8, 0),
        "timed: the instant does not depend on the viewer"
    );
    assert_eq!(start(&n, berlin.id), ms(2026, 11, 20, 8, 0));
    assert_eq!(
        start(&b, floating.id),
        ms(2026, 11, 20, 7, 0),
        "floating 08:00 is 08:00 in Berlin"
    );
    assert_eq!(
        start(&n, floating.id),
        ms(2026, 11, 20, 13, 0),
        "and 08:00 in New York"
    );
    assert_eq!(start(&b, day.id), ms(2026, 11, 19, 23, 0));
    assert_eq!(start(&n, day.id), ms(2026, 11, 20, 5, 0));
    let narrow =
        f.p.window(
            utc(ms(2026, 11, 20, 8, 30)),
            utc(ms(2026, 11, 20, 8, 45)),
            &iana("Europe/Berlin"),
            &[],
        )
        .await
        .unwrap();
    assert_eq!(
        narrow
            .instances
            .iter()
            .map(|i| i.appointment)
            .collect::<Vec<_>>(),
        [day.id, berlin.id],
        "exact overlap, not the hull; sorted by start"
    );
    let r = f.p.window(from, to, &custom("/Custom/X"), &[]).await;
    assert!(
        matches!(r, Err(Error::UnknownZone(_))),
        "a custom zone is never a view zone"
    );
}

#[tokio::test]
async fn the_window_carries_the_count_and_my_status() {
    let f = fx().await;
    let mut d = f.draft("Runde", timed(at(2026, 11, 20, 9, 0), "Europe/Berlin", 60));
    let p = |key: &str, n, status| Participant {
        key: key.into(),
        who: Attendee::Actor(person(n)),
        roles: Roles::default(),
        status,
        expect_reply: true,
        comment: None,
        send_to: vec![],
        invited_by: None,
        delegated_to: vec![],
        delegated_from: vec![],
        schedule_sequence: 0,
        schedule_updated: None,
    };
    d.rest.participants = Some(vec![
        p("me", 1, ParticipationStatus::Accepted),
        p("ann", 2, ParticipationStatus::Declined),
        p("ben", 3, ParticipationStatus::NeedsAction),
    ]);
    f.p.create(d).await.unwrap();
    f.make_berlin("allein", 20, 15).await;
    let w =
        f.p.window(
            utc(ms(2026, 11, 20, 0, 0)),
            utc(ms(2026, 11, 21, 0, 0)),
            &iana("UTC"),
            &[],
        )
        .await
        .unwrap();
    assert_eq!(w.instances.len(), 2);
    assert_eq!(
        (w.instances[0].participant_count, w.instances[0].my_status),
        (3, Some(ParticipationStatus::Accepted))
    );
    assert_eq!(
        (w.instances[1].participant_count, w.instances[1].my_status),
        (0, None)
    );
    assert!(w.damaged.is_empty());
}

#[tokio::test]
async fn trash_period_restore_and_the_setting() {
    let f = fx().await;
    let a = f.make_berlin("a", 2, 9).await;
    let b = f.make_berlin("b", 3, 9).await;
    f.exec("INSERT INTO setting (key, int_value) VALUES ('trash_days', 7)")
        .await
        .unwrap();
    let da =
        f.p.delete(a.id, a.revision, SeriesScope::Whole)
            .await
            .unwrap();
    assert_eq!(
        da.restorable_until,
        Some(utc(ms(2026, 11, 16, 10, 0) + 7 * DAY)),
        "the period of the setting"
    );
    f.exec("UPDATE setting SET int_value = 30 WHERE key = 'trash_days'")
        .await
        .unwrap();
    let listed =
        f.p.list(
            &AppointmentQuery {
                include_deleted: true,
                ..AppointmentQuery::default()
            },
            None,
            10,
        )
        .await
        .unwrap()
        .items;
    assert_eq!(
        listed
            .iter()
            .find(|s| s.id == a.id)
            .unwrap()
            .restorable_until,
        da.restorable_until,
        "a later change applies only to later deletes"
    );
    assert_eq!(
        listed
            .iter()
            .find(|s| s.id == b.id)
            .unwrap()
            .restorable_until,
        None
    );
    assert_eq!(
        f.p.list(&AppointmentQuery::default(), None, 10)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert!(
        matches!(
            f.p.delete(a.id, a.revision, SeriesScope::Whole).await,
            Err(Error::NotFound(_))
        ),
        "already in the trash"
    );
    assert!(
        matches!(
            f.p.restore(b.id, SeriesScope::Whole).await,
            Err(Error::Invalid(_))
        ),
        "not in the trash"
    );
    f.set_now(ms(2026, 11, 16, 10, 0) + 8 * DAY);
    assert!(
        matches!(
            f.p.restore(a.id, SeriesScope::Whole).await,
            Err(Error::NotFound(_))
        ),
        "the period is over"
    );
    f.set_now(ms(2026, 11, 16, 10, 0) + 3 * DAY);
    let back = f.p.restore(a.id, SeriesScope::Whole).await.unwrap();
    assert_eq!(
        back.revision.0,
        a.revision.0 + 2,
        "delete and restore each count"
    );
    assert_eq!(f.p.get(a.id).await.unwrap().unwrap().title, "a");
}

#[tokio::test]
async fn history_is_typed_oldest_first_and_pages() {
    let f = fx().await;
    let a = f.make_berlin("alt", 2, 9).await;
    f.set_now(ms(2026, 11, 17, 10, 0));
    let u =
        f.p.update(
            a.id,
            a.revision,
            AppointmentPatch {
                title: Some("neu".into()),
                when: Some(timed(at(2026, 11, 2, 10, 30), "Europe/Berlin", 60)),
                ..AppointmentPatch::default()
            },
            SeriesScope::Whole,
        )
        .await
        .unwrap();
    f.p.delete(a.id, u.revision, SeriesScope::Whole)
        .await
        .unwrap();
    let all = f.p.history(a.id, None, 10).await.unwrap().items;
    let kinds: Vec<HistoryKind> = all.iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        [
            HistoryKind::Create,
            HistoryKind::Update,
            HistoryKind::Delete
        ]
    );
    assert_eq!(all[0].actor, common::actor());
    assert_eq!(all[1].at, utc(ms(2026, 11, 17, 10, 0)));
    let title = all[1].changes.iter().find(|c| c.field == "title").unwrap();
    assert_eq!(
        (title.old.clone(), title.new.clone()),
        (
            Some(Value::Text("alt".into())),
            Some(Value::Text("neu".into()))
        )
    );
    let local = all[1]
        .changes
        .iter()
        .find(|c| c.field == "when_local")
        .unwrap();
    assert!(
        matches!((&local.old, &local.new), (Some(Value::Local(_)), Some(Value::Local(n))) if n.canonical() == "2026-11-02T10:30:00")
    );
    assert!(all[1].changes.iter().any(|c| c.field == "sequence"));
    assert!(
        !all[1].changes.iter().any(|c| c.field == "priority"),
        "only what changed"
    );
    let first = f.p.history(a.id, None, 2).await.unwrap();
    assert_eq!(first.items.len(), 2);
    let rest = f.p.history(a.id, first.next.as_ref(), 2).await.unwrap();
    assert_eq!((rest.items.len(), rest.next), (1, None));
    assert!(matches!(
        f.p.history(fs_calendar_testdata::ids::object_id(5), None, 5)
            .await,
        Err(Error::NotFound(_))
    ));
}
