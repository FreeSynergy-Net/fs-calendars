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
pub mod fields;
pub mod history;
pub mod object_tables;
pub mod ops;
pub mod provider;
pub mod query;
pub mod read;
pub mod read_children;
pub mod registry;
pub mod row_values;
pub mod rules;
pub mod scope_delete;
pub mod scope_ops;
pub mod series;
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
