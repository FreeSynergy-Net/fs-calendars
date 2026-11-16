//! The calendar's `Domain` for the shared layer (R1): prefix, tables, steps, dependent objects, deadlines, channels.
//!
//! Pattern: Registry (a program reports its table family once; the layer creates the tables, writes them down and
//! runs the steps that are missing). The calendar has one step, `Create`; a later change is a new step appended, with
//! a fixture test (ADR 0086).

use fs_objects::v5::Step;
use fs_objects::v5::envelope::Child;
use fs_objects::v5::purge::Expiry;
use fs_objects::v5::registry::Domain;

use crate::calendar_tables::{ALL, PREFIX};
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

/// The exceptions of a series are objects of their own: their envelopes go with the master at the final delete.
const CHILDREN: &[Child] = &[Child {
    table: "calendar_appointment",
    parent: "master",
    object: "id",
}];

/// The cuts that a restore period ended (ADR 0073).
const EXPIRIES: &[Expiry] = &[Expiry {
    table: "calendar_cut",
    until: "until",
    key: &["appointment", "recurrence_id", "following"],
    on_expired: Some(on_cut),
}];

/// The domain of the calendar.
pub const DOMAIN: Domain = Domain {
    prefix: PREFIX,
    about: "calendar: collections, appointments, participants, alerts",
    tables: &ALL,
    channels: CHANNELS,
    children: CHILDREN,
    expiries: EXPIRIES,
    steps: &[Step::Create(&ALL)],
};
