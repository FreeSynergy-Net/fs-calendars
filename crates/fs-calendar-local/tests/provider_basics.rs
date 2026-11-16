//! The provider through the trait: calendars, create/get, scopes, capabilities, parts not stored yet.

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

mod common;

use common::{fx, ms, utc};
use fs_calendar_testdata::draft::{
    AppointmentPatch, AppointmentQuery, Appointments, BasicValue, Error, LocalDateTime, Location,
    SeriesScope,
};
use fs_calendar_testdata::sample::{all_day, at, timed};
use fs_calendar_testdata::zone::iana;

#[tokio::test]
async fn calendars_are_created_listed_and_a_bad_zone_is_refused() {
    let f = fx().await;
    let work =
        f.p.create_calendar("Arbeit", None, Some(iana("Europe/Berlin")))
            .await
            .unwrap();
    let names: Vec<String> =
        f.p.calendars()
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect();
    assert_eq!(names, ["Arbeit", "Persönlich"]);
    assert_eq!(work.default_zone, Some(iana("Europe/Berlin")));
    assert_eq!(work.owner, common::actor());
    assert!(matches!(
        f.p.create_calendar("X", None, Some(iana("Mars/Base")))
            .await,
        Err(Error::UnknownZone(_))
    ));
    assert!(matches!(
        f.p.create_calendar("  ", None, None).await,
        Err(Error::Invalid(_))
    ));
    assert_eq!(
        f.p.calendars().await.unwrap().len(),
        2,
        "nothing of the refused ones stays"
    );
}

#[tokio::test]
async fn create_get_roundtrip_with_every_when_kind() {
    let f = fx().await;
    let end_zone = fs_calendar_testdata::draft::When::Timed {
        start: at(2026, 11, 20, 9, 5),
        zone: iana("Europe/Berlin"),
        length: fs_calendar_testdata::draft::Span {
            days: 0,
            seconds: 9 * 3600,
        },
        end_zone: Some(iana("America/New_York")),
        zone_assumed: true,
    };
    let floating = fs_calendar_testdata::draft::When::Floating {
        start: at(2026, 11, 20, 8, 0),
        length: fs_calendar_testdata::draft::Span {
            days: 0,
            seconds: 1800,
        },
    };
    for (t, w) in [
        ("timed", timed(at(2026, 11, 20, 9, 0), "Europe/Berlin", 45)),
        ("flug", end_zone),
        ("schwebend", floating),
        ("ganztägig", all_day(2026, 12, 24, 3)),
    ] {
        let a = f.make(t, w.clone()).await;
        assert_eq!(a.when, w);
        let got = f.p.get(a.id).await.unwrap().unwrap();
        assert_eq!(got, a, "{t}");
        assert_eq!(got.revision.0, 1);
        assert_eq!(got.created, utc(ms(2026, 11, 16, 10, 0)));
    }
}

#[tokio::test]
async fn create_checks_before_it_writes() {
    let f = fx().await;
    let rows = f.count("objects").await;
    let bad_zone =
        f.p.create(f.draft("x", timed(at(2026, 11, 20, 9, 0), "Mars/Base", 30)))
            .await;
    assert!(matches!(bad_zone, Err(Error::UnknownZone(_))));
    assert!(matches!(
        f.p.create(f.draft("  ", timed(at(2026, 11, 20, 9, 0), "Europe/Berlin", 30)))
            .await,
        Err(Error::Invalid(_))
    ));
    let mut d = f.draft("x", timed(at(2026, 11, 20, 9, 0), "Europe/Berlin", 30));
    d.calendar = fs_calendar_testdata::ids::object_id(99);
    assert!(matches!(f.p.create(d).await, Err(Error::NotFound(_))));
    assert_eq!(f.count("objects").await, rows, "nothing written");
}

#[tokio::test]
async fn parts_that_are_not_stored_yet_are_refused_never_dropped() {
    let f = fx().await;
    let rows = f.count("objects").await;
    let mut series = f.draft("Serie", timed(at(2026, 11, 20, 9, 0), "Europe/Berlin", 30));
    series.rest.links = Some(vec![fs_calendar_testdata::draft::Link {
        key: "l".into(),
        href: Some("https://example.org".into()),
        file: None,
        content_type: None,
        size: None,
        rel: None,
        title: None,
    }]);
    let mut place = f.draft("Ort", timed(at(2026, 11, 20, 9, 0), "Europe/Berlin", 30));
    place.rest.locations = Some(vec![Location {
        key: "l".into(),
        name: "Büro".into(),
        description: None,
        types: vec![],
        coordinates: None,
        zone: None,
        relative_to: None,
        place: None,
    }]);
    for d in [series, place] {
        let r = f.p.create(d).await;
        assert!(
            matches!(r, Err(Error::Unsupported { ref capability, .. }) if capability == "event.create"),
            "{r:?}"
        );
    }
    assert_eq!(f.count("objects").await, rows);
    let a = f.make_berlin("ok", 2, 9).await;
    let this = SeriesScope::This {
        recurrence_id: LocalDateTime::parse("2026-11-02T09:00:00").unwrap(),
    };
    assert!(
        matches!(
            f.p.update(a.id, a.revision, AppointmentPatch::default(), this)
                .await,
            Err(Error::Invalid(_))
        ),
        "not a series"
    );
    assert!(matches!(
        f.p.delete(a.id, a.revision, this).await,
        Err(Error::Invalid(_))
    ));
    assert!(
        matches!(
            f.p.search("x", None, 5).await,
            Err(Error::Unsupported { .. })
        ),
        "not offered in this slice"
    );
}

#[tokio::test]
async fn capabilities_say_what_this_slice_offers() {
    let f = fx().await;
    let caps = f.p.capabilities().0;
    let on: Vec<&str> = caps
        .iter()
        .filter(|(_, v)| *v)
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(
        on,
        [
            "calendar.read",
            "calendar.create",
            "event.create",
            "event.update",
            "event.delete",
            "event.restore",
            "event.history"
        ]
    );
    let _ = AppointmentQuery::default();
}

#[tokio::test]
async fn keywords_and_categories_are_sets_here_too() {
    let f = fx().await;
    let mut d = f.draft("Menge", timed(at(2026, 12, 1, 9, 0), "Europe/Berlin", 30));
    d.rest.keywords = Some(vec!["b".into(), "a".into(), "b".into()]);
    let a = f.p.create(d).await.unwrap();
    let mut k = a.keywords;
    k.sort();
    assert_eq!(
        k,
        ["a", "b"],
        "duplicates fall together (as a set; the order is not a contract)"
    );
}
