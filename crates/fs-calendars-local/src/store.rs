//! The calendar's view of the v5 store: the shared layer (`fs_objects::v5`) with the trait's error and types.
//!
//! Pattern: Adapter (the layer speaks `fs-types`; the locked trait speaks its stand-ins, so every call maps the error
//! and, where an id or a person crosses, the type). When the trait moves to `fs-types` this file collapses to a
//! `pub use`. `Conn` is the guard of the layer (drop guard, `finish`); `Borrowed` is the connection a period-run hook
//! gets from the layer.

use fs_calendars_testdata::draft::Result;
use fs_objects::v5;
use turso::Value;

use crate::bridge::adapt;
use crate::codec::Row;
use crate::registry::DOMAIN;
use crate::typed::check_zones;
use fs_objects::v5::spec::Table;

/// The store (one file, one program: the calendar's).
pub struct Store(v5::Store);

/// The connection for one unit of work.
pub enum Conn<'a> {
    /// The guard of a unit of work the calendar opened (`begin`, `read`).
    Own(v5::Conn<'a>),
    /// The connection of the layer's period run, lent to a hook.
    Borrowed(&'a v5::Conn<'a>),
}

impl Store {
    /// Open (creating if needed) the store at `path` (`:memory:` is volatile), register the `calendar_` domain.
    ///
    /// # Errors
    /// `Unavailable` when the engine cannot open the file or a step fails; `Invalid` on a clash in the registry.
    pub async fn open(path: &str, now: i64) -> Result<Self> {
        let s = Self::open_bare(path, now).await?;
        s.register_domain(&DOMAIN, now).await?;
        Ok(s)
    }

    /// Open the store with the layer's own tables only (no program domain yet).
    ///
    /// # Errors
    /// `Unavailable` when the engine cannot open the file.
    pub async fn open_bare(path: &str, now: i64) -> Result<Self> {
        v5::Store::open(path, now).await.map(Self).map_err(adapt)
    }

    /// Lock the connection and open a transaction (`BEGIN IMMEDIATE`).
    ///
    /// # Errors
    /// `Unavailable` when the engine refuses.
    pub async fn begin(&self) -> Result<Conn<'_>> {
        self.0.begin().await.map(Conn::Own).map_err(adapt)
    }

    /// Lock the connection for reading (no transaction).
    pub async fn read(&self) -> Conn<'_> {
        Conn::Own(self.0.read().await)
    }

    /// Register a program domain (the layer applies the steps that are missing).
    ///
    /// # Errors
    /// `Invalid` for a reserved prefix or a clash, `Unavailable` for a second program in this file or a failed step.
    pub async fn register_domain(&self, d: &v5::Domain, now: i64) -> Result<()> {
        self.0.register_domain(d, now).await.map_err(adapt)
    }
}

impl<'a> Conn<'a> {
    /// The layer's connection (for the layer's own functions).
    #[must_use]
    pub fn inner(&self) -> &v5::Conn<'a> {
        match self {
            Self::Own(c) => c,
            Self::Borrowed(c) => c,
        }
    }

    /// Rows of a query.
    ///
    /// # Errors
    /// The engine's error.
    pub async fn query(&self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>> {
        self.inner().query(sql, params).await.map_err(adapt)
    }

    /// Execute a statement; the number of rows it changed.
    ///
    /// # Errors
    /// The engine's error.
    pub async fn exec(&self, sql: &str, params: Vec<Value>) -> Result<u64> {
        self.inner().exec(sql, params).await.map_err(adapt)
    }

    /// Execute a statement without parameters.
    ///
    /// # Errors
    /// The engine's error.
    pub async fn run(&self, sql: &str) -> Result<()> {
        self.inner().run(sql).await.map_err(adapt)
    }

    /// Insert one row after the checks of the layer and of the zone names (`values` in column order).
    ///
    /// # Errors
    /// `Invalid` naming `table.column`, `UnknownZone` for a zone that does not exist.
    pub async fn insert(&self, t: &Table, values: Vec<Value>) -> Result<()> {
        check_zones(t, &values)?;
        self.inner().insert(t, values).await.map_err(adapt)
    }

    /// Insert or update on the primary key `key` (never `REPLACE`).
    ///
    /// # Errors
    /// As [`Conn::insert`].
    pub async fn upsert(&self, t: &Table, key: &[&str], values: Vec<Value>) -> Result<()> {
        check_zones(t, &values)?;
        self.inner().upsert(t, key, values).await.map_err(adapt)
    }

    /// End the unit of work: commit on `Ok`, roll back on `Err`. The calendar's own error survives the rollback.
    ///
    /// # Errors
    /// The error of `r`, or of the commit.
    pub async fn finish<T>(self, r: Result<T>) -> Result<T> {
        let Self::Own(c) = self else { return r };
        match r {
            Ok(v) => c.finish(Ok(v)).await.map_err(adapt),
            Err(e) => {
                let _ = c
                    .finish::<()>(Err(v5::Error::Unavailable(format!("{e:?}"))))
                    .await;
                Err(e)
            }
        }
    }
}
