//! Time and ids: a clock a test can set, and monotonic UUIDv7-shaped object ids.
//!
//! Pattern: Strategy (the clock is a function value, so a test fixes the time).

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::Ordering;

use fs_calendar_testdata::draft::{Error, ObjectId, Result};

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

/// Last `(ms, counter)` handed out in this process: ids made here only grow, whatever the clock does.
static LAST: Mutex<(u64, u16)> = Mutex::new((0, 0));

/// Random bytes from the OS. Without them there is no id: a clock and a process number are not randomness, and an
/// id that may repeat on another device is worse than a refused write.
fn random<const N: usize>() -> Result<[u8; N]> {
    let mut b = [0_u8; N];
    getrandom::fill(&mut b)
        .map_err(|e| Error::Unavailable(format!("no OS randomness for an object id: {e}")))?;
    Ok(b)
}

/// A new object id, UUIDv7 (RFC 9562): 48 bit milliseconds, version 7, a 12 bit counter (random start in each new
/// millisecond, then +1), variant bits and 62 random bits. Unique across processes and devices by the randomness,
/// strictly increasing within one process by the counter; a clock that goes back keeps the last millisecond.
/// # Errors
/// `Unavailable` when the OS gives no randomness.
pub fn new_id(now_ms: i64) -> Result<ObjectId> {
    let now = u64::try_from(now_ms).unwrap_or(0) & 0xFFFF_FFFF_FFFF;
    let (ms, seq) = {
        let mut last = LAST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let start = u16::from_be_bytes(random::<2>()?) & 0x7FF;
        let next = if now > last.0 {
            (now, start)
        } else if last.1 >= 0xFFF {
            (last.0 + 1, start)
        } else {
            (last.0, last.1 + 1)
        };
        *last = next;
        next
    };
    let mut b = [0_u8; 16];
    b[..6].copy_from_slice(&ms.to_be_bytes()[2..]);
    b[6] = 0x70 | u8::try_from(seq >> 8).unwrap_or(0);
    b[7] = u8::try_from(seq & 0xFF).unwrap_or(0);
    b[8..].copy_from_slice(&random::<8>()?);
    b[8] = (b[8] & 0x3F) | 0x80;
    Ok(ObjectId(b))
}
