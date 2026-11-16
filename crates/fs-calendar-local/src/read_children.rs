//! Reading the child rows of an appointment (participants, alerts, keywords, categories) and the series parts.
//!
//! Pattern: Adapter (rows to the trait's types); a row that does not read back is `Unavailable("damaged row")`.

use fs_calendar_testdata::draft::{
    Alert, Appointment, Attendee, Participant, Result, Roles, Trigger, Utc,
};
use turso::Value;

use crate::calendar_tables::{ALERT, PARTICIPANT};
use crate::codec::{Row, damaged, get_text, id_value, person_of, reference_of};
use crate::read::Rd;
use crate::store::Conn;
use crate::words::{ALERT_ACTION, PARTICIPATION, RELATIVE_TO, from_word};
use fs_objects::v5::spec::Table;

async fn child_rows(c: &Conn<'_>, t: &Table, id: &Value) -> Result<Vec<Row>> {
    let cols: Vec<&str> = t.cols.iter().map(|c| c.name).collect();
    c.query(
        &format!(
            "SELECT {} FROM {} WHERE appointment = ? ORDER BY {}",
            cols.join(", "),
            t.name,
            if t.cols.iter().any(|x| x.name == "key") {
                "key"
            } else {
                "value"
            }
        ),
        vec![id.clone()],
    )
    .await
}

fn participant(row: &Row) -> Result<Participant> {
    let r = Rd {
        t: &PARTICIPANT,
        row,
    };
    let who_id = r.text("who_id")?;
    let who = match r.text("who_kind")?.as_str() {
        "actor" => {
            let at = PARTICIPANT
                .cols
                .iter()
                .position(|c| c.name == "who_id")
                .ok_or_else(|| damaged("who_id"))?;
            Attendee::Actor(person_of(row, at)?)
        }
        "resource" => Attendee::Resource(reference_of(&who_id)?),
        "location" => Attendee::Location(reference_of(&who_id)?),
        _ => return Err(damaged("who_kind")),
    };
    Ok(Participant {
        key: r.text("key")?,
        who,
        roles: Roles {
            owner: r.flag("role_owner")?,
            attendee: r.flag("role_attendee")?,
            optional: r.flag("role_optional")?,
            informational: r.flag("role_informational")?,
            chair: r.flag("role_chair")?,
            contact: r.flag("role_contact")?,
        },
        status: from_word(&PARTICIPATION, &r.text("status")?)?,
        expect_reply: r.flag("expect_reply")?,
        comment: r.opt_text("comment")?,
        send_to: Vec::new(),
        invited_by: r.opt_text("invited_by")?,
        delegated_to: Vec::new(),
        delegated_from: Vec::new(),
        schedule_sequence: u32::try_from(r.int("schedule_sequence")?)
            .map_err(|_| damaged("schedule_sequence"))?,
        schedule_updated: r.opt_int("schedule_updated")?.map(Utc::from_millis),
    })
}

fn alert(row: &Row) -> Result<Alert> {
    let r = Rd { t: &ALERT, row };
    let trigger = match r.text("trigger_kind")?.as_str() {
        "offset" => Trigger::Offset {
            seconds: r.int("offset_seconds")?,
            relative_to: from_word(&RELATIVE_TO, &r.text("relative_to")?)?,
        },
        "absolute" => Trigger::Absolute {
            at: Utc::from_millis(r.int("at")?),
        },
        _ => return Err(damaged("trigger_kind")),
    };
    Ok(Alert {
        key: r.text("key")?,
        trigger,
        action: from_word(&ALERT_ACTION, &r.text("action")?)?,
        acknowledged: r.opt_int("acknowledged")?.map(Utc::from_millis),
    })
}

/// Fill the child lists of a head.
///
/// # Errors
/// `Unavailable` when a child row does not read back.
pub async fn fill(c: &Conn<'_>, a: &mut Appointment) -> Result<()> {
    let id = id_value(a.id);
    a.participants = child_rows(c, &PARTICIPANT, &id)
        .await?
        .iter()
        .map(participant)
        .collect::<Result<_>>()?;
    a.alerts = child_rows(c, &ALERT, &id)
        .await?
        .iter()
        .map(alert)
        .collect::<Result<_>>()?;
    for (table, out) in [
        ("calendar_keyword", &mut a.keywords),
        ("calendar_category", &mut a.categories),
    ] {
        let rows = c
            .query(
                &format!("SELECT value FROM {table} WHERE appointment = ? ORDER BY value"),
                vec![id.clone()],
            )
            .await?;
        *out = rows.iter().map(|r| get_text(r, 0)).collect::<Result<_>>()?;
    }
    Ok(())
}

/// Fill the rules, `related` and the overrides of a master.
///
/// # Errors
/// `Unavailable` when a row does not read back.
pub async fn fill_series(c: &Conn<'_>, a: &mut Appointment) -> Result<()> {
    (a.recurrence, a.excluded_recurrence) = crate::rules::read_rules(c, a.id).await?;
    a.related = crate::rules::read_related(c, a.id).await?;
    a.overrides = crate::series::read_overrides(c, a.id).await?;
    a.origin = crate::origin::read_origin(c, a.id).await?;
    Ok(())
}
