//! Round trips through the border: ICS to store to ICS (original and edited), JSCalendar and ICS into each other,
//! and the export rules (exception as `EXDATE`, floating mark, a rule kept as written).

#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]

mod common;

use common::exchange::{Ex, ICS_TYPE, JSCAL_TYPE, ex, export_text, fixture, import_text};
use fs_calendar_testdata::draft::{
    Appointment, AppointmentPatch, Appointments, Frequency, RuleEnd, SeriesScope, When,
};
use fs_calendar_testdata::sample::{at, rule, timed};

/// An appointment without the parts a new store makes itself.
fn same(mut a: Appointment) -> Appointment {
    (
        a.id, a.calendar, a.revision, a.created, a.updated, a.sequence, a.origin,
    ) = (
        fs_calendar_testdata::ids::object_id(0),
        fs_calendar_testdata::ids::object_id(0),
        fs_calendar_testdata::draft::Revision(0),
        fs_calendar_testdata::draft::Utc::from_millis(0),
        fs_calendar_testdata::draft::Utc::from_millis(0),
        0,
        None,
    );
    a.participants.sort_by(|x, y| x.key.cmp(&y.key));
    a.alerts.sort_by(|x, y| x.key.cmp(&y.key));
    a.keywords.sort();
    a.categories.sort();
    a
}

async fn only(e: &Ex, uid: &str) -> Appointment {
    e.local
        .get_by_uid(uid)
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("no {uid}"))
}

#[tokio::test]
async fn an_unchanged_import_exports_the_original_byte_for_byte() {
    let e = ex().await;
    let text = fixture("google.ics");
    let r = import_text(&e, &text, ICS_TYPE).await;
    let (out, lost) = export_text(&e, &r.created, ICS_TYPE).await;
    assert_eq!(out, text, "the received file, unchanged");
    assert!(lost.is_empty());
}

#[tokio::test]
async fn an_edited_import_exports_new_ics_that_reads_back_equal() {
    let e = ex().await;
    let r = import_text(&e, &fixture("google.ics"), ICS_TYPE).await;
    let a = e.local.get(r.created[0]).await.unwrap().unwrap();
    let edited = e
        .local
        .update(
            a.id,
            a.revision,
            AppointmentPatch {
                title: Some("Chorprobe (neu)".into()),
                ..Default::default()
            },
            SeriesScope::Whole,
        )
        .await
        .unwrap();
    let (out, lost) = export_text(&e, &[a.id], ICS_TYPE).await;
    assert!(
        out.contains("Chorprobe (neu)")
            && out.contains("RRULE:FREQ=WEEKLY")
            && out.contains("EXDATE"),
        "{out}"
    );
    assert!(lost.iter().all(|l| l.field != "recurrence"), "{lost:?}");
    let other = ex().await;
    let back = import_text(&other, &out, ICS_TYPE).await;
    assert_eq!(back.created.len(), 1, "{back:?}");
    let b = only(&other, &edited.uid).await;
    let (b, edited) = (same(b), same(edited));
    assert_eq!(b.participants, edited.participants, "participants");
    assert_eq!(b.alerts, edited.alerts, "alerts");
    assert_eq!(b.overrides, edited.overrides, "overrides");
    assert_eq!(b.recurrence, edited.recurrence, "recurrence");
    assert_eq!(b, edited);
}

#[tokio::test]
async fn jscalendar_and_ics_carry_the_same_event_both_ways() {
    let e = ex().await;
    let mut d =
        e.fx.draft("Runde", timed(at(2026, 11, 2, 9, 0), "Europe/Berlin", 90));
    let mut r = rule(Frequency::Weekly);
    r.end = RuleEnd::Count(5);
    d.rest.recurrence = Some(vec![r]);
    d.rest.keywords = Some(vec!["a".into(), "b".into()]);
    d.rest.description = Some(("Text".into(), fs_calendar_testdata::draft::TextType::Plain));
    let a = e.local.create(d).await.unwrap();
    let this = SeriesScope::This {
        recurrence_id: at(2026, 11, 9, 9, 0),
    };
    e.local
        .update(
            a.id,
            a.revision,
            AppointmentPatch {
                title: Some("verlegt".into()),
                ..Default::default()
            },
            this,
        )
        .await
        .unwrap();
    let a = e.local.get(a.id).await.unwrap().unwrap();
    for ty in [JSCAL_TYPE, ICS_TYPE] {
        let (text, lost) = export_text(&e, &[a.id], ty).await;
        assert!(lost.is_empty(), "{ty}: {lost:?}");
        let other = ex().await;
        let back = import_text(&other, &text, ty).await;
        assert_eq!(
            (back.created.len(), back.failed.len()),
            (1, 0),
            "{ty}: {back:?} {text}"
        );
        assert_eq!(same(only(&other, &a.uid).await), same(a.clone()), "{ty}");
    }
    // JSCalendar to ICS and back: through a second store.
    let (js, _) = export_text(&e, &[a.id], JSCAL_TYPE).await;
    let mid = ex().await;
    import_text(&mid, &js, JSCAL_TYPE).await;
    let m = only(&mid, &a.uid).await;
    let (ics, _) = export_text(&mid, &[m.id], ICS_TYPE).await;
    let end = ex().await;
    import_text(&end, &ics, ICS_TYPE).await;
    assert_eq!(same(only(&end, &a.uid).await), same(a));
}

#[tokio::test]
async fn a_deleted_changed_occurrence_is_only_an_exdate_in_the_export() {
    let e = ex().await;
    let mut d =
        e.fx.draft("Serie", timed(at(2026, 11, 2, 9, 0), "Europe/Berlin", 60));
    let mut r = rule(Frequency::Weekly);
    r.end = RuleEnd::Count(4);
    d.rest.recurrence = Some(vec![r]);
    let a = e.local.create(d).await.unwrap();
    let this = SeriesScope::This {
        recurrence_id: at(2026, 11, 9, 9, 0),
    };
    let u = e
        .local
        .update(
            a.id,
            a.revision,
            AppointmentPatch {
                title: Some("Geheim".into()),
                ..Default::default()
            },
            this,
        )
        .await
        .unwrap();
    e.local.delete(a.id, u.revision, this).await.unwrap();
    let (ics, _) = export_text(&e, &[a.id], ICS_TYPE).await;
    assert_eq!(
        ics.matches("BEGIN:VEVENT").count(),
        1,
        "no second VEVENT for the deleted occurrence: {ics}"
    );
    assert!(
        ics.contains("EXDATE") && ics.contains("20261109T090000"),
        "{ics}"
    );
    assert!(
        !ics.contains("Geheim") && !ics.contains("RECURRENCE-ID"),
        "the change of a deleted occurrence is not exported: {ics}"
    );
}

#[tokio::test]
async fn floating_survives_the_export_and_a_rule_kept_as_written_comes_back() {
    let e = ex().await;
    let f = e
        .local
        .create(e.fx.draft(
            "Ueberall",
            When::Floating {
                start: at(2026, 11, 20, 8, 0),
                length: fs_calendar_testdata::draft::Span {
                    days: 0,
                    seconds: 3600,
                },
            },
        ))
        .await
        .unwrap();
    let (ics, _) = export_text(&e, &[f.id], ICS_TYPE).await;
    assert!(
        ics.contains("X-FS-FLOATING:TRUE") && !ics.contains("TZID"),
        "{ics}"
    );
    let other = ex().await;
    import_text(&other, &ics, ICS_TYPE).await;
    assert!(matches!(
        only(&other, &f.uid).await.when,
        When::Floating { .. }
    ));

    let r = import_text(&e, &fixture("dialect.ics"), ICS_TYPE).await;
    let id = e
        .local
        .get_by_uid("yearday-1@example.org")
        .await
        .unwrap()
        .unwrap();
    assert!(r.created.contains(&id.id));
    e.local
        .update(
            id.id,
            id.revision,
            AppointmentPatch {
                title: Some("Jahrestage 2".into()),
                ..Default::default()
            },
            SeriesScope::Whole,
        )
        .await
        .unwrap();
    let (out, _) = export_text(&e, &[id.id], ICS_TYPE).await;
    assert!(
        out.contains("BYYEARDAY=1,100,200"),
        "the rule as it was written: {out}"
    );
    assert!(out.contains("Jahrestage 2"));
}

#[tokio::test]
async fn a_series_outside_the_subset_keeps_exdate_and_exceptions_through_an_edit() {
    let e = ex().await;
    let r = import_text(&e, &fixture("yearday_series.ics"), ICS_TYPE).await;
    assert_eq!((r.created.len(), r.failed.len()), (1, 0), "{r:?}");
    let fields: Vec<&str> = r.lost.iter().map(|l| l.field.as_str()).collect();
    assert!(
        fields
            .iter()
            .filter(|f| f.starts_with("recurrenceOverrides/"))
            .count()
            >= 2,
        "the EXDATE and the exception are reported, not dropped: {fields:?}"
    );
    let a = e.local.get(r.created[0]).await.unwrap().unwrap();
    assert!(
        a.overrides.is_empty() && a.recurrence.is_empty(),
        "shown as its first occurrence"
    );
    e.local
        .update(
            a.id,
            a.revision,
            AppointmentPatch {
                title: Some("Neu".into()),
                ..Default::default()
            },
            SeriesScope::Whole,
        )
        .await
        .unwrap();
    let (out, _) = export_text(&e, &[a.id], ICS_TYPE).await;
    assert!(
        out.contains("Neu") && out.contains("BYYEARDAY=1,100,200"),
        "{out}"
    );
    assert!(
        out.contains("EXDATE") && out.contains("20270410T090000"),
        "the deleted occurrence stays deleted: {out}"
    );
    assert!(
        out.contains("RECURRENCE-ID") && out.contains("Verschoben"),
        "the moved occurrence stays: {out}"
    );
}
