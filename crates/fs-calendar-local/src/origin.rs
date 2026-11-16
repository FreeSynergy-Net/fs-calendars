//! Where an imported object came from, and the file as it was received (`object_origin`, `object_original`), plus
//! the parts of it this program cannot interpret (`calendar_unread`).
//!
//! Pattern: Transaction script. The original is immutable after insert (ADR 0030); "unchanged since import" is
//! computed (`rev = rev_at_import`), never stored (ADR 0085).

use fs_calendar_testdata::draft::{
    BasicValue, FileRef, LocalDateTime, MediaType, ObjectId, Origin, OriginChannel, Reference,
    Result, Revision, Utc,
};
use sha2::{Digest, Sha256};
use turso::Value;

use crate::codec::{
    get_int, get_opt_int, get_opt_text, get_text, id_value, int, int_to_local, local_to_int,
    opt_int, opt_text, reference_of, text,
};
use crate::series_tables::UNREAD;
use crate::store::Conn;
use crate::words::{from_word, word};
use fs_objects::v5::tables_more::{OBJECT_ORIGIN, OBJECT_ORIGINAL};

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

/// Lower-case hex SHA-256 of bytes.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .concat()
}

/// Lower-case hex of 32 bytes.
#[must_use]
pub fn hex32(b: &[u8; 32]) -> String {
    b.iter()
        .map(|x| format!("{x:02x}"))
        .collect::<Vec<_>>()
        .concat()
}

/// The `urn:sha256:<hex>` IRI of a kept original.
#[must_use]
pub fn original_iri(hex: &str) -> String {
    format!("urn:sha256:{hex}")
}

/// Write the origin row and the original; `rev_at_import` is the revision the object has when the import is done.
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
    let kept = o.original.as_ref().map(|(b, m)| (sha256_hex(b), m));
    if let Some((bytes, media)) = &o.original {
        let hex = sha256_hex(bytes);
        c.insert(
            &OBJECT_ORIGINAL,
            vec![
                id_value(id),
                text(&media.0),
                Value::Blob(bytes.clone()),
                text(&hex),
                int(now),
                Value::Null,
            ],
        )
        .await?;
    }
    let (iri, hex, media) = match &kept {
        Some((h, m)) => (opt_text(Some(&original_iri(h))), text(h), text(&m.0)),
        None => (Value::Null, Value::Null, Value::Null),
    };
    c.insert(
        &OBJECT_ORIGIN,
        vec![
            id_value(id),
            text(word(&CHANNELS, o.channel)),
            text(&o.source.canonical()),
            text(&o.media_type.0),
            text(&o.foreign_uid),
            opt_int(o.foreign_recurrence_id.map(local_to_int).transpose()?),
            opt_text(o.foreign_etag.as_deref()),
            opt_text(o.prod_id.as_deref()),
            int(now),
            text(&o.importer),
            int(i64::try_from(rev_at_import.0).unwrap_or(0)),
            iri,
            hex,
            media,
        ],
    )
    .await
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
    let rows = c
        .query(
            "SELECT bytes, media_type FROM object_original WHERE object = ?",
            vec![id_value(id)],
        )
        .await?;
    match rows.first().map(|r| (r.first().cloned(), get_text(r, 1))) {
        Some((Some(Value::Blob(b)), m)) => Ok(Some((b, MediaType(m?)))),
        Some(_) => Err(crate::codec::damaged("original")),
        None => Ok(None),
    }
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
    p: &fs_calendar_testdata::draft::AppointmentPatch,
) -> Result<()> {
    // The start is part of it: the kept `EXDATE` and exceptions point at the old times.
    if (p.recurrence.is_some() || p.excluded_recurrence.is_some() || p.when.is_some())
        && !read_unread(c, id).await?.is_empty()
    {
        return Err(fs_calendar_testdata::draft::Error::Unsupported {
            capability: "event.update".to_owned(),
            reason: "the rule and the start of this series are kept as written and cannot be edited here (Serie nur teilweise darstellbar)".to_owned(),
        });
    }
    Ok(())
}
