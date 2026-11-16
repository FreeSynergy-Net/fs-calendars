//! JSCalendar (RFC 8984) at the border: the loss report has one form, nothing is dropped quietly.

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

mod common;

use common::exchange::{ICS_TYPE, JSCAL_TYPE, ex, import_text, media};
use fs_calendar_testdata::draft::{Appointments, Error, Exchange, ExportTarget, Level, When};

const EVENT: &str = r#"{
  "@type": "Event", "version": "2.0", "uid": "js-1@example.org", "title": "Gespraech",
  "start": "2026-12-01T10:00:00", "timeZone": "Mars/Base", "duration": "PT45M",
  "participants": {
    "p1": {"@type": "Participant", "email": "ann@example.org", "name": "Ann", "roles": {"attendee": true}, "participationStatus": "accepted"},
    "p2": {"@type": "Participant", "name": "Ohne Adresse", "roles": {"attendee": true}}
  },
  "locations": {"l1": {"@type": "Location", "name": "Buero"}},
  "links": {"k1": {"@type": "Link", "href": "https://example.org"}},
  "vendor:extra": 1
}"#;

#[tokio::test]
async fn every_loss_names_the_object_the_field_and_the_reason() {
    let e = ex().await;
    let r = import_text(&e, EVENT, JSCAL_TYPE).await;
    assert_eq!((r.created.len(), r.failed.len()), (1, 0), "{r:?}");
    let a = e.local.get(r.created[0]).await.unwrap().unwrap();
    assert!(
        matches!(&a.when, When::Timed { zone, zone_assumed: true, .. } if zone.id() == "Europe/Berlin"),
        "an unknown zone is never guessed as such: {:?}",
        a.when
    );
    assert_eq!(
        a.participants.len(),
        1,
        "a person without an address is not a free name"
    );
    let fields: Vec<&str> = r.lost.iter().map(|l| l.field.as_str()).collect();
    for want in [
        "timeZone",
        "participants/p2",
        "locations",
        "links",
        "vendor:extra",
    ] {
        assert!(fields.contains(&want), "{want} in {fields:?}");
    }
    assert!(
        r.lost
            .iter()
            .all(|l| l.uid.as_deref() == Some("js-1@example.org") && !l.reason.is_empty())
    );
    assert_eq!(r.zone_assumed, ["js-1@example.org"]);
}

#[tokio::test]
async fn a_bad_event_fails_alone_and_a_foreign_format_is_unsupported() {
    let e = ex().await;
    let r = import_text(&e, r#"{"@type":"Group","entries":[{"@type":"Event","uid":"bad","title":"x"},{"@type":"Event","uid":"ok","title":"y","start":"2026-12-01T10:00:00","timeZone":"UTC"}]}"#, JSCAL_TYPE).await;
    assert_eq!((r.created.len(), r.failed.len()), (1, 1), "{r:?}");
    let t =
        e.ex.export(
            &r.created,
            &ExportTarget {
                media_type: media("application/pdf"),
                destination: None,
            },
        )
        .await;
    assert!(
        matches!(t, Err(Error::Unsupported { ref capability, .. }) if capability == "exchange.export"),
        "{t:?}"
    );
    let f = e.files.put(b"x", &media("text/plain"));
    let i =
        e.ex.import(f, &media("text/plain"), &common::exchange::ctx(&e))
            .await;
    assert!(matches!(i, Err(Error::Unsupported { .. })));
}

#[tokio::test]
async fn the_formats_say_what_is_missing() {
    let e = ex().await;
    let f = e.ex.formats();
    assert_eq!(
        f.iter()
            .map(|x| x.media_type.0.as_str())
            .collect::<Vec<_>>(),
        [ICS_TYPE, JSCAL_TYPE]
    );
    assert!(f.iter().all(|x| matches!(&x.import, Level::Partial { missing } if missing.contains(&"locations".to_owned()))));
}
