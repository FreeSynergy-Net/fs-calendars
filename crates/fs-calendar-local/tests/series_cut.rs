//! Series through the trait, part 2: the cut and its restore, the split, D8 (core step 2, PLAN sections 5 and 10).

#![allow(
    clippy::many_single_char_names,
    clippy::assert_is_empty,
    unused_imports
)]

mod common;

use common::series::{days, following, rid, starts, this, title, weekly};
use common::{Fx, fx, ms, utc};
use fs_calendar_testdata::draft::{
    Appointment, AppointmentPatch, AppointmentQuery, Appointments, BasicValue, Error, Frequency,
    LocalDateTime, Relation, RuleEnd, SeriesScope, Window,
};
use fs_calendar_testdata::sample::{at, rule, timed};
use fs_calendar_testdata::zone::iana;

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
