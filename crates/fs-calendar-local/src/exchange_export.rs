//! The writing half of the exchange: what is taken from the received original (ADR 0030) and what an edit cannot write.
//!
//! Pattern: Adapter helpers. A series whose rule is outside the engine's subset is written back from the original:
//! rule, `EXDATE` and exceptions as received. Parts that are only in the original are named as losses when an edited
//! object is written.

use fs_calendar_testdata::draft::{Loss, MediaType};
use fs_jscalendar::ical::parse_entries;
use serde_json::{Map, Value};

use crate::exchange::{ICS, JSCAL};
use crate::jscal_out::RECURRENCE_KEYS;
use crate::provider_admin::ExportSource;

/// The members of the received event that this program does not store.
const ONLY_IN_ORIGINAL: [&str; 5] = [
    "locations",
    "virtualLocations",
    "links",
    "localizations",
    "replyTo",
];

/// The received event (JSON, upgraded) of an object, out of its kept original.
#[must_use]
pub fn original_event(src: &ExportSource) -> Option<Value> {
    let (bytes, MediaType(media)) = src.original.as_ref()?;
    let text = std::str::from_utf8(bytes).ok()?;
    let entries: Vec<Value> = match media.as_str() {
        ICS => parse_entries(text, "Event").ok()?.0,
        JSCAL => {
            let up = fs_jscalendar::upgrade::upgrade(serde_json::from_str(text).ok()?).value;
            match up.get("entries").and_then(Value::as_array) {
                Some(a) => a.clone(),
                None => vec![up],
            }
        }
        _ => return None,
    };
    let uid = &src.appointment.uid;
    entries
        .into_iter()
        .find(|e| e.get("uid").and_then(Value::as_str) == Some(uid.as_str()))
}

/// The whole recurrence part of the original, when the rule of this object is marked as not read.
#[must_use]
pub fn kept_recurrence(src: &ExportSource) -> Option<Map<String, Value>> {
    if !src.unread.iter().any(|f| f == "recurrence") {
        return None;
    }
    let ev = original_event(src)?;
    Some(
        RECURRENCE_KEYS
            .iter()
            .filter_map(|k| ev.get(*k).map(|v| ((*k).to_owned(), v.clone())))
            .collect(),
    )
}

/// The parts that only the original holds, named for an object that is written anew.
#[must_use]
pub fn only_in_original(src: &ExportSource) -> Vec<Loss> {
    let Some(ev) = original_event(src) else {
        return Vec::new();
    };
    ONLY_IN_ORIGINAL
        .iter()
        .filter(|k| ev.get(**k).is_some_and(|v| !v.is_null()))
        .map(|k| Loss {
            uid: Some(src.appointment.uid.clone()),
            field: (*k).to_owned(),
            reason: "only in the received original, not stored: not written".into(),
        })
        .collect()
}
