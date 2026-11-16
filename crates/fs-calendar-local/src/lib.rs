//! `LocalAppointments`: the first provider of the `Appointments` trait, over typed v5 tables (pilot
//! `programm-grundgeruest`, core step 1).
//!
//! Pattern: Repository (one provider of the trait) over a small typed store; the store layer follows the fs-objects v5
//! draft (`docs/v5-entwurf.md`) and moves into fs-objects when that draft is accepted.

#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::many_single_char_names
)]

pub mod calendar_tables;
pub mod change_codec;
pub mod clock;
pub mod codec;
pub mod cuts;
pub mod envelope;
pub mod exchange;
pub mod exchange_export;
pub mod exchange_import;
pub mod fields;
pub mod fields_rule;
pub mod files;
pub mod history;
pub mod import_ops;
pub mod jscal_in;
pub mod jscal_out;
pub mod jscal_parts;
pub mod jscal_rule;
pub mod jscal_time;
pub mod jscal_util;
pub mod object_tables;
pub mod ops;
pub mod origin;
pub mod provider;
pub mod provider_admin;
pub mod query;
pub mod read;
pub mod read_children;
pub mod registry;
pub mod row_values;
pub mod rules;
pub mod scope_delete;
pub mod scope_ops;
pub mod series;
pub mod series_hide;
pub mod series_tables;
pub mod spec;
pub mod store;
pub mod sweep;
pub mod typed;
pub mod words;
pub mod write;

pub use clock::Clock;
pub use provider::LocalAppointments;
pub use store::Store;
