//! The series tables of the domain `calendar_*`: rules, rule parts, exception fields, `related`, cuts.
//!
//! Pattern: Value Object (const table specifications).

use fs_objects::v5::spec::{
    Col,
    Logical::{Bool, Enum, Int, Local, Ref, Text, Utc},
    Table,
};

pub const FREQUENCY: &[&str] = &[
    "yearly", "monthly", "weekly", "daily", "hourly", "minutely", "secondly",
];
pub const SKIP: &[&str] = &["omit", "backward", "forward"];
pub const WEEKDAY: &[&str] = &["mo", "tu", "we", "th", "fr", "sa", "su"];
pub const PART: &[&str] = &[
    "day",
    "month_day",
    "month",
    "year_day",
    "week_no",
    "hour",
    "minute",
    "second",
    "set_position",
];
pub const RELATION: &[&str] = &["first", "next", "child", "parent"];
pub const OVERRIDE_FIELD: &[&str] = &[
    "title",
    "description",
    "locale",
    "when",
    "status",
    "free_busy",
    "privacy",
    "priority",
    "color",
    "keywords",
    "categories",
    "participants",
    "alerts",
    "use_default_alerts",
];

const RULE_FK: &str =
    "FOREIGN KEY (appointment) REFERENCES calendar_appointment(id) ON DELETE CASCADE";

pub const RULE: Table = Table {
    name: "calendar_rule",
    cols: &[
        Col::req("appointment", Ref),
        Col::req("excluded", Bool),
        Col::req("seq", Int),
        Col::req("frequency", Enum(FREQUENCY)),
        Col::req("interval", Int),
        Col::opt("rscale", Text),
        Col::req("skip", Enum(SKIP)),
        Col::req("first_day_of_week", Enum(WEEKDAY)),
        Col::opt("count", Int),
        Col::opt("until", Local),
    ],
    constraints: &[
        "PRIMARY KEY (appointment, excluded, seq)",
        RULE_FK,
        "CHECK (count IS NULL OR until IS NULL)",
        "CHECK (interval >= 1)",
    ],
    indexes: &[],
};

/// The by-parts of a rule; `pos` keeps the order of a list.
pub const RULE_PART: Table = Table {
    name: "calendar_rule_part",
    cols: &[
        Col::req("appointment", Ref),
        Col::req("excluded", Bool),
        Col::req("seq", Int),
        Col::req("part", Enum(PART)),
        Col::req("pos", Int),
        Col::req("value", Int),
        Col::opt("nth", Int),
        Col::req("leap", Bool),
    ],
    constraints: &[
        "PRIMARY KEY (appointment, excluded, seq, part, pos)",
        "FOREIGN KEY (appointment, excluded, seq) REFERENCES calendar_rule(appointment, excluded, seq) ON DELETE CASCADE",
    ],
    indexes: &[],
};

/// Which fields an exception row sets (the rest is inherited from the master).
pub const OVERRIDE: Table = Table {
    name: "calendar_override_field",
    cols: &[
        Col::req("appointment", Ref),
        Col::req("field", Enum(OVERRIDE_FIELD)),
    ],
    constraints: &["PRIMARY KEY (appointment, field)", RULE_FK],
    indexes: &[],
};

pub const RELATED: Table = Table {
    name: "calendar_related",
    cols: &[
        Col::req("appointment", Ref),
        Col::req("uid", Text),
        Col::req("relation", Enum(RELATION)),
    ],
    constraints: &["PRIMARY KEY (appointment, uid, relation)", RULE_FK],
    indexes: &["CREATE INDEX IF NOT EXISTS calendar_related_uid ON calendar_related (uid)"],
};

/// A partial delete (one occurrence, or this and the following) that `restore(id, scope)` can undo.
pub const CUT: Table = Table {
    name: "calendar_cut",
    cols: &[
        Col::req("appointment", Ref),
        Col::req("recurrence_id", Local),
        Col::req("following", Bool),
        Col::req("until", Utc),
        Col::req("rev_after", Int),
    ],
    constraints: &[
        "PRIMARY KEY (appointment, recurrence_id, following)",
        RULE_FK,
    ],
    indexes: &[],
};

/// The rule ends a cut took away.
pub const CUT_RULE: Table = Table {
    name: "calendar_cut_rule",
    cols: &[
        Col::req("appointment", Ref),
        Col::req("recurrence_id", Local),
        Col::req("following", Bool),
        Col::req("seq", Int),
        Col::opt("end_count", Int),
        Col::opt("end_until", Local),
    ],
    constraints: &[
        "PRIMARY KEY (appointment, recurrence_id, following, seq)",
        "FOREIGN KEY (appointment, recurrence_id, following) REFERENCES calendar_cut(appointment, recurrence_id, following) ON DELETE CASCADE",
    ],
    indexes: &[],
};

/// A part of an imported object that this program cannot interpret. Only the mark is stored; the part itself stays in
/// `object_original` (ADR 0086 point 5), and the export takes it from there.
pub const UNREAD: Table = Table {
    name: "calendar_unread",
    cols: &[
        Col::req("appointment", Ref),
        Col::req("field", Enum(&["recurrence"])),
    ],
    constraints: &["PRIMARY KEY (appointment, field)", RULE_FK],
    indexes: &[],
};
