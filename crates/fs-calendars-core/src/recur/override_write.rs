//! Writing one instance of a series as an override (RFC 8984 `recurrenceOverrides`): the pure rules.
//!
//! Moved here from `fs-calendars-app/src/backend/occurrence.rs` (sorting run S2b, not compiled): `NOT_OVERRIDDEN`,
//! `key_of_id`, `object`, `with_patch` verbatim; `baseline` and `patch_of` are the two blocks of the closure of
//! `change_occurrence` (occurrence.rs lines 138-163) lifted into functions, same statements, the edit (`apply` of the
//! form values) stays with the caller. The store calls (`is_occurrence`, `write_override`) stay in the window crate.
//!
//! Pattern: Command (continued from `patch.rs`, which reads a patch) — a change becomes the override: the properties
//! that differ from the series after the edit; a delete becomes `{"excluded": true}`.

use serde_json::{Map, Value};

/// Properties that belong to the series or to the store, never to an override.
pub const NOT_OVERRIDDEN: &[&str] = &[
    "@type",
    "version",
    "uid",
    "prodId",
    "created",
    "updated",
    "sequence",
    "recurrenceRule",
    "recurrenceOverrides",
    "recurrenceId",
    "recurrenceIdTimeZone",
];

/// The recurrence id in an id `urn:uuid:<id>#<recurrence id>`, if the id names one instance.
pub fn key_of_id(id: &str) -> Option<String> {
    id.split_once('#').map(|(_, key)| key.to_owned())
}

/// A JSON object as its map; anything else is an empty map.
pub fn object(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

/// The master with the top-level members of `patch` applied (nested paths are left for the engine).
pub fn with_patch(master: &Map<String, Value>, patch: &Map<String, Value>) -> Map<String, Value> {
    let mut out = master.clone();
    for (k, v) in patch.iter().filter(|(k, _)| !k.contains('/')) {
        if v.is_null() {
            out.remove(k);
        } else {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

/// The instance before the edit: the master with the old override applied. It starts at its key (the series' start
/// only for the first one) unless an override moved it, so a move back to the start of the series is a difference.
pub fn baseline(
    master_json: &Map<String, Value>,
    old: &Map<String, Value>,
    key: &str,
) -> Map<String, Value> {
    let mut baseline = with_patch(master_json, old);
    if !old.contains_key("start") {
        baseline.insert("start".to_owned(), Value::String(key.to_owned()));
    }
    baseline
}

/// The new override: the old one plus every property in which the edited instance differs from the baseline.
/// `None` when nothing differs (nothing is written, the revision stays).
pub fn patch_of(
    baseline: &Map<String, Value>,
    edited: &Map<String, Value>,
    old: &Map<String, Value>,
    key: &str,
) -> Option<Map<String, Value>> {
    let mut patch = old.clone();
    for k in baseline
        .keys()
        .chain(edited.keys())
        .filter(|k| !NOT_OVERRIDDEN.contains(&k.as_str()))
    {
        if baseline.get(k) != edited.get(k) {
            patch.insert(k.clone(), edited.get(k).cloned().unwrap_or(Value::Null));
        }
    }
    // a start equal to the key is the default, not an override
    if patch.get("start").and_then(Value::as_str) == Some(key) {
        patch.remove("start");
    }
    (patch != *old).then_some(patch)
}

/// The override of a deleted instance: it is excluded, the series stays.
pub fn exclusion() -> Map<String, Value> {
    Map::from_iter([("excluded".to_owned(), Value::Bool(true))])
}
