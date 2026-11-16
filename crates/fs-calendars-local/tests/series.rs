//! Series through the trait: rules, exceptions, scopes, the cut and its restore (core step 2, PLAN sections 5 and 10).

#![allow(
    clippy::many_single_char_names,
    clippy::assert_is_empty,
    unused_imports
)]

mod common;

use common::series::{days, following, rid, starts, this, title, weekly};
use common::{Fx, fx, ms, utc};
use fs_calendars_testdata::draft::{
    Appointment, AppointmentPatch, AppointmentQuery, Appointments, BasicValue, Error, Frequency,
    LocalDateTime, Relation, RuleEnd, SeriesScope, Window,
};
use fs_calendars_testdata::sample::{at, rule, timed};
use fs_calendars_testdata::zone::iana;

#[tokio::test]
async fn the_rules_are_stored_and_the_window_resolves_the_series() {
    let f = fx().await;
    let a = weekly(&f).await;
    assert_eq!(
        f.p.get(a.id).await.unwrap().unwrap().recurrence,
        a.recurrence
    );
    let w = days(&f).await;
    assert_eq!(
        starts(&w),
        ["1", "8", "15", "22", "29"],
        "five Mondays, 08:00 UTC"
    );
    assert!(
        w.instances
            .iter()
            .all(|i| i.recurring && i.recurrence_id.is_some() && !i.overridden)
    );
    let narrow =
        f.p.window(
            utc(ms(2026, 11, 8, 0, 0)),
            utc(ms(2026, 11, 10, 0, 0)),
            &iana("UTC"),
            &[],
        )
        .await
        .unwrap();
    assert_eq!(narrow.instances.len(), 1);
    let after =
        f.p.window(
            utc(ms(2027, 1, 1, 0, 0)),
            utc(ms(2027, 2, 1, 0, 0)),
            &iana("UTC"),
            &[],
        )
        .await
        .unwrap();
    assert_eq!(after.instances.len(), 0, "the series has ended");
    let list =
        f.p.list(
            &AppointmentQuery {
                from: Some(utc(ms(2026, 11, 20, 0, 0))),
                ..AppointmentQuery::default()
            },
            None,
            10,
        )
        .await
        .unwrap();
    assert_eq!(
        (list.items.len(), list.items[0].recurring),
        (1, true),
        "found through the series end, not the first start"
    );
    let gone =
        f.p.list(
            &AppointmentQuery {
                from: Some(utc(ms(2027, 1, 1, 0, 0))),
                ..AppointmentQuery::default()
            },
            None,
            10,
        )
        .await
        .unwrap();
    assert_eq!(gone.items.len(), 0);
}

#[tokio::test]
async fn an_endless_series_stays_visible_far_ahead() {
    let f = fx().await;
    let mut d = f.draft("immer", timed(rid(2), "Europe/Berlin", 60));
    d.rest.recurrence = Some(vec![rule(Frequency::Weekly)]);
    f.p.create(d).await.unwrap();
    let w =
        f.p.window(
            utc(ms(2030, 6, 1, 0, 0)),
            utc(ms(2030, 6, 30, 0, 0)),
            &iana("UTC"),
            &[],
        )
        .await
        .unwrap();
    assert_eq!(w.instances.len(), 4);
    assert_eq!(
        f.rows("SELECT series_end_utc FROM calendar_appointment")
            .await[0][0],
        turso::Value::Null
    );
}

#[tokio::test]
async fn this_changes_one_occurrence_through_an_exception_row() {
    let f = fx().await;
    let a = weekly(&f).await;
    let moved = AppointmentPatch {
        title: Some("verlegt".into()),
        when: Some(timed(at(2026, 11, 9, 10, 30), "Europe/Berlin", 30)),
        ..AppointmentPatch::default()
    };
    let u = f.p.update(a.id, a.revision, moved, this(9)).await.unwrap();
    assert_eq!(u.revision.0, a.revision.0 + 1);
    assert_eq!(
        u.overrides.len(),
        1,
        "one exception, the master is not rewritten"
    );
    assert_eq!(u.title, "Montag");
    assert_eq!(
        f.count("calendar_appointment").await,
        2,
        "master + exception row"
    );
    assert_eq!(
        f.rows("SELECT field FROM calendar_override_field ORDER BY field")
            .await
            .len(),
        2
    );
    let w = days(&f).await;
    let one = w.instances.iter().find(|i| i.overridden).unwrap();
    assert_eq!(
        (one.title.as_str(), one.start.millis()),
        ("verlegt", ms(2026, 11, 9, 9, 30))
    );
    assert_eq!(w.instances.len(), 5);
    let again =
        f.p.update(a.id, u.revision, title("verlegt"), this(9))
            .await
            .unwrap();
    assert_eq!(
        again.revision, u.revision,
        "same change again: nothing is written"
    );
    let h = f.p.history(a.id, None, 10).await.unwrap().items;
    assert!(h[1].changes.iter().any(
        |c| c.field == "override.title" && c.row_key.as_deref() == Some("2026-11-09T09:00:00")
    ));
    let bad = f.p.update(a.id, u.revision, title("x"), this(10)).await;
    assert!(
        matches!(bad, Err(Error::Invalid(_))),
        "the 10th is not an occurrence: {bad:?}"
    );
}

#[tokio::test]
async fn deleting_one_occurrence_and_restoring_it() {
    let f = fx().await;
    let a = weekly(&f).await;
    let d = f.p.delete(a.id, a.revision, this(16)).await.unwrap();
    assert_eq!(
        d.restorable_until,
        Some(utc(ms(2026, 11, 16, 10, 0) + 30 * common::DAY))
    );
    assert_eq!(starts(&days(&f).await), ["1", "8", "22", "29"]);
    let g = f.p.get(a.id).await.unwrap().unwrap();
    assert_eq!(g.overrides.len(), 1);
    let back = f.p.restore(a.id, this(16)).await.unwrap();
    assert!(back.overrides.is_empty());
    assert_eq!(starts(&days(&f).await).len(), 5);
    assert_eq!(
        f.count("calendar_appointment").await,
        1,
        "the exception row is gone"
    );
    assert!(
        matches!(f.p.restore(a.id, this(16)).await, Err(Error::Invalid(_))),
        "nothing was deleted there any more"
    );
}
