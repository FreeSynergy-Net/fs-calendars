//! The `Appointments` trait draft (pilot `programm-grundgeruest`, plan step 2).
//!
//! `appointments_v1/` is a byte-exact copy of the folder `.scratch/pilot-calendar/appointments_v1/` (folder hash, `tools/hash-ordner.sh`:
//! `2b7fc98b5bf777a0c4f32ddf6d9c2b3ac5c784425df0ef2fe61c31c8ec5519b8`, lock candidate 4). It is NOT locked yet (ADR 0085 is
//! closed, the lock comes with `trait-sperre`): the types of its `shared` module are stand-ins. When the trait is locked, step 3 replaces this
//! module by `pub use fs_calendars_core::interface::appointments::*` and nothing else in this crate changes.
//! Do not edit the copy: a different trait is a new plan.

#[rustfmt::skip]
#[allow(clippy::all, clippy::pedantic, missing_docs)]
pub mod appointments_v1;

pub use appointments_v1::shared::*;
pub use appointments_v1::*;
