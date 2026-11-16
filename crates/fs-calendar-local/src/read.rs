//! Reading appointments back from the typed rows.
//!
//! Pattern: Adapter (rows to the trait's types). A row that does not read back is `Unavailable("damaged row")`,
//! so a bad row costs only the call that touches it.
use fs_calendar_testdata::draft::{Appointment, Result, Revision, Span, Utc, When};
use turso::Value;

use crate::calendar_tables::APPOINTMENT;
use crate::codec::{
    Row, damaged, get_bool, get_id, get_int, get_opt_int, get_opt_text, get_text, int_to_date,
    int_to_local, zone_of,
};
pub use crate::read_children::{fill, fill_series};
use crate::spec::Table;
use crate::store::Conn;
use crate::words::{FREE_BUSY, PRIVACY, STATUS, TEXT_TYPE, from_word};

/// A row with column access by name.
pub struct Rd<'a> {
    pub t: &'a Table,
    pub row: &'a Row,
}

impl Rd<'_> {
    fn ix(&self, name: &str) -> Result<usize> {
        self.t
            .cols
            .iter()
            .position(|c| c.name == name)
            .ok_or_else(|| damaged(name))
    }
    pub fn int(&self, n: &str) -> Result<i64> {
        get_int(self.row, self.ix(n)?)
    }
    pub fn opt_int(&self, n: &str) -> Result<Option<i64>> {
        get_opt_int(self.row, self.ix(n)?)
    }
    pub fn text(&self, n: &str) -> Result<String> {
        get_text(self.row, self.ix(n)?)
    }
    pub fn opt_text(&self, n: &str) -> Result<Option<String>> {
        get_opt_text(self.row, self.ix(n)?)
    }
    pub fn flag(&self, n: &str) -> Result<bool> {
        get_bool(self.row, self.ix(n)?)
    }
    pub fn id(&self, n: &str) -> Result<fs_calendar_testdata::draft::ObjectId> {
        get_id(self.row, self.ix(n)?)
    }
}

/// An appointment with the trash state of its envelope.
pub struct Loaded {
    pub appt: Appointment,
    pub deleted: Option<i64>,
    pub purge_after: Option<i64>,
}

/// `SELECT` of the appointment columns plus `rev`, `deleted`, `purge_after` of the envelope; append `WHERE ...`.
#[must_use]
pub fn select() -> String {
    select_with("")
}

/// Like [`select`] with more columns after `purge_after` (`extra` starts with a comma).
#[must_use]
pub fn select_with(extra: &str) -> String {
    let cols: Vec<String> = APPOINTMENT
        .cols
        .iter()
        .map(|c| format!("t.{}", c.name))
        .collect();
    format!(
        "SELECT {}, o.rev, o.deleted, o.purge_after{extra} FROM calendar_appointment t JOIN objects o ON o.id = t.id",
        cols.join(", ")
    )
}

/// The `When` of a row.
///
/// # Errors
/// `Unavailable` when the row does not read back.
pub fn when_of(r: &Rd<'_>) -> Result<When> {
    let length = Span {
        days: u32::try_from(r.int("when_len_days")?).map_err(|_| damaged("length"))?,
        seconds: u32::try_from(r.int("when_len_seconds")?).map_err(|_| damaged("length"))?,
    };
    Ok(match r.text("when_kind")?.as_str() {
        "timed" => When::Timed {
            start: int_to_local(r.opt_int("when_local")?.ok_or_else(|| damaged("start"))?)?,
            zone: zone_of(&r.text("when_zone")?)?,
            length,
            end_zone: r
                .opt_text("when_end_zone")?
                .map(|z| zone_of(&z))
                .transpose()?,
            zone_assumed: r.flag("when_zone_assumed")?,
        },
        "floating" => When::Floating {
            start: int_to_local(r.opt_int("when_local")?.ok_or_else(|| damaged("start"))?)?,
            length,
        },
        "all_day" => When::AllDay {
            first: int_to_date(r.opt_int("when_date")?.ok_or_else(|| damaged("date"))?)?,
            days: length.days,
        },
        _ => return Err(damaged("when_kind")),
    })
}

/// An appointment row without child rows, plus the envelope columns that follow it.
///
/// # Errors
/// `Unavailable` when the row does not read back.
pub fn head(row: &Row) -> Result<Loaded> {
    let r = Rd {
        t: &APPOINTMENT,
        row,
    };
    let n = APPOINTMENT.cols.len();
    let appt = Appointment {
        id: r.id("id")?,
        calendar: r.id("collection")?,
        uid: r.text("uid")?,
        revision: Revision(u64::try_from(get_int(row, n)?).map_err(|_| damaged("rev"))?),
        sequence: u32::try_from(r.int("sequence")?).map_err(|_| damaged("sequence"))?,
        created: Utc::from_millis(r.int("created")?),
        updated: Utc::from_millis(r.int("updated")?),
        title: r.text("title")?,
        description: r.text("description")?,
        description_type: from_word(&TEXT_TYPE, &r.text("description_type")?)?,
        locale: r.opt_text("locale")?,
        when: when_of(&r)?,
        status: from_word(&STATUS, &r.text("status")?)?,
        free_busy: from_word(&FREE_BUSY, &r.text("free_busy")?)?,
        privacy: from_word(&PRIVACY, &r.text("privacy")?)?,
        priority: u8::try_from(r.int("priority")?).map_err(|_| damaged("priority"))?,
        color: r.opt_text("color")?,
        keywords: Vec::new(),
        categories: Vec::new(),
        locations: Vec::new(),
        virtual_locations: Vec::new(),
        links: Vec::new(),
        participants: Vec::new(),
        reply_to: Vec::new(),
        alerts: Vec::new(),
        use_default_alerts: r.flag("use_default_alerts")?,
        recurrence: Vec::new(),
        excluded_recurrence: Vec::new(),
        overrides: Vec::new(),
        related: Vec::new(),
        localizations: Vec::new(),
        hide_attendees: r.flag("hide_attendees")?,
        may_invite_self: r.flag("may_invite_self")?,
        may_invite_others: r.flag("may_invite_others")?,
        origin: None,
    };
    Ok(Loaded {
        appt,
        deleted: get_opt_int(row, n + 1)?,
        purge_after: get_opt_int(row, n + 2)?,
    })
}

/// Load one appointment (any trash state) by a `WHERE` condition on `t`/`o`.
///
/// # Errors
/// `Unavailable` when a row does not read back.
pub async fn load(c: &Conn<'_>, condition: &str, params: Vec<Value>) -> Result<Option<Loaded>> {
    let rows = c
        .query(&format!("{} WHERE {condition}", select()), params)
        .await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    let mut l = head(row)?;
    fill(c, &mut l.appt).await?;
    fill_series(c, &mut l.appt).await?;
    Ok(Some(l))
}
