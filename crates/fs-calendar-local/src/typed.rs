//! The check of zone names, the one part of R2 that stays with the program: the layer checks that a zone name is well
//! formed, only the calendar has the zone database and knows whether it exists.
//!
//! Pattern: Strategy (one check, run before the layer's own checks of the row).

use std::collections::HashMap;

use fs_calendar_testdata::draft::{Result, Zone};
use fs_calendar_testdata::zone::resolve;
use turso::Value;

use fs_objects::v5::spec::{Logical, Table};

/// Every `ZONE` value of the row must name a zone that exists.
///
/// # Errors
/// `UnknownZone` for a zone that does not exist (never guessed); a malformed name or a custom zone is left to the
/// layer, which refuses it as `Invalid`.
pub fn check_zones(t: &Table, values: &[Value]) -> Result<()> {
    for (c, v) in t.cols.iter().zip(values) {
        if let (Logical::Zone, Value::Text(name)) = (c.logical, v) {
            if name.starts_with('/') {
                continue;
            }
            if let Ok(zone) = Zone::iana(name) {
                resolve(&zone, &HashMap::new())?;
            }
        }
    }
    Ok(())
}
