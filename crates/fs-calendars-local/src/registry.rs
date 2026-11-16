//! The calendar's `Domain` for the shared layer (R1): prefix, tables, steps, dependent objects, deadlines, channels.
//!
//! Pattern: Registry (a program reports its table family once; the layer creates the tables, writes them down and
//! runs the steps that are missing). The calendar has two steps, both `Create`; a later change is a new step appended, with
//! a fixture test (ADR 0086: `tests/migration_step2.rs`).
//!
//! Known limit of step 2: a store of Stand 1 gets no `calendar_rule_original` row for a rule that Stand 1 only marked
//! as unread (`calendar_unread`): such a series reads as without a rule here and its rule cannot be edited (the mark
//! keeps the refusal); the export is not affected, it comes from `object_original`. No migration fills the rows:
//! Stand 1 never left the working branch, there is no file in use that has them.

use fs_objects::v5::Step;
use fs_objects::v5::envelope::Child;
use fs_objects::v5::purge::Expiry;
use fs_objects::v5::registry::Domain;

use crate::calendar_tables::{ALL, PREFIX, STEP1, STEP2};
use crate::sweep::on_cut;

/// The channels `object_origin.channel` may hold in the calendar.
pub const CHANNELS: &[&str] = &[
    "file",
    "caldav",
    "google",
    "outlook",
    "itip",
    "hub",
    "subscription",
];

/// Dependent objects whose envelopes go with their parent at the final delete: the exceptions of a series (objects of
/// their own) and the appointments of a collection (`calendar_appointment.collection` is a foreign key without
/// `ON DELETE CASCADE`: a collection that ended its trash period with appointments would otherwise block the whole
/// period run). The layer follows them recursively and deletes the dependents first.
const CHILDREN: &[Child] = &[
    Child {
        table: "calendar_appointment",
        parent: "master",
        object: "id",
    },
    Child {
        table: "calendar_appointment",
        parent: "collection",
        object: "id",
    },
];

/// The cuts that a restore period ended (ADR 0073).
const EXPIRIES: &[Expiry] = &[Expiry {
    table: "calendar_cut",
    until: "until",
    key: &["appointment", "recurrence_id", "following"],
    on_expired: Some(on_cut),
}];

/// The domain of the calendar. Step 2 adds the original text of a recurrence rule (`Recurrence::Original`) and the cascade of a deleted calendar.
pub const DOMAIN: Domain = Domain::new(
    PREFIX,
    "calendar: collections, appointments, participants, alerts",
    &ALL,
    &[Step::Create(&STEP1), Step::Create(&STEP2)],
)
.with_channels(CHANNELS)
.with_children(CHILDREN)
.with_expiries(EXPIRIES);
