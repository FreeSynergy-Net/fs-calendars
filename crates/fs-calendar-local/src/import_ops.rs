//! Importing one object (one `uid`) in one transaction: the appointment, its exceptions, the origin, the original.
//!
//! Pattern: Transaction script. A `uid` that exists already is refused (`Invalid`), so a second import of the same
//! file adds nothing; an exception that is not an occurrence of the series is a loss, not an error.

use fs_calendar_testdata::draft::{
    Appointment, AppointmentDraft, Loss, ObjectId, Participant, PersonRef, Result, Revision,
    SeriesScope,
};
use fs_calendar_testdata::scope::occurrence;

use crate::codec::id_value;
use crate::jscal_in::{RawChange, Read};
use crate::ops::create_in;
use crate::origin::{NewOrigin, record, set_unread};
use crate::read::load;
use crate::scope_delete::delete_partial;
use crate::scope_ops::update_this;
use crate::store::Conn;

/// What one imported object left behind.
pub struct Imported {
    pub appointment: Appointment,
    pub losses: Vec<Loss>,
}

async fn current(c: &Conn<'_>, id: ObjectId) -> Result<Appointment> {
    load(c, "t.id = ?", vec![id_value(id)])
        .await?
        .map(|l| l.appt)
        .ok_or(fs_calendar_testdata::draft::Error::NotFound(id))
}

/// Write the object of `read` (people already resolved to `participants`) and record where it came from.
///
/// # Errors
/// `Invalid` (the `uid` exists, or a value does not fit), `NotFound` (calendar), or the engine's error; nothing is
/// written then.
pub async fn import_one(
    c: &Conn<'_>,
    actor: &PersonRef,
    now: i64,
    calendar: ObjectId,
    read: &Read,
    participants: Vec<Participant>,
    origin: &NewOrigin,
) -> Result<Imported> {
    let mut rest = read.rest.clone();
    rest.participants = Some(participants);
    let draft = AppointmentDraft {
        calendar,
        uid: Some(read.uid.clone()),
        title: read.title.clone(),
        when: read.when.clone(),
        rest,
    };
    let mut a = create_in(c, actor, now, draft).await?;
    let mut losses = read.losses.clone();
    for (rid, change) in &read.overrides {
        if occurrence(&a, *rid).is_err() {
            losses.push(Loss {
                uid: Some(read.uid.clone()),
                field: format!(
                    "recurrenceOverrides/{}",
                    fs_calendar_testdata::draft::BasicValue::canonical(rid)
                ),
                reason: "not an occurrence of the series: left out".into(),
            });
            continue;
        }
        let scope = SeriesScope::This {
            recurrence_id: *rid,
        };
        match change {
            RawChange::Excluded => {
                delete_partial(c, actor, now, a.id, a.revision, scope).await?;
            }
            RawChange::Changed(p) => {
                update_this(c, actor, now, a.id, a.revision, p, *rid).await?;
            }
        }
        a = current(c, a.id).await?;
    }
    if read.unread_rule {
        set_unread(c, a.id, "recurrence").await?;
    }
    record(c, a.id, origin, Revision(a.revision.0), now).await?;
    let appointment = current(c, a.id).await?;
    Ok(Imported {
        appointment,
        losses,
    })
}
