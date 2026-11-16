//! Test providers and the fixed sample data of FreeSynergy calendar (pilot `programm-grundgeruest`, step K0/F0).
//!
//! DRAFT: written against the not yet locked `Appointments` trait draft ([`draft`], state after the closing of ADR 0085, 2026-11-16). The frontend develops
//! against [`FakeAppointments`]; the core agent owns this crate.
//!
//! * [`FakeAppointments`]: in memory, all methods but `free_busy`; [`EmptyAppointments`] is the empty one (Z1).
//! * [`SlowAppointments`]: any provider, delayed (Z2). [`FailingAppointments`]: always `Unavailable` (Z3).
//! * [`FakePeople`]: the consumed `PersonDirectory`.
//! * [`sample`]: the fixed set of appointments (every case of the plan, section 5).

pub mod check;
pub mod draft;
pub mod failing;
pub mod fake;
pub mod ids;
pub mod instances;
pub mod patch;
pub mod people;
pub mod provider;
pub mod read;
pub mod recur;
pub mod sample;
pub mod sample_items;
pub mod sample_people;
pub mod scope;
pub mod slow;
pub mod social;
pub mod state;
pub mod trash;
pub mod write;
pub mod zone;

pub use failing::FailingAppointments;
pub use fake::{DECISIONS, EmptyAppointments, FakeAppointments};
pub use people::FakePeople;
pub use sample::{Sample, sample};
pub use slow::SlowAppointments;
