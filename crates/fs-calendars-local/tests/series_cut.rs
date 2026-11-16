//! Series through the trait, part 2: the cut and its restore, the split, D8 (core step 2, PLAN sections 5 and 10).

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
async fn cutting_the_series_is_undoable_and_restore_reports_changed_after_a_later_write() {
    let f = fx().await;
    let a = weekly(&f).await;
    f.p.update(a.id, a.revision, title("Zwei"), this(23))
        .await
        .unwrap();
    let cur = f.p.get(a.id).await.unwrap().unwrap();
    f.p.delete(a.id, cur.revision, following(16)).await.unwrap();
    assert_eq!(
        starts(&days(&f).await),
        ["1", "8"],
        "the cut series ends before the 16th"
    );
    let cut = f.p.get(a.id).await.unwrap().unwrap();
    assert!(
        cut.overrides.is_empty(),
        "the exception after the cut is hidden"
    );
    let back = f.p.restore(a.id, following(16)).await.unwrap();
    assert_eq!(
        back.recurrence, a.recurrence,
        "the old end of the rule is back"
    );
    assert_eq!(back.overrides.len(), 1, "and the hidden exception");
    assert_eq!(days(&f).await.instances.len(), 5);
    // a second cut, then a change of the series, then restore: Changed, nothing is overwritten
    f.p.delete(a.id, back.revision, following(16))
        .await
        .unwrap();
    let now = f.p.get(a.id).await.unwrap().unwrap();
    let changed =
        f.p.update(a.id, now.revision, title("Neu"), SeriesScope::Whole)
            .await
            .unwrap();
    let r = f.p.restore(a.id, following(16)).await;
    assert!(
        matches!(r, Err(Error::Changed { current }) if current == changed.revision),
        "{r:?}"
    );
    assert_eq!(
        f.p.get(a.id).await.unwrap().unwrap().recurrence,
        now.recurrence,
        "still cut"
    );
    f.set_now(ms(2026, 11, 16, 10, 0) + 31 * common::DAY);
    assert!(
        matches!(
            f.p.restore(a.id, following(16)).await,
            Err(Error::NotFound(_))
        ),
        "the period is over"
    );
}

#[tokio::test]
async fn this_and_following_splits_the_series() {
    let f = fx().await;
    let a = weekly(&f).await;
    f.p.update(a.id, a.revision, title("Sonderfall"), this(23))
        .await
        .unwrap();
    let cur = f.p.get(a.id).await.unwrap().unwrap();
    let tail =
        f.p.update(a.id, cur.revision, title("Neu"), following(16))
            .await
            .unwrap();
    assert_ne!(tail.id, a.id);
    assert_ne!(tail.uid, a.uid);
    assert_eq!(
        tail.recurrence[0].end,
        RuleEnd::Count(3),
        "the count follows the split"
    );
    assert_eq!(
        tail.overrides.len(),
        1,
        "the exception of the 23rd moved to the tail"
    );
    assert!(
        tail.related
            .iter()
            .any(|r| r.relation == Relation::First && r.uid == a.uid)
    );
    let head = f.p.get(a.id).await.unwrap().unwrap();
    assert!(
        head.related
            .iter()
            .any(|r| r.relation == Relation::Next && r.uid == tail.uid)
    );
    assert!(head.overrides.is_empty());
    assert!(
        matches!(head.recurrence[0].end, RuleEnd::Until(u) if u.canonical() == "2026-11-16T08:59:59")
    );
    let w = days(&f).await;
    let titles: Vec<&str> = w.instances.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(titles, ["Montag", "Montag", "Neu", "Sonderfall", "Neu"]);
    assert_eq!(f.p.history(tail.id, None, 5).await.unwrap().items.len(), 1);
}

#[tokio::test]
async fn this_and_following_at_the_first_occurrence_acts_like_the_whole_series() {
    let f = fx().await;
    let a = weekly(&f).await;
    let u =
        f.p.update(a.id, a.revision, title("Alle"), following(2))
            .await
            .unwrap();
    assert_eq!(
        (u.id, u.uid.as_str()),
        (a.id, a.uid.as_str()),
        "D8: no split"
    );
    assert_eq!(f.count("calendar_appointment").await, 1);
    let d = f.p.delete(a.id, u.revision, following(2)).await.unwrap();
    assert!(d.restorable_until.is_some());
    assert!(
        f.p.get(a.id).await.unwrap().is_none(),
        "the whole series is in the trash"
    );
    assert_eq!(
        f.p.restore(a.id, following(2)).await.unwrap().id,
        a.id,
        "any scope takes a trashed series back"
    );
}

#[tokio::test]
async fn deleting_a_changed_occurrence_and_restoring_it_gives_the_change_back() {
    let f = fx().await;
    let a = weekly(&f).await;
    let moved = AppointmentPatch {
        title: Some("verlegt".into()),
        ..AppointmentPatch::default()
    };
    let u = f.p.update(a.id, a.revision, moved, this(9)).await.unwrap();
    f.p.delete(a.id, u.revision, this(9)).await.unwrap();
    assert_eq!(starts(&days(&f).await).len(), 4);
    let back = f.p.restore(a.id, this(9)).await.unwrap();
    assert_eq!(
        back.overrides, u.overrides,
        "the change of the 9th is not lost"
    );
    let w = days(&f).await;
    assert_eq!(
        w.instances.iter().filter(|i| i.title == "verlegt").count(),
        1
    );
    assert_eq!(f.count("calendar_appointment").await, 2);
}

#[tokio::test]
async fn the_split_counts_every_rule_and_logs_the_head_fully() {
    let f = fx().await;
    let mut d = f.draft("Zwei Regeln", timed(rid(2), "Europe/Berlin", 60));
    let (mut a1, mut a2) = (rule(Frequency::Weekly), rule(Frequency::Daily));
    (a1.end, a2.end) = (RuleEnd::Count(6), RuleEnd::Count(10));
    d.rest.recurrence = Some(vec![a1, a2]);
    let a = f.p.create(d).await.unwrap();
    let tail =
        f.p.update(a.id, a.revision, title("Rest"), following(9))
            .await
            .unwrap();
    assert_eq!(
        tail.recurrence.iter().map(|r| r.end).collect::<Vec<_>>(),
        [RuleEnd::Count(5), RuleEnd::Count(3)],
        "weekly 2,9,.. leaves 5 of 6; daily 2..8 are 7 before the 9th, 10 - 7 = 3 left"
    );
    let h = f.p.history(a.id, None, 10).await.unwrap().items;
    let fields: Vec<&str> = h[1].changes.iter().map(|c| c.field.as_str()).collect();
    assert!(
        fields.contains(&"related") && fields.contains(&"sequence"),
        "{fields:?}"
    );
    assert!(
        fields.iter().any(|f| f.starts_with("rule.until")),
        "{fields:?}"
    );
}

#[tokio::test]
async fn a_failing_split_leaves_the_series_as_it_was() {
    let f = fx().await;
    let a = weekly(&f).await;
    // The tail's rule row will collide with the head's: the failure comes late, after the head was written.
    f.exec("CREATE UNIQUE INDEX one_rule ON calendar_rule (frequency, excluded)")
        .await
        .unwrap();
    let before = (
        f.count("activity").await,
        f.count("objects").await,
        f.count("calendar_rule").await,
    );
    let r =
        f.p.update(a.id, a.revision, title("Rest"), following(16))
            .await;
    assert!(matches!(r, Err(Error::Invalid(_))), "{r:?}");
    assert_eq!(
        f.p.get(a.id).await.unwrap().unwrap(),
        a,
        "nothing of the split is left on the series"
    );
    assert_eq!(
        before,
        (
            f.count("activity").await,
            f.count("objects").await,
            f.count("calendar_rule").await
        )
    );
    f.exec("DROP INDEX one_rule").await.unwrap();
    assert!(
        f.p.update(a.id, a.revision, title("Rest"), following(16))
            .await
            .is_ok(),
        "and the next try works"
    );
}
