//! The fixed sample of the pilot through `LocalAppointments`: what the slice stores comes back equal; what it does not
//! store yet is refused with `Unsupported` (the test names which, so the gap stays visible).

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

mod common;

use common::fx;
use fs_calendars_testdata::draft::{
    Appointment, AppointmentDraft, AppointmentPatch, Appointments, Error, ObjectId, OverrideChange,
    SeriesScope,
};
use fs_calendars_testdata::sample::sample;

fn draft_of(a: &Appointment, calendar: ObjectId) -> AppointmentDraft {
    AppointmentDraft {
        calendar,
        uid: Some(a.uid.clone()),
        title: a.title.clone(),
        when: a.when.clone(),
        rest: AppointmentPatch {
            description: Some((a.description.clone(), a.description_type)),
            locale: Some(a.locale.clone()),
            status: Some(a.status),
            free_busy: Some(a.free_busy),
            privacy: Some(a.privacy),
            priority: Some(a.priority),
            color: Some(a.color.clone()),
            keywords: Some(a.keywords.clone()),
            categories: Some(a.categories.clone()),
            participants: Some(a.participants.clone()),
            alerts: Some(a.alerts.clone()),
            use_default_alerts: Some(a.use_default_alerts),
            recurrence: Some(a.recurrence.clone()),
            excluded_recurrence: Some(a.excluded_recurrence.clone()),
            locations: Some(a.locations.clone()),
            virtual_locations: Some(a.virtual_locations.clone()),
            links: Some(a.links.clone()),
            localizations: Some(a.localizations.clone()),
            ..AppointmentPatch::default()
        },
    }
}

fn sorted(mut a: Appointment) -> Appointment {
    a.participants.sort_by(|x, y| x.key.cmp(&y.key));
    a.alerts.sort_by(|x, y| x.key.cmp(&y.key));
    a.keywords.sort();
    a.categories.sort();
    a
}

#[tokio::test]
async fn every_sample_appointment_is_stored_equal_or_refused_by_name() {
    let f = fx().await;
    let (mut stored, mut refused) = (Vec::new(), Vec::new());
    for (slug, a) in sample().appointments {
        let mut want = a.clone();
        want.reply_to.clear();
        want.origin = None;
        let created = match f.p.create(draft_of(&want, f.cal)).await {
            Ok(g) => g,
            Err(e) => {
                refused.push((slug, e));
                continue;
            }
        };
        // Exceptions come in through the scopes, as a program would write them.
        let mut got = created;
        for o in &a.overrides {
            let rid = o.recurrence_id;
            let scope = SeriesScope::This { recurrence_id: rid };
            match &o.change {
                OverrideChange::Excluded => {
                    f.p.delete(got.id, got.revision, scope).await.unwrap();
                }
                OverrideChange::Changed(p) => {
                    f.p.update(got.id, got.revision, p.clone(), scope)
                        .await
                        .unwrap();
                }
            }
            got = f.p.get(got.id).await.unwrap().unwrap();
        }
        let mut e = sorted(want);
        (
            e.id, e.calendar, e.revision, e.created, e.updated, e.sequence,
        ) = (
            got.id,
            got.calendar,
            got.revision,
            got.created,
            got.updated,
            got.sequence,
        );
        assert_eq!(sorted(got.clone()), e, "{slug}");
        assert_eq!(
            sorted(f.p.get_by_uid(&a.uid).await.unwrap().unwrap()),
            e,
            "{slug} read back"
        );
        stored.push(slug);
    }
    let names: Vec<&str> = refused.iter().map(|(s, _)| *s).collect();
    assert_eq!(
        names,
        ["custom-zone", "invitation"],
        "exactly these are refused"
    );
    for (slug, e) in &refused {
        match e {
            Error::Unsupported { capability, reason } => {
                assert_eq!(capability, "event.create", "{slug}");
                println!("{slug}: {reason}");
            }
            other => panic!("{slug}: expected Unsupported, got {other:?}"),
        }
    }
    assert_eq!(stored.len(), 48);
    assert!(f.count("calendar_appointment").await >= 48);
}
