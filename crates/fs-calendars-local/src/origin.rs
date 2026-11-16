//! Where an imported object came from, and the file as it was received (`object_origin`, `object_original`), plus
//! the parts of it this program cannot interpret (`calendar_unread`).
//!
//! Pattern: Transaction script. The original is immutable after insert (ADR 0030); "unchanged since import" is
//! computed (`rev = rev_at_import`), never stored (ADR 0085).

use fs_calendars_testdata::draft::{
    FileRef, LocalDateTime, MediaType, ObjectId, Origin, OriginChannel, Reference, Result,
    Revision, Utc,
};
use fs_objects::v5::original as layer;

use crate::codec::{
    get_int, get_opt_int, get_opt_text, get_text, id_value, int_to_local, reference_of, text,
};
use crate::convert::adapt;
use crate::registry::DOMAIN;
use crate::series_tables::UNREAD;
use crate::store::Conn;
use crate::words::{from_word, word};
use fs_objects::v5::tables_more::OBJECT_ORIGIN;

const CHANNELS: [(OriginChannel, &str); 7] = [
    (OriginChannel::File, "file"),
    (OriginChannel::CalDav, "caldav"),
    (OriginChannel::Google, "google"),
    (OriginChannel::Outlook, "outlook"),
    (OriginChannel::Itip, "itip"),
    (OriginChannel::Hub, "hub"),
    (OriginChannel::Subscription, "subscription"),
];

/// What an import tells the store about the object it made.
#[derive(Clone, Debug)]
pub struct NewOrigin {
    pub channel: OriginChannel,
    pub source: Reference,
    pub media_type: MediaType,
    pub foreign_uid: String,
    pub foreign_recurrence_id: Option<LocalDateTime>,
    pub foreign_etag: Option<String>,
    pub prod_id: Option<String>,
    pub importer: String,
    /// The received file and its type, when it is kept.
    pub original: Option<(Vec<u8>, MediaType)>,
}

pub use fs_objects::v5::original::{hex32, original_iri, sha256_hex};

/// Write the origin row and the original (the layer's `record`, with the trait's types); `rev_at_import` is the
/// revision the object has when the import is done.
///
/// # Errors
/// `Invalid` naming `table.column`, or the engine's error.
pub async fn record(
    c: &Conn<'_>,
    id: ObjectId,
    o: &NewOrigin,
    rev_at_import: Revision,
    now: i64,
) -> Result<()> {
    let ours = layer::NewOrigin {
        channel: word(&CHANNELS, o.channel).to_owned(),
        source: o.source.clone(),
        media_type: o.media_type.clone(),
        foreign_uid: o.foreign_uid.clone(),
        foreign_recurrence_id: o.foreign_recurrence_id,
        foreign_etag: o.foreign_etag.clone(),
        prod_id: o.prod_id.clone(),
        importer: o.importer.clone(),
        original: o.original.clone(),
    };
    layer::record(c.inner(), &DOMAIN, id, &ours, rev_at_import, now)
        .await
        .map_err(adapt)
}

/// The origin of an object, `None` when it was not imported.
///
/// # Errors
/// `Unavailable` for a row that does not read back.
pub async fn read_origin(c: &Conn<'_>, id: ObjectId) -> Result<Option<Origin>> {
    let cols: Vec<&str> = OBJECT_ORIGIN.cols.iter().map(|x| x.name).collect();
    let rows = c
        .query(
            &format!(
                "SELECT {} FROM object_origin WHERE object = ?",
                cols.join(", ")
            ),
            vec![id_value(id)],
        )
        .await?;
    let Some(r) = rows.first() else {
        return Ok(None);
    };
    let original = match (
        get_opt_text(r, 11)?,
        get_opt_text(r, 12)?,
        get_opt_text(r, 13)?,
    ) {
        (Some(iri), Some(hex), Some(media)) => Some(FileRef {
            iri: reference_of(&iri)?,
            sha256: from_hex(&hex)?,
            media_type: MediaType(media),
        }),
        _ => None,
    };
    Ok(Some(Origin {
        channel: from_word(&CHANNELS, &get_text(r, 1)?)?,
        source: reference_of(&get_text(r, 2)?)?,
        media_type: MediaType(get_text(r, 3)?),
        foreign_uid: get_text(r, 4)?,
        foreign_recurrence_id: get_opt_int(r, 5)?.map(int_to_local).transpose()?,
        foreign_etag: get_opt_text(r, 6)?,
        prod_id: get_opt_text(r, 7)?,
        imported: Utc::from_millis(get_int(r, 8)?),
        importer: get_text(r, 9)?,
        rev_at_import: Revision(u64::try_from(get_int(r, 10)?).unwrap_or(0)),
        original,
    }))
}

fn from_hex(hex: &str) -> Result<[u8; 32]> {
    let bad = || crate::codec::damaged("sha256");
    let mut out = [0_u8; 32];
    if hex.len() != 64 {
        return Err(bad());
    }
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).map_err(|_| bad())?;
    }
    Ok(out)
}

/// The kept original of an object (bytes and media type).
///
/// # Errors
/// `Unavailable` for a row that does not read back.
pub async fn original_of(c: &Conn<'_>, id: ObjectId) -> Result<Option<(Vec<u8>, MediaType)>> {
    layer::original_of(c.inner(), id).await.map_err(adapt)
}

/// Mark a part this program cannot interpret (the part stays in the original).
///
/// # Errors
/// `Invalid` or the engine's error.
pub async fn set_unread(c: &Conn<'_>, id: ObjectId, field: &str) -> Result<()> {
    c.insert(&UNREAD, vec![id_value(id), text(field)]).await
}

/// The parts of an object that could not be interpreted (field names).
///
/// # Errors
/// `Unavailable` for a row that does not read back.
pub async fn read_unread(c: &Conn<'_>, id: ObjectId) -> Result<Vec<String>> {
    let rows = c
        .query(
            "SELECT field FROM calendar_unread WHERE appointment = ? ORDER BY field",
            vec![id_value(id)],
        )
        .await?;
    rows.iter().map(|r| get_text(r, 0)).collect()
}

/// An edit of the rule of a series whose rule is kept as written is refused (`Unsupported`, never a silent change).
///
/// # Errors
/// `Unsupported` for such an edit.
pub async fn refuse_rule_edit(
    c: &Conn<'_>,
    id: ObjectId,
    p: &fs_calendars_testdata::draft::AppointmentPatch,
) -> Result<()> {
    // The start is part of it: the kept `EXDATE` and exceptions point at the old times. Giving back the rules as they
    // are is no edit (R1: the original rule travels with the appointment).
    let (rules, excluded) = crate::rules::read_rules(c, id).await?;
    let edits = p.when.is_some()
        || p.recurrence.as_ref().is_some_and(|r| *r != rules)
        || p.excluded_recurrence
            .as_ref()
            .is_some_and(|r| *r != excluded);
    if edits && !read_unread(c, id).await?.is_empty() {
        return Err(fs_calendars_testdata::draft::Error::Unsupported {
            capability: "event.update".to_owned(),
            reason: "the rule and the start of this series are kept as written and cannot be edited here (Serie nur teilweise darstellbar)".to_owned(),
        });
    }
    Ok(())
}
