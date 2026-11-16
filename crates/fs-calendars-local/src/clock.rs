//! Time and ids: the clock and the `UUIDv7` ids of the shared layer, the id in the trait's type.
//!
//! Pattern: Adapter (`Clock` is the layer's; `new_id` maps the layer's id and error to the stand-ins of the trait).

use fs_calendars_testdata::draft::{ObjectId, Result};
pub use fs_objects::v5::Clock;

use crate::convert::adapt;

/// A new object id, `UUIDv7` (RFC 9562), from the shared layer.
///
/// # Errors
/// `Unavailable` when the OS gives no randomness.
pub fn new_id(now_ms: i64) -> Result<ObjectId> {
    fs_objects::v5::new_id(now_ms).map_err(adapt)
}
