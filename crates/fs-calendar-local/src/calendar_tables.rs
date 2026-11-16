//! The domain tables `calendar_*` of this slice (plan section 2.1 with the draft's `WHEN` composite, section 2.3).
//!
//! Pattern: Value Object (const table specifications). Not stored yet: channels, delegations, `reply_to`, locations,
//! virtual locations, links, localizations, custom zones, alert fire times.

use crate::object_tables::{ART, WHO_KINDS};
use crate::spec::{
    Col,
    Logical::{Bool, Date, Enum, Int, Iri, Local, Ref, Text, Utc, Zone},
    Table,
};

pub const PREFIX: &str = "calendar";
pub const VERSION: i64 = 4;
pub const STATUS: &[&str] = &["confirmed", "tentative", "cancelled"];
pub const FREE_BUSY: &[&str] = &["busy", "free"];
pub const PRIVACY: &[&str] = &["public", "private", "secret"];
pub const TEXT_TYPE: &[&str] = &["plain", "html"];
pub const WHEN_KIND: &[&str] = &["timed", "floating", "all_day"];
pub const PARTICIPATION: &[&str] = &[
    "needs-action",
    "accepted",
    "declined",
    "tentative",
    "delegated",
];
pub const TRIGGER: &[&str] = &["offset", "absolute"];
pub const RELATIVE_TO: &[&str] = &["start", "end"];
pub const ALERT_ACTION: &[&str] = &["display", "email"];
pub const COLLECTION: Table = Table {
    name: "calendar_collection",
    cols: &[
        Col::req("id", Ref),
        Col::req("name", Text),
        Col::opt("color", Text),
        Col::opt("default_zone", Zone),
        Col::req("owner_id", Iri),
        Col::req("owner_art", Enum(ART)),
        Col::opt("owner_abzug", Ref),
    ],
    constraints: &[
        "PRIMARY KEY (id)",
        "FOREIGN KEY (id) REFERENCES objects(id) ON DELETE CASCADE",
    ],
    indexes: &[
        "CREATE INDEX IF NOT EXISTS calendar_collection_owner ON calendar_collection (owner_id)",
    ],
};

/// One appointment; `when_*` is the draft's `WHEN` composite. `when_utc_min`/`max` are the hull (derived).
pub const APPOINTMENT: Table = Table {
    name: "calendar_appointment",
    cols: &[
        Col::req("id", Ref),
        Col::req("collection", Ref),
        Col::req("uid", Text),
        Col::opt("master", Ref),
        Col::opt("recurrence_id", Local),
        Col::req("excluded", Bool),
        Col::req("sequence", Int),
        Col::req("created", Utc),
        Col::req("updated", Utc),
        Col::req("title", Text),
        Col::req("description", Text),
        Col::req("description_type", Enum(TEXT_TYPE)),
        Col::opt("locale", Text),
        Col::req("when_kind", Enum(WHEN_KIND)),
        Col::opt("when_local", Local),
        Col::opt("when_date", Date),
        Col::opt("when_zone", Zone),
        Col::opt("when_end_zone", Zone),
        Col::req("when_zone_assumed", Bool),
        Col::req("when_len_days", Int),
        Col::req("when_len_seconds", Int),
        Col::derived("when_utc_min", Utc, false),
        Col::derived("when_utc_max", Utc, false),
        Col::req("status", Enum(STATUS)),
        Col::req("free_busy", Enum(FREE_BUSY)),
        Col::req("privacy", Enum(PRIVACY)),
        Col::req("priority", Int),
        Col::opt("color", Text),
        Col::req("use_default_alerts", Bool),
        Col::req("hide_attendees", Bool),
        Col::req("may_invite_self", Bool),
        Col::req("may_invite_others", Bool),
        Col::derived("series_end_utc", Utc, true),
        Col::derived("search_text", Text, false),
        Col::opt("hidden_by", Local),
    ],
    constraints: &[
        "PRIMARY KEY (id)",
        "FOREIGN KEY (id) REFERENCES objects(id) ON DELETE CASCADE",
        "FOREIGN KEY (collection) REFERENCES calendar_collection(id)",
        "FOREIGN KEY (master) REFERENCES calendar_appointment(id) ON DELETE CASCADE",
        "CHECK (when_kind = 'timed' AND when_local IS NOT NULL AND when_date IS NULL AND when_zone IS NOT NULL \
         OR when_kind = 'floating' AND when_local IS NOT NULL AND when_date IS NULL AND when_zone IS NULL \
         AND when_zone_assumed = 0 AND when_end_zone IS NULL \
         OR when_kind = 'all_day' AND when_local IS NULL AND when_date IS NOT NULL AND when_zone IS NULL \
         AND when_end_zone IS NULL AND when_zone_assumed = 0 AND when_len_seconds = 0 AND when_len_days >= 1)",
    ],
    indexes: &[
        "CREATE UNIQUE INDEX IF NOT EXISTS calendar_appointment_uid ON calendar_appointment (uid) WHERE master IS NULL",
        "CREATE UNIQUE INDEX IF NOT EXISTS calendar_appointment_exception ON calendar_appointment (master, recurrence_id) WHERE master IS NOT NULL",
        "CREATE INDEX IF NOT EXISTS calendar_appointment_start ON calendar_appointment (collection, when_utc_min)",
        "CREATE INDEX IF NOT EXISTS calendar_appointment_hull ON calendar_appointment (when_utc_min, series_end_utc)",
    ],
};

pub const PARTICIPANT: Table = Table {
    name: "calendar_participant",
    cols: &[
        Col::req("appointment", Ref),
        Col::req("key", Text),
        Col::req("who_kind", Enum(WHO_KINDS)),
        Col::req("who_id", Iri),
        Col::opt("who_art", Enum(ART)),
        Col::opt("who_abzug", Ref),
        Col::req("role_owner", Bool),
        Col::req("role_attendee", Bool),
        Col::req("role_optional", Bool),
        Col::req("role_informational", Bool),
        Col::req("role_chair", Bool),
        Col::req("role_contact", Bool),
        Col::req("status", Enum(PARTICIPATION)),
        Col::req("expect_reply", Bool),
        Col::opt("comment", Text),
        Col::opt("invited_by", Text),
        Col::req("schedule_sequence", Int),
        Col::opt("schedule_updated", Utc),
    ],
    constraints: &[
        "PRIMARY KEY (appointment, key)",
        "FOREIGN KEY (appointment) REFERENCES calendar_appointment(id) ON DELETE CASCADE",
        "CHECK ((who_kind = 'actor') = (who_art IS NOT NULL))",
    ],
    indexes: &[
        "CREATE INDEX IF NOT EXISTS calendar_participant_who ON calendar_participant (who_id)",
    ],
};

pub const ALERT: Table = Table {
    name: "calendar_alert",
    cols: &[
        Col::req("appointment", Ref),
        Col::req("key", Text),
        Col::req("trigger_kind", Enum(TRIGGER)),
        Col::opt("offset_seconds", Int),
        Col::opt("relative_to", Enum(RELATIVE_TO)),
        Col::opt("at", Utc),
        Col::req("action", Enum(ALERT_ACTION)),
        Col::opt("acknowledged", Utc),
    ],
    constraints: &[
        "PRIMARY KEY (appointment, key)",
        "FOREIGN KEY (appointment) REFERENCES calendar_appointment(id) ON DELETE CASCADE",
        "CHECK (trigger_kind = 'offset' AND offset_seconds IS NOT NULL AND relative_to IS NOT NULL AND at IS NULL \
         OR trigger_kind = 'absolute' AND at IS NOT NULL AND offset_seconds IS NULL AND relative_to IS NULL)",
    ],
    indexes: &[],
};

pub const KEYWORD: Table = value_table("calendar_keyword");
pub const CATEGORY: Table = value_table("calendar_category");

const VALUE_COLS: &[Col] = &[Col::req("appointment", Ref), Col::req("value", Text)];
const VALUE_KEY: &[&str] = &[
    "PRIMARY KEY (appointment, value)",
    "FOREIGN KEY (appointment) REFERENCES calendar_appointment(id) ON DELETE CASCADE",
];

const fn value_table(name: &'static str) -> Table {
    Table {
        name,
        cols: VALUE_COLS,
        constraints: VALUE_KEY,
        indexes: &[],
    }
}

pub use crate::series_tables::{CUT, CUT_RULE, OVERRIDE, RELATED, RULE, RULE_PART, UNREAD};

/// All tables of the domain, parents first.
pub const ALL: [Table; 13] = [
    COLLECTION,
    APPOINTMENT,
    PARTICIPANT,
    ALERT,
    KEYWORD,
    CATEGORY,
    RULE,
    RULE_PART,
    OVERRIDE,
    RELATED,
    CUT,
    CUT_RULE,
    UNREAD,
];
