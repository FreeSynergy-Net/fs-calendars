//! Undo of a partial delete (`restore` with the scope of the delete) and `respond` with `expected`.
#![allow(clippy::many_single_char_names)]

mod common;

use common::{loaded, utc, window};
use fs_calendars_testdata::draft::{Answer, Appointments, Error, SeriesScope};
use fs_calendars_testdata::ids::person;
use fs_calendars_testdata::sample::at;

fn count(
    w: &fs_calendars_testdata::draft::Window,
    id: fs_calendars_testdata::draft::ObjectId,
) -> usize {
    w.instances.iter().filter(|i| i.appointment == id).count()
}

#[tokio::test]
async fn restore_this_takes_the_excluded_override_back() {
    let (s, f) = loaded();
    let a = s.get("weekly");
    let (from, to) = (utc(2026, 11, 1, 0, 0), utc(2027, 6, 1, 0, 0));
    let before = count(&window(&f, from, to).await, a.id);
    let scope = SeriesScope::This {
        recurrence_id: at(2026, 11, 16, 10, 0),
    };
    let d = f.delete(a.id, a.revision, scope).await.unwrap();
    assert!(d.restorable_until.is_some());
    assert_eq!(count(&window(&f, from, to).await, a.id), before - 1);
    f.restore(a.id, scope).await.unwrap();
    assert_eq!(count(&window(&f, from, to).await, a.id), before);
    assert!(matches!(
        f.restore(a.id, scope).await,
        Err(Error::Invalid(_))
    ));
}

#[tokio::test]
async fn restore_this_and_following_gives_the_series_back() {
    let (s, f) = loaded();
    let a = s.get("weekly");
    let (from, to) = (utc(2026, 11, 1, 0, 0), utc(2027, 6, 1, 0, 0));
    let before = count(&window(&f, from, to).await, a.id);
    let scope = SeriesScope::ThisAndFollowing {
        recurrence_id: at(2026, 11, 25, 10, 0),
    };
    f.delete(a.id, a.revision, scope).await.unwrap();
    assert!(count(&window(&f, from, to).await, a.id) < before);
    f.restore(a.id, scope).await.unwrap();
    let now = f.get(a.id).await.unwrap().unwrap();
    assert_eq!(now.recurrence, a.recurrence);
    assert_eq!(count(&window(&f, from, to).await, a.id), before);
}

#[tokio::test]
async fn restore_after_the_period_or_without_a_delete_is_refused() {
    let (s, f) = loaded();
    let a = s.get("weekly");
    let scope = SeriesScope::This {
        recurrence_id: at(2026, 11, 16, 10, 0),
    };
    assert!(matches!(
        f.restore(a.id, scope).await,
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        f.restore(a.id, SeriesScope::Whole).await,
        Err(Error::Invalid(_))
    ));
    f.delete(a.id, a.revision, scope).await.unwrap();
    f.set_now(utc(2027, 3, 1, 0, 0));
    assert!(matches!(
        f.restore(a.id, scope).await,
        Err(Error::Invalid(_))
    ));
}

#[tokio::test]
async fn respond_compares_against_expected() {
    let (s, f) = loaded();
    let a = s.get("invitation");
    let stale = fs_calendars_testdata::draft::Revision(a.revision.0 + 5);
    let r = f
        .respond(
            a.id,
            stale,
            &person(4),
            Answer::Accept,
            None,
            None,
            SeriesScope::Whole,
        )
        .await;
    assert!(matches!(r, Err(Error::Changed { current }) if current == a.revision));
    let ok = f
        .respond(
            a.id,
            a.revision,
            &person(4),
            Answer::Accept,
            None,
            None,
            SeriesScope::Whole,
        )
        .await;
    assert!(ok.is_ok());
}
