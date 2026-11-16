//! `FakePeople`: a `PersonDirectory` with a few fixed people (the sample's) and a "proposed" list.
//!
//! Pattern: Fake behind the consumed `PersonDirectory` trait. Names live here, never in an appointment.

use std::sync::Mutex;

use async_trait::async_trait;

use crate::draft::{Error, PersonArt, PersonDirectory, PersonRef, Reference, Result};
use crate::ids::{person, person_iri};

/// One known person.
#[derive(Clone, Debug)]
pub struct Known {
    /// The reference appointments hold.
    pub person: PersonRef,
    /// Display name (the only place the name exists).
    pub name: String,
    /// Address as `mailto:...`.
    pub address: String,
}

/// The directory.
#[derive(Debug)]
pub struct FakePeople {
    known: Mutex<Vec<Known>>,
    proposed: Mutex<Vec<Known>>,
    next: Mutex<u64>,
}

impl FakePeople {
    /// A directory with exactly these people.
    #[must_use]
    pub fn new(known: Vec<Known>) -> Self {
        Self {
            known: Mutex::new(known),
            proposed: Mutex::new(Vec::new()),
            next: Mutex::new(900),
        }
    }

    /// Entries made by `propose` (state "proposed", nobody confirmed them).
    #[must_use]
    pub fn proposed(&self) -> Vec<Known> {
        self.proposed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Make a known person (helper for the sample).
    #[must_use]
    pub fn known(n: u64, name: &str, address: &str) -> Known {
        Known {
            person: person(n),
            name: name.to_owned(),
            address: address.to_owned(),
        }
    }
}

fn all(m: &Mutex<Vec<Known>>) -> Vec<Known> {
    m.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

#[async_trait]
impl PersonDirectory for FakePeople {
    async fn find_by_address(&self, uri: &str) -> Result<Option<PersonRef>> {
        let hit = all(&self.known)
            .into_iter()
            .chain(all(&self.proposed))
            .find(|k| k.address.eq_ignore_ascii_case(uri));
        Ok(hit.map(|k| k.person))
    }

    async fn propose(&self, display_hint: &str, uri: &str) -> Result<PersonRef> {
        if let Some(found) = self.find_by_address(uri).await? {
            return Ok(found);
        }
        if display_hint.trim().is_empty() && uri.trim().is_empty() {
            return Err(Error::Invalid(Vec::new()));
        }
        let n = {
            let mut next = self
                .next
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *next += 1;
            *next
        };
        let p = PersonRef {
            id: Reference(person_iri(n).0),
            art: PersonArt::Profile,
            abzug: None,
        };
        self.proposed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(Known {
                person: p.clone(),
                name: display_hint.to_owned(),
                address: uri.to_owned(),
            });
        Ok(p)
    }

    async fn display(&self, p: &PersonRef) -> Result<Option<(String, Option<String>)>> {
        let hit = all(&self.known)
            .into_iter()
            .chain(all(&self.proposed))
            .find(|k| k.person.id == p.id);
        Ok(hit.map(|k| (k.name, Some(k.address))))
    }
}
