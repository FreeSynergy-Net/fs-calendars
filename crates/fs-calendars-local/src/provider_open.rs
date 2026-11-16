//! Opening `LocalAppointments` and its small accessors.
//!
//! Pattern: Adapter (inherent `impl` block; the trait impl is in `provider`).

use fs_calendars_testdata::draft::{PersonRef, Result};

use crate::clock::Clock;
use crate::codec::text;
use crate::provider::LocalAppointments;
use crate::store::Store;

impl LocalAppointments {
    /// Open the store at `path` (`:memory:` is volatile) and register the `calendar_` domain.
    ///
    /// # Errors
    /// `Unavailable` when the engine cannot open the file; `Invalid` on a clash in the registry.
    pub async fn open(path: &str, actor: PersonRef, clock: Clock) -> Result<Self> {
        let now = clock.now();
        let store = Store::open(path, now).await?;
        Ok(Self {
            store,
            actor,
            clock,
        })
    }

    /// Whether an appointment with this `uid` exists (a trashed one too: `uid` is unique).
    ///
    /// # Errors
    /// The engine's error.
    pub async fn uid_exists(&self, uid: &str) -> Result<bool> {
        let c = self.store.read().await;
        let rows = c
            .query(
                "SELECT 1 FROM calendar_appointment WHERE uid = ? AND master IS NULL",
                vec![text(uid)],
            )
            .await?;
        Ok(!rows.is_empty())
    }

    /// The acting person.
    #[must_use]
    pub fn actor(&self) -> &PersonRef {
        &self.actor
    }

    /// The store (the tests look at the rows).
    #[must_use]
    pub fn store(&self) -> &Store {
        &self.store
    }
}
