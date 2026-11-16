//! Abfragen und Antworten, Protokoll.

use super::*;


/// Ein Vorkommen im Zeitfenster (für Raster und Agenda). Zeiten schon für Sortieren/Lage in UTC.
#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    pub appointment: ObjectId,
    pub calendar: ObjectId,
    pub recurrence_id: Option<LocalDateTime>,
    pub title: String,
    pub when: When,
    pub start: Utc,
    pub end: Utc,
    pub status: Status,
    pub free_busy: FreeBusy,
    pub privacy: Privacy,
    pub overridden: bool,
    pub recurring: bool,
    pub participant_count: u32,
    /// Teilnahmestatus der fragenden Person, wenn sie Teilnehmer ist.
    pub my_status: Option<ParticipationStatus>,
}

/// Was im Fenster nicht beantwortet werden konnte (kostet nur sich selbst).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Damaged { pub appointment: ObjectId, pub recurrence_id: Option<LocalDateTime>, pub reason: String, pub cut: bool }

/// Antwort auf ein Zeitfenster.
#[derive(Clone, Debug, Default)]
pub struct Window { pub instances: Vec<Instance>, pub damaged: Vec<Damaged> }

/// Kurzform für Listen und Suche. `restorable_until`: `None` = nicht gelöscht, `Some` = im Papierkorb bis dahin
/// (nur mit `include_deleted` sichtbar).
#[derive(Clone, Debug, PartialEq)]
pub struct AppointmentSummary { pub id: ObjectId, pub calendar: ObjectId, pub uid: String, pub title: String, pub when: When, pub recurring: bool, pub revision: Revision, pub restorable_until: Option<Utc> }

/// Filter für `list`.
#[derive(Clone, Debug, Default)]
pub struct AppointmentQuery { pub calendars: Vec<ObjectId>, pub text: Option<String>, pub from: Option<Utc>, pub to: Option<Utc>, pub participant: Option<Attendee>, pub include_deleted: bool }

/// Wen einladen (Menschen, Räume, Ressourcen).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invitee { pub who: Attendee, pub roles: Roles, pub expect_reply: bool }

/// Ergebnis eines Löschens (Papierkorb; `restore` holt zurück, bis die Frist endet).
#[derive(Clone, Debug, PartialEq)]
pub struct Deleted { pub id: ObjectId, pub scope: SeriesScope, pub restorable_until: Option<Utc> }

/// Belegte Zeit eines Teilnehmers (Person, Raum, Ressource; frei/belegt, Dossier A5 "Verfügbarkeit").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Busy { pub who: Attendee, pub start: Utc, pub end: Utc, pub tentative: bool }

/// Ein typisierter Eintrag im Aktivitätsprotokoll (ADR 0086 Punkt 2).
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryEntry { pub at: Utc, pub actor: PersonRef, pub kind: HistoryKind, pub changes: Vec<FieldChange> }
/// Art der Aktivität (AS2-Name).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryKind { Create, Update, Delete, Undo, Invite, Accept, TentativeAccept, Reject }
/// Feld, alter Wert, neuer Wert.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldChange { pub field: String, pub row_key: Option<String>, pub old: Option<Value>, pub new: Option<Value> }
/// Typisierter Wert im Protokoll. `Object` = Objekt-Id im eigenen Speicher, `Reference` = fremder oder
/// programmübergreifender Verweis. Aufzählungswerte (`when_kind`, `status` …) stehen als `Text` mit dem
/// kanonischen JSCalendar- bzw. AS2-Wort.
#[derive(Clone, Debug, PartialEq)]
pub enum Value { Int(i64), Real(f64), Text(String), Bool(bool), Utc(Utc), Local(LocalDateTime), Date(Date), Zone(Zone), Object(ObjectId), Reference(Reference), Person(PersonRef), Attendee(Attendee) }

/// Änderung für Beobachter (Raster neu laden).
#[derive(Clone, Debug, PartialEq)]
pub struct Change { pub appointment: ObjectId, pub calendar: ObjectId, pub kind: HistoryKind, pub at: Utc }
