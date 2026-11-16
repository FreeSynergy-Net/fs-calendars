//! The v5 store: one connection with `PRAGMA foreign_keys = ON`, one transaction per write (R3, R4).
//!
//! Pattern: Adapter (the turso engine behind typed helpers); `Conn` is a guard that owns the connection for one
//! unit of work. `finish` commits or rolls back; a guard dropped without `finish` leaves the transaction open, so
//! every write path ends in `finish`.

use fs_calendar_testdata::draft::{Error, Result};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::{Mutex, MutexGuard};
use turso::{Builder, Connection, Value};

use crate::codec::{Row, invalid};
use crate::spec::Table;
use crate::typed::check_row;

/// The store.
pub struct Store {
    _db: turso::Database,
    conn: Mutex<Connection>,
    /// Set when a unit of work was dropped without `finish` (a cancelled future): the next access rolls back first.
    dirty: Arc<AtomicBool>,
}

/// The connection for one unit of work.
pub struct Conn<'a> {
    guard: MutexGuard<'a, Connection>,
    in_tx: bool,
    dirty: Arc<AtomicBool>,
}

impl Drop for Conn<'_> {
    fn drop(&mut self) {
        if self.in_tx {
            self.dirty.store(true, Ordering::SeqCst);
        }
    }
}

/// An engine error as the trait's error: a refused constraint is `Invalid`, anything else `Unavailable`.
#[must_use]
pub fn engine(e: &turso::Error) -> Error {
    let msg = e.to_string();
    let low = msg.to_lowercase();
    if [
        "constraint failed",
        "constraint violation",
        "foreign key",
        "cannot store",
        "datatype mismatch",
    ]
    .iter()
    .any(|w| low.contains(w))
    {
        invalid("store", &msg)
    } else {
        Error::Unavailable(msg)
    }
}

impl Store {
    /// Open (creating if needed) the store at `path`; `:memory:` is volatile. Registers the store's own tables.
    ///
    /// # Errors
    /// `Unavailable` when the engine cannot open the file.
    pub async fn open(path: &str, now: i64) -> Result<Self> {
        let db = Builder::new_local(path)
            .build()
            .await
            .map_err(|e| engine(&e))?;
        let conn = db.connect().map_err(|e| engine(&e))?;
        conn.execute("PRAGMA foreign_keys = ON", ())
            .await
            .map_err(|e| engine(&e))?;
        let store = Self {
            _db: db,
            conn: Mutex::new(conn),
            dirty: Arc::new(AtomicBool::new(false)),
        };
        let c = store.begin().await?;
        let r = crate::registry::bootstrap(&c, now).await;
        c.finish(r).await?;
        Ok(store)
    }

    /// The connection; a transaction that a cancelled future left open is rolled back first (K1).
    async fn lock(&self) -> MutexGuard<'_, Connection> {
        let guard = self.conn.lock().await;
        if self.dirty.swap(false, Ordering::SeqCst) {
            let _ = guard.execute("ROLLBACK", ()).await;
        }
        guard
    }

    /// Lock the connection and open a transaction (`BEGIN IMMEDIATE`).
    ///
    /// # Errors
    /// `Unavailable` when the engine refuses.
    pub async fn begin(&self) -> Result<Conn<'_>> {
        let guard = self.lock().await;
        guard
            .execute("BEGIN IMMEDIATE", ())
            .await
            .map_err(|e| engine(&e))?;
        Ok(Conn {
            guard,
            in_tx: true,
            dirty: self.dirty.clone(),
        })
    }

    /// Lock the connection for reading (no transaction).
    pub async fn read(&self) -> Conn<'_> {
        Conn {
            guard: self.lock().await,
            in_tx: false,
            dirty: self.dirty.clone(),
        }
    }

    /// Register a domain (R1): its tables are created and written down.
    ///
    /// # Errors
    /// `Invalid` for a reserved prefix or a clash.
    pub async fn register_domain(
        &self,
        prefix: &str,
        version: i64,
        about: &str,
        tables: &[Table],
        now: i64,
    ) -> Result<()> {
        let c = self.begin().await?;
        let r = crate::registry::register(&c, prefix, version, about, tables, now).await;
        c.finish(r).await
    }
}

impl Conn<'_> {
    /// Rows of a query.
    ///
    /// # Errors
    /// The engine's error.
    pub async fn query(&self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>> {
        let mut rows = self
            .guard
            .query(sql, turso::params_from_iter(params))
            .await
            .map_err(|e| engine(&e))?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| engine(&e))? {
            let mut r = Vec::with_capacity(row.column_count());
            for i in 0..row.column_count() {
                r.push(row.get_value(i).map_err(|e| engine(&e))?);
            }
            out.push(r);
        }
        Ok(out)
    }

    /// Execute a statement; the number of rows it changed.
    ///
    /// # Errors
    /// The engine's error.
    pub async fn exec(&self, sql: &str, params: Vec<Value>) -> Result<u64> {
        self.guard
            .execute(sql, turso::params_from_iter(params))
            .await
            .map_err(|e| engine(&e))
    }

    /// Execute a statement without parameters.
    ///
    /// # Errors
    /// The engine's error.
    pub async fn run(&self, sql: &str) -> Result<()> {
        self.exec(sql, Vec::new()).await.map(|_| ())
    }

    /// Insert one row after the checks of R2 (`values` in column order).
    ///
    /// # Errors
    /// `Invalid` naming `table.column`.
    pub async fn insert(&self, t: &Table, values: Vec<Value>) -> Result<()> {
        check_row(t, &values)?;
        let names: Vec<&str> = t.cols.iter().map(|c| c.name).collect();
        let marks = vec!["?"; names.len()].join(",");
        self.exec(
            &format!(
                "INSERT INTO {} ({}) VALUES ({marks})",
                t.name,
                names.join(",")
            ),
            values,
        )
        .await
        .map(|_| ())
    }

    /// Insert or update on the primary key `key` (never `REPLACE`: that would cascade, draft R4).
    ///
    /// # Errors
    /// `Invalid` naming `table.column`.
    pub async fn upsert(&self, t: &Table, key: &[&str], values: Vec<Value>) -> Result<()> {
        check_row(t, &values)?;
        let names: Vec<&str> = t.cols.iter().map(|c| c.name).collect();
        let marks = vec!["?"; names.len()].join(",");
        let set: Vec<String> = names
            .iter()
            .filter(|n| !key.contains(n))
            .map(|n| format!("{n} = excluded.{n}"))
            .collect();
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({marks}) ON CONFLICT ({}) DO UPDATE SET {}",
            t.name,
            names.join(","),
            key.join(","),
            set.join(", ")
        );
        self.exec(&sql, values).await.map(|_| ())
    }

    /// End the unit of work: commit on `Ok`, roll back on `Err` (nothing is written anywhere).
    ///
    /// # Errors
    /// The error of `r`, or of the commit.
    pub async fn finish<T>(mut self, r: Result<T>) -> Result<T> {
        if !self.in_tx {
            return r;
        }
        // `in_tx` stays true until the engine has answered: a future cancelled before that point is dropped with
        // `in_tx` set, so `Drop` marks the connection and the next access rolls back (K1).
        Suspend(false).await;
        let out = match r {
            Ok(v) => match self.guard.execute("COMMIT", ()).await {
                Ok(_) => Ok(v),
                Err(e) => {
                    let _ = self.guard.execute("ROLLBACK", ()).await;
                    Err(engine(&e))
                }
            },
            Err(e) => {
                let _ = self.guard.execute("ROLLBACK", ()).await;
                Err(e)
            }
        };
        self.in_tx = false;
        out
    }
}

/// Gives the scheduler one turn (a real suspension point, so that a cancel there can be tested).
struct Suspend(bool);

impl std::future::Future for Suspend {
    type Output = ();
    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<()> {
        if self.0 {
            return std::task::Poll::Ready(());
        }
        self.0 = true;
        cx.waker().wake_by_ref();
        std::task::Poll::Pending
    }
}
