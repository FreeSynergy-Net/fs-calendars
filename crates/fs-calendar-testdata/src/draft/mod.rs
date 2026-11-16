//! The `Appointments` trait draft (pilot `programm-grundgeruest`, plan step 2).
//!
//! `appointments_v1.rs` is a byte-exact copy of `.scratch/pilot-calendar/appointments_v1.rs` (SHA-256
//! `f8e7fd1fbe3f89928f503c7d32f4fd608ce14138936e4541a77983a8e49c87ec`). It is NOT locked yet (ADR 0085 is a
//! draft): the types of its `shared` module are stand-ins. When the trait is locked, step 3 replaces this
//! module by `pub use fs_calendar_core::interface::appointments::*` and nothing else in this crate changes.
//! Do not edit the copy: a different trait is a new plan.

#[rustfmt::skip]
#[allow(clippy::all, clippy::pedantic, missing_docs)]
pub mod appointments_v1;

pub use appointments_v1::shared::*;
pub use appointments_v1::*;
