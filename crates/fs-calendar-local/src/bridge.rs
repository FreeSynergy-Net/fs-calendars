//! The border between the locked trait (its stand-in types) and the shared layer (`fs-types`): ids, revisions,
//! persons, kinds and the error. The values of the log are in `bridge_values`.
//!
//! Pattern: Adapter (one function per type and direction; no state). The layer is authoritative: the error mapping
//! follows it, the calendar adapts. When the trait moves to `fs-types` this file disappears.

use fs_calendar_testdata::draft as si;
use fs_calendar_testdata::draft::BasicValue as _;
use fs_objects::v5;
use fs_objects::v5::log::Kind;
use fs_types as ft;
use ft::BasicValue as _;

/// An id of the trait as the layer's id.
#[must_use]
pub fn oid(v: si::ObjectId) -> ft::ObjectId {
    ft::ObjectId(v.0)
}

/// An id of the layer as the trait's id.
#[must_use]
pub fn oid_back(v: ft::ObjectId) -> si::ObjectId {
    si::ObjectId(v.0)
}

/// A revision of the trait as the layer's.
#[must_use]
pub fn rev(v: si::Revision) -> ft::Revision {
    ft::Revision(v.0)
}

/// A revision of the layer as the trait's.
#[must_use]
pub fn rev_back(v: ft::Revision) -> si::Revision {
    si::Revision(v.0)
}

fn bad(field: &str, why: &str) -> si::Error {
    si::Error::Invalid(vec![si::Problem {
        field: field.to_owned(),
        reason: why.to_owned(),
    }])
}

/// A reference of the trait as the layer's.
///
/// # Errors
/// `Invalid` for a value `fs-types` refuses.
pub fn reference(v: &si::Reference) -> si::Result<ft::Reference> {
    ft::Reference::parse(&v.canonical()).map_err(|_| bad("reference", "not an absolute IRI"))
}

/// A reference of the layer as the trait's.
///
/// # Errors
/// `Invalid` for a value the stand-in refuses.
pub fn reference_back(v: &ft::Reference) -> si::Result<si::Reference> {
    si::Reference::parse(&v.canonical()).map_err(|_| bad("reference", "not an absolute IRI"))
}

fn art(v: si::PersonArt) -> ft::PersonArt {
    match v {
        si::PersonArt::Profile => ft::PersonArt::Profile,
        si::PersonArt::Person => ft::PersonArt::Person,
        si::PersonArt::Group => ft::PersonArt::Group,
        si::PersonArt::Organization => ft::PersonArt::Organization,
    }
}

fn art_back(v: ft::PersonArt) -> si::PersonArt {
    match v {
        ft::PersonArt::Profile => si::PersonArt::Profile,
        ft::PersonArt::Person => si::PersonArt::Person,
        ft::PersonArt::Group => si::PersonArt::Group,
        ft::PersonArt::Organization => si::PersonArt::Organization,
    }
}

/// A person of the trait as the layer's.
///
/// # Errors
/// `Invalid` for a reference `fs-types` refuses.
pub fn person(v: &si::PersonRef) -> si::Result<ft::PersonRef> {
    Ok(ft::PersonRef {
        id: reference(&v.id)?,
        art: art(v.art),
        abzug: v.abzug.map(oid),
    })
}

/// A person of the layer as the trait's.
///
/// # Errors
/// `Invalid` for a reference the stand-in refuses.
pub fn person_back(v: &ft::PersonRef) -> si::Result<si::PersonRef> {
    Ok(si::PersonRef {
        id: reference_back(&v.id)?,
        art: art_back(v.art),
        abzug: v.abzug.map(oid_back),
    })
}

/// The kind of an activity of the trait as the layer's.
#[must_use]
pub fn kind(k: si::HistoryKind) -> Kind {
    match k {
        si::HistoryKind::Create => Kind::Create,
        si::HistoryKind::Update => Kind::Update,
        si::HistoryKind::Delete => Kind::Delete,
        si::HistoryKind::Undo => Kind::Undo,
        si::HistoryKind::Invite => Kind::Invite,
        si::HistoryKind::Accept => Kind::Accept,
        si::HistoryKind::TentativeAccept => Kind::TentativeAccept,
        si::HistoryKind::Reject => Kind::Reject,
    }
}

/// The kind of an activity of the layer as the trait's.
#[must_use]
pub fn kind_back(k: Kind) -> si::HistoryKind {
    match k {
        Kind::Create => si::HistoryKind::Create,
        Kind::Update => si::HistoryKind::Update,
        Kind::Delete => si::HistoryKind::Delete,
        Kind::Undo => si::HistoryKind::Undo,
        Kind::Invite => si::HistoryKind::Invite,
        Kind::Accept => si::HistoryKind::Accept,
        Kind::TentativeAccept => si::HistoryKind::TentativeAccept,
        Kind::Reject => si::HistoryKind::Reject,
    }
}

/// The error of the layer as the trait's: the layer is authoritative, what the trait has no word for is `Unavailable`.
#[must_use]
pub fn adapt(e: v5::Error) -> si::Error {
    match e {
        v5::Error::NotFound(id) => si::Error::NotFound(oid_back(id)),
        v5::Error::Invalid(ps) => si::Error::Invalid(
            ps.into_iter()
                .map(|p| si::Problem {
                    field: p.field,
                    reason: p.reason,
                })
                .collect(),
        ),
        v5::Error::Changed { current } => si::Error::Changed {
            current: rev_back(current),
        },
        other => si::Error::Unavailable(other.to_string()),
    }
}

/// The error of the trait as the layer's (for a hook that runs inside the layer's period run).
#[must_use]
pub fn to_v5(e: si::Error) -> v5::Error {
    match e {
        si::Error::NotFound(id) => v5::Error::NotFound(oid(id)),
        si::Error::Invalid(ps) => v5::Error::Invalid(
            ps.into_iter()
                .map(|p| ft::Problem {
                    field: p.field,
                    reason: p.reason,
                })
                .collect(),
        ),
        si::Error::Changed { current } => v5::Error::Changed {
            current: rev(current),
        },
        si::Error::UnknownZone(z) => v5::Error::Invalid(vec![ft::Problem {
            field: "zone".to_owned(),
            reason: format!("unknown zone {z}"),
        }]),
        other => v5::Error::Unavailable(format!("{other:?}")),
    }
}
