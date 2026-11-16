//! Writing an appointment: the row, the derived hull (R11) and the child rows, inside the caller's transaction (R3).
//!
//! Pattern: Transaction script. The row is written with an upsert, never `DELETE` + `INSERT` (draft R4); child
//! tables have no children of their own in this slice, so they are replaced.

use std::collections::{BTreeSet, HashMap};

use fs_calendar_testdata::check;
use fs_calendar_testdata::draft::{
    Appointment, Error, ObjectId, OverrideChange, PersonRef, Result, When, Zone, ZoneKind,
};

use crate::calendar_tables::{ALERT, APPOINTMENT, CATEGORY, COLLECTION, KEYWORD, PARTICIPANT};
use crate::codec::{id_value, opt_text, person_values, text};
use crate::fields::unsupported;
use crate::row_values::{Link, alert_values, appointment_values, participant_values};
use crate::store::Conn;

/// Whether a zone is a custom one (`/...`).
pub fn custom(z: &Zone) -> bool {
    z.kind() == ZoneKind::Custom
}

fn custom_when(w: &When) -> bool {
    matches!(w, When::Timed { zone, end_zone, .. } if custom(zone) || end_zone.as_ref().is_some_and(custom))
}

/// Check everything before anything is written: custom zones and parts not stored yet are `Unsupported`, then the
/// pilot's checks.
///
/// # Errors
/// `Unsupported` for a part that is not stored yet (never dropped quietly); `Invalid`/`UnknownZone` from the checks.
pub fn validate(a: &Appointment, capability: &str) -> Result<()> {
    let no = |reason: String| {
        Err(Error::Unsupported {
            capability: capability.to_owned(),
            reason,
        })
    };
    let moved = a.overrides.iter().any(|o| matches!(&o.change, OverrideChange::Changed(p) if p.when.as_ref().is_some_and(custom_when)));
    if custom_when(&a.when) || moved {
        return no(
            "custom zones are not stored yet (the calendar_custom_zone tables are missing)"
                .to_owned(),
        );
    }
    if let Some(part) = unsupported(a) {
        return no(format!("{part} is not stored yet"));
    }
    check::appointment(a, &HashMap::new())
}

/// Write a master: the row, the child rows, the rules and `related`. The envelope row must exist already.
///
/// # Errors
/// `Invalid` naming `table.column`, `UnknownZone`, or the engine's error.
pub async fn write_appointment(c: &Conn<'_>, a: &Appointment) -> Result<()> {
    write_row(c, a, None).await?;
    crate::rules::write_rules(c, a.id, false, &a.recurrence).await?;
    crate::rules::write_rules(c, a.id, true, &a.excluded_recurrence).await?;
    crate::rules::write_related(c, a.id, &a.related).await
}

/// Write the row and the child rows of an appointment or of an exception (`link`).
///
/// # Errors
/// `Invalid` naming `table.column`, `UnknownZone`, or the engine's error.
pub async fn write_row(c: &Conn<'_>, a: &Appointment, link: Option<Link>) -> Result<()> {
    c.upsert(&APPOINTMENT, &["id"], appointment_values(a, link)?)
        .await?;
    let id = id_value(a.id);
    for t in [
        "calendar_participant",
        "calendar_alert",
        "calendar_keyword",
        "calendar_category",
    ] {
        c.exec(
            &format!("DELETE FROM {t} WHERE appointment = ?"),
            vec![id.clone()],
        )
        .await?;
    }
    for p in &a.participants {
        c.insert(&PARTICIPANT, participant_values(a.id, p)).await?;
    }
    for x in &a.alerts {
        c.insert(&ALERT, alert_values(a.id, x)).await?;
    }
    for (t, list) in [(&KEYWORD, &a.keywords), (&CATEGORY, &a.categories)] {
        for v in list.iter().collect::<BTreeSet<_>>() {
            c.insert(t, vec![id.clone(), text(v)]).await?;
        }
    }
    Ok(())
}

/// Insert the row of a calendar (the envelope row must exist).
///
/// # Errors
/// `Invalid` naming `table.column`, or the engine's error.
pub async fn write_collection(
    c: &Conn<'_>,
    id: ObjectId,
    name: &str,
    color: Option<&str>,
    zone: Option<&Zone>,
    owner: &PersonRef,
) -> Result<()> {
    let [oid, oart, oabzug] = person_values(owner);
    c.insert(
        &COLLECTION,
        vec![
            id_value(id),
            text(name),
            opt_text(color),
            opt_text(zone.map(Zone::id)),
            oid,
            oart,
            oabzug,
        ],
    )
    .await
}
