//! Time and ids: a clock a test can set, and monotonic UUIDv7-shaped object ids.
//!
//! Pattern: Strategy (the clock is a function value, so a test fixes the time).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use fs_calendar_testdata::draft::ObjectId;

/// Milliseconds since the epoch, from the system or from a test.
#[derive(Clone)]
pub struct Clock(Arc<dyn Fn() -> i64 + Send + Sync>);

impl Clock {
    /// The system clock.
    #[must_use]
    pub fn system() -> Self {
        Self(Arc::new(|| {
            let d = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            i64::try_from(d.as_millis()).unwrap_or(i64::MAX)
        }))
    }

    /// A clock that reads a shared counter (tests move it).
    #[must_use]
    pub fn from_counter(ms: Arc<std::sync::atomic::AtomicI64>) -> Self {
        Self(Arc::new(move || ms.load(Ordering::SeqCst)))
    }

    /// Now, in milliseconds.
    #[must_use]
    pub fn now(&self) -> i64 {
        (self.0)()
    }
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A new object id: 48 bit milliseconds, version 7, variant bits, and a process-wide counter that only grows
/// (so ids made in one process are strictly increasing).
#[must_use]
pub fn new_id(now_ms: i64) -> ObjectId {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst) + 1;
    let ms = u64::try_from(now_ms).unwrap_or(0).to_be_bytes();
    let mut b = [0_u8; 16];
    b[..6].copy_from_slice(&ms[2..]);
    b[6] = 0x70;
    b[8..].copy_from_slice(&n.to_be_bytes());
    b[8] = (b[8] & 0x3f) | 0x80;
    ObjectId(b)
}
