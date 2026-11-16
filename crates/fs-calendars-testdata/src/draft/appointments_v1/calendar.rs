//! Kalender (Sammlung von Terminen): eigene, abonnierte und Konto-Kalender, ändern, Papierkorb.
//!
//! Pattern: Value Object (Kalender, Quelle, Abo-Zustand und Papierkorb-Frist als reine Daten).

use super::{ImportReport, ObjectId, PersonRef, Reference, Revision, Utc, Zone};

/// Ein Kalender. Sichtbarkeit und Farbe in der Seitenleiste sind Einstellung der Person, nicht Daten; `color` hier
/// ist die Vorgabe des Kalenders. `owner`: Person, Gruppe oder Organisation (`PersonArt::Person`/`Group`/
/// `Organization`; `Profile` und KI-Konten nie, Frage 22: JA); WAS jemand darf, sagt fs-access (Rechte-Teil-Trait),
/// nicht dieses Feld. `trash`: `None` = nicht gelöscht, `Some` = im Papierkorb (nur mit `Trash::Include`/`Only`).
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
    /// Abo eines fremden Kalenders per Link (Frage 25.2: JA, aktiv). SCHREIBGESCHÜTZT: Termine darin schreibt nur der
    /// Abruf (`refresh_calendar`); `create`/`update`/`delete`/`restore`/`invite`/`respond` ergeben `Error::ReadOnly`
    /// (`purge` eines Abo-Termins im Papierkorb ist erlaubt). Jeder Termin trägt `Origin { channel: Subscription,
    /// source: <IRI dieses Kalenders> }`, NIE die Feed-Adresse (sie kann geheim sein, siehe [`Subscription`]).
    Subscribed(Subscription),
    /// Sammlung eines Kontos (`CalDAV`, RFC 4791; Frage 23). `collection`: IRI der Sammlung, `state`: letzter Abgleich.
    /// Der Abgleich (Sync-Token, `ETag` je Termin in `Origin.foreign_etag`) ist Technik des Anbieters, kein Vertrag.
    Account { collection: Reference, state: FetchState },
}

/// Ein Abo: Quelle, Abrufregel, letzter Abruf und Fehlerzustand.
///
/// `url`: nur `https:` und `webcal:` (= https); jedes andere Schema, ein Ziel in einem privaten, Link-lokalen oder
/// Loopback-Netz (auch über DNS oder eine Umleitung dorthin) ist `Invalid` am Feld `url` (Schutz vor SSRF), beim
/// Anlegen wie bei jedem Abruf. Größen- und Zeitgrenzen des Abrufs setzt das Programm. Die Adresse kann ein Geheimnis
/// tragen ("geheime Adresse"): sichtbar nur für Personen mit `calendar.update` an diesem Kalender (sonst `None`), nie im
/// Verlauf als Klartext (dort nur "geändert"), nie in `Origin.source` oder einem Export.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subscription { pub url: Option<Reference>, pub refresh: Refresh, pub state: FetchState }

/// Wann ein Abo abgerufen wird. Abrufe gehen nur an die eine Adresse des Abos, nie anderswohin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refresh {
    /// Nur auf Klick (`refresh_calendar`). Vorgabe.
    Manual,
    /// Zusätzlich regelmäßig alle `seconds` Sekunden (der Anbieter darf eine Mindestzeit setzen, dann `Invalid`).
    Every { seconds: u32 },
}

/// Zustand des letzten Abrufs bzw. Abgleichs. Ein Fehler löscht nichts: die Termine des letzten guten Abrufs bleiben.
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
/// `owner` = Übergabe an eine andere Person oder Gruppe (Recht aus fs-access; nie an eine KI, `Invalid`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct CalendarPatch {
    pub name: Option<String>,
    pub color: Option<Option<String>>,
    pub default_zone: Option<Option<Zone>>,
    pub owner: Option<PersonRef>,
    pub url: Option<Reference>,
    pub refresh: Option<Refresh>,
}

/// Bis wann Gelöschtes zurückgeholt werden kann (Papierkorb, ADR 0073 Nachtrag 2026-11-16). Die Frist ist eine
/// Einstellung des Programms (Pflicht; Standard 30 Tage, mindestens 1 Tag, "nie automatisch löschen" erlaubt), nicht
/// des Traits. Die Zusage gilt: nichts geht früher weg. Nach Ablauf geht das Objekt samt Verlauf endgültig (Kaskade).
/// Vorrang: eine Aufbewahrung (`keep_until`, fs-objects `object_hold`) geht vor Frist UND vor "nie automatisch":
/// solange sie gilt, ist es `Held`; danach `Until` bzw. `Manual`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Restorable {
    /// Keine Aufbewahrung; die Frist läuft bis dahin ("noch N Tage").
    Until(Utc),
    /// Aufbewahrt ("aufbewahrt bis …"): `Some(t)` = bis zum SPÄTEREN von Frist und Aufbewahrungsende (so liefert es
    /// fs-objects `restorable_until`); `None` = Aufbewahrung ohne Ende. `purge` ist dann `Forbidden`.
    Held(Option<Utc>),
    /// "Nie automatisch löschen" ist eingestellt, keine Aufbewahrung: bleibt, bis jemand `purge` aufruft.
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
