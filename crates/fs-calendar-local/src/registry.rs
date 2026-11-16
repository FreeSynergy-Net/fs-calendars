//! R1: the registry of domains and columns (`fs_domain`, `fs_column`), and the registration of a domain.
//!
//! Pattern: Registry (a program registers its tables; the store creates them and writes them down).

use fs_calendar_testdata::draft::Result;
use turso::Value;

use crate::codec::{int, invalid, text};
use crate::object_tables::ALL as OBJECT_TABLES;
use crate::spec::{
    Col,
    Logical::{Bool, Int, Text, Utc},
    Table,
};
use crate::store::Conn;

pub const FS_DOMAIN: Table = Table {
    name: "fs_domain",
    cols: &[
        Col::req("prefix", Text),
        Col::req("version", Int),
        Col::req("description", Text),
        Col::req("applied", Utc),
    ],
    constraints: &["PRIMARY KEY (prefix)"],
    indexes: &[],
};
pub const FS_COLUMN: Table = Table {
    name: "fs_column",
    cols: &[
        Col::req("domain", Text),
        Col::req("tbl", Text),
        Col::req("col", Text),
        Col::req("logical", Text),
        Col::req("nullable", Bool),
        Col::req("derived", Bool),
    ],
    constraints: &["PRIMARY KEY (domain, tbl, col)"],
    indexes: &[],
};

/// The prefixes a program may not take (draft R1).
pub const RESERVED: [&str; 5] = ["object", "activity", "person", "fs", "setting"];
/// The reserved domain that holds the store's own tables.
pub const OWN: &str = "fs_";

/// Create the registry and the store's own tables and register them under the reserved domain.
///
/// # Errors
/// The engine's error.
pub async fn bootstrap(c: &Conn<'_>, now: i64) -> Result<()> {
    c.run(&FS_DOMAIN.ddl()).await?;
    c.run(&FS_COLUMN.ddl()).await?;
    let mut own: Vec<Table> = OBJECT_TABLES.to_vec();
    own.extend([FS_DOMAIN, FS_COLUMN]);
    register(c, OWN, 2, "tables of the store itself", &own, now).await?;
    let id = crate::codec::id_text(crate::clock::new_id(now)?);
    c.exec(
        "INSERT INTO meta (key, value) VALUES ('store_id', ?) ON CONFLICT (key) DO NOTHING",
        vec![text(&id)],
    )
    .await?;
    Ok(())
}

/// Register a domain: create its tables and write down its columns. Same prefix, same version and same tables
/// again is a no-op; a different table list under a used prefix, or a reserved prefix, is refused.
///
/// # Errors
/// `Invalid` for a reserved or malformed prefix, a table outside the prefix, or a clash; the engine's error.
pub async fn register(
    c: &Conn<'_>,
    prefix: &str,
    version: i64,
    about: &str,
    tables: &[Table],
    now: i64,
) -> Result<()> {
    let own = prefix == OWN;
    let shape = !prefix.is_empty()
        && prefix
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_');
    if !own && (!shape || prefix.ends_with('_') || RESERVED.contains(&prefix)) {
        return Err(invalid("prefix", "reserved or malformed"));
    }
    if !own {
        let lead = format!("{prefix}_");
        if let Some(t) = tables.iter().find(|t| !t.name.starts_with(&lead)) {
            return Err(invalid(
                "tables",
                &format!("{} does not start with {lead}", t.name),
            ));
        }
    }
    let mut names: Vec<String> = tables.iter().map(|t| t.name.to_owned()).collect();
    names.sort();
    let rows = c
        .query(
            "SELECT version FROM fs_domain WHERE prefix = ?",
            vec![text(prefix)],
        )
        .await?;
    if !rows.is_empty() {
        let known = c
            .query(
                "SELECT DISTINCT tbl FROM fs_column WHERE domain = ? ORDER BY tbl",
                vec![text(prefix)],
            )
            .await?;
        let known: Vec<String> = known
            .iter()
            .filter_map(|r| {
                if let Some(Value::Text(t)) = r.first() {
                    Some(t.clone())
                } else {
                    None
                }
            })
            .collect();
        let same_version = matches!(rows[0].first(), Some(Value::Integer(v)) if *v == version);
        return if known == names && same_version {
            Ok(())
        } else {
            Err(invalid(
                "prefix",
                "already registered with other tables or another version",
            ))
        };
    }
    for t in tables {
        c.run(&t.ddl()).await?;
        for ix in t.indexes {
            c.run(ix).await?;
        }
        for col in t.cols {
            let row = vec![
                text(prefix),
                text(t.name),
                text(col.name),
                text(col.logical.label()),
                Value::Integer(i64::from(col.null)),
                Value::Integer(i64::from(col.derived)),
            ];
            c.insert(&FS_COLUMN, row).await?;
        }
    }
    c.insert(
        &FS_DOMAIN,
        vec![text(prefix), int(version), text(about), int(now)],
    )
    .await
}
