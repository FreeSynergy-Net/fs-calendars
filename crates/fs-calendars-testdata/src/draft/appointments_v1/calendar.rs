//! Kalender (Sammlung von Terminen): eigene und abonnierte, ändern, Papierkorb.
//!
//! Pattern: Value Object (Kalender, Quelle, Abo-Zustand und Papierkorb-Frist als reine Daten).

use super::{ImportReport, ObjectId, PersonRef, Reference, Revision, Utc, Zone};

/// Ein Kalender. Sichtbarkeit und Farbe in der Seitenleiste sind Einstellung der Person, nicht Daten; `color` hier
/// ist die Vorgabe des Kalenders. `owner`: die Person, die ihn angelegt hat; ob auch eine Gruppe Eigentümerin sein
/// kann (`PersonArt::Group`), ist offen (Frage 22, siehe `mod.rs`). `trash`: `None` = nicht gelöscht, `Some` = im
/// Papierkorb (nur mit `calendars(true)` sichtbar).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarInfo {
    pub id: ObjectId,
    pub name: String,
    pub color: Option<String>,
    pub default_zone: Option<Zone>,
    pub owner: PersonRef,
    pub source: CalendarSource,
    pub trash: Option<Restorable>,
    pub revision: Revision,
}

/// Woher die Termine eines Kalenders kommen.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CalendarSource {
    /// Eigener Kalender: Termine entstehen hier (`create`, `Exchange::import`).
    Own,
    /// Abo eines fremden Kalenders per URL (`webcal:`/`https:`, `text/calendar`). SCHREIBGESCHÜTZT: Termine darin
    /// schreibt nur der Abruf (`refresh_calendar`); `create`/`update`/`delete`/`invite`/`respond` ergeben
    /// `Error::ReadOnly`. Jeder Termin trägt `Origin { channel: Subscription, source: <url> }`. Ob das Programm
    /// fremde Kalender abonnieren darf, ist offen (Frage 25, siehe `mod.rs`); bis dahin `calendar.subscribe = false`.
    Subscribed(Subscription),
}

/// Ein Abo: Quelle, Abrufregel, letzter Abruf und Fehlerzustand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subscription { pub url: Reference, pub refresh: Refresh, pub state: FetchState }

/// Wann ein Abo abgerufen wird. Abrufe gehen nur an die eine Adresse des Abos, nie anderswohin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refresh {
    /// Nur auf Klick (`refresh_calendar`). Vorgabe.
    Manual,
    /// Zusätzlich regelmäßig alle `seconds` Sekunden (der Anbieter darf eine Mindestzeit setzen, dann `Invalid`).
    Every { seconds: u32 },
}

/// Zustand des letzten Abrufs. Ein Fehler löscht nichts: die Termine des letzten guten Abrufs bleiben sichtbar.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FetchState {
    /// Noch nie abgerufen.
    Never,
    /// Letzter Abruf gelungen.
    Fresh { at: Utc },
    /// Letzter Abruf gescheitert (`reason` = Code, kein Satz); `attempts` gescheiterte Abrufe in Folge.
    Failed { at: Utc, reason: String, last_success: Option<Utc>, attempts: u32 },
}

/// Änderung eines Kalenders: `Some` = setzen, `None` = bleibt. `url`/`refresh` nur bei einem Abo, sonst `Invalid`.
/// Den Eigentümer ändert diese Form nicht (Frage 22).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct CalendarPatch {
    pub name: Option<String>,
    pub color: Option<Option<String>>,
    pub default_zone: Option<Option<Zone>>,
    pub url: Option<Reference>,
    pub refresh: Option<Refresh>,
}

/// Bis wann Gelöschtes zurückgeholt werden kann (Papierkorb, ADR 0073 Nachtrag 2026-11-16). Die Frist ist eine
/// Einstellung des Programms (Pflicht; Standard 30 Tage, mindestens 1 Tag), nicht des Traits. Die Zusage gilt:
/// nichts geht früher weg. Nach Ablauf geht das Objekt samt Verlauf endgültig (Kaskade).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Restorable {
    /// Frist läuft bis dahin ("noch N Tage").
    Until(Utc),
    /// Eine Aufbewahrung (`keep_until`) geht vor: bis dahin ("aufbewahrt bis …"); `None` = ohne Ende.
    Held(Option<Utc>),
    /// "Nie automatisch löschen" ist eingestellt: bleibt, bis jemand endgültig löscht (`purge`).
    Manual,
}

/// Ergebnis von `delete_calendar`: der Kalender und `appointments` Termine darin liegen im Papierkorb, alle mit
/// derselben Frist; ihr Verlauf geht mit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CalendarDeleted { pub id: ObjectId, pub restorable: Restorable, pub appointments: u32 }

/// Ergebnis eines Abrufs (`refresh_calendar`): neuer Zustand des Kalenders (auch bei Fehler: `FetchState::Failed`),
/// was neu kam (`report`, Verluste je Feld wie beim Import), was sich geändert hat (`updated`) und welche Termine die
/// Quelle nicht mehr hat (`removed`; sie gehen in den Papierkorb, nicht endgültig).
#[derive(Clone, Debug)]
pub struct Refreshed { pub calendar: CalendarInfo, pub report: ImportReport, pub updated: Vec<ObjectId>, pub removed: Vec<ObjectId> }
