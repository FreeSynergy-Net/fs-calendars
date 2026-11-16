//! Grenze (ADR 0085 Abschnitt 1, Strang D E15): Formate liefern Anbieter außerhalb des Fach-Traits.
//!
//! Pattern: Adapter (Formate und Personenverzeichnis an der Grenze, hinter Traits); jede Methode ist Pflicht.

use super::{FileRef, MediaType, ObjectId, PersonRef, Problem, Reference, Result, Zone};
use async_trait::async_trait;

/// Fähigkeit eines Formats je Richtung.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Level { Full, Partial { missing: Vec<String> }, No }
/// Was ein Format-Anbieter kann.
#[derive(Clone, Debug)]
pub struct FormatSupport { pub media_type: MediaType, pub import: Level, pub export: Level }
/// Was beim Umwandeln nicht abbildbar war (nie still verloren).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loss { pub uid: Option<String>, pub field: String, pub reason: String }
/// Ergebnis eines Imports.
#[derive(Clone, Debug, Default)]
pub struct ImportReport {
    pub created: Vec<ObjectId>,
    pub skipped: Vec<String>,
    pub failed: Vec<Problem>,
    pub lost: Vec<Loss>,
    /// Zeiten, deren Zone eingesetzt wurde (`zone_assumed`), je `uid`.
    pub zone_assumed: Vec<String>,
    /// Sammelvorschlag Personen (ADR 0058/0085), Id des Vorschlags.
    pub people_proposal: Option<ObjectId>,
    /// Aufgaben aus der Datei, weitergegeben an fs-tasks (ADR 0050).
    pub forwarded_tasks: Vec<String>,
}
/// Ziel eines Exports: Medientyp und, bei Konten, die Quell-IRI. Ziel = Quelle entscheidet über die
/// Rundlauf-Regel (ADR 0085 Abschnitt 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportTarget { pub media_type: MediaType, pub destination: Option<Reference> }
/// Ergebnis eines Exports.
#[derive(Clone, Debug)]
pub struct ExportReport { pub written: FileRef, pub lost: Vec<Loss> }
/// Grundlage der Normalisierung beim Import: Zone für schwebende Zeiten (Einstellung der Person, sonst
/// Gerät, sonst UTC mit Meldung; ADR 0085 "Gerätezone fehlt").
#[derive(Clone, Debug)]
pub struct ImportContext { pub calendar: ObjectId, pub assume_zone: Zone, pub assume_zone_source: ZoneSource }
/// Woher die eingesetzte Zone stammt (wird gemeldet).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoneSource { PersonSetting, Device, LastKnownDevice, UtcFallback }

/// Grenz-Adapter eines Formats (`text/calendar`, `application/jscalendar+json`, später iTIP/iMIP).
/// Liest Fremdformat, normalisiert (Zeiten, Personen über [`PersonDirectory`]) und schreibt über
/// [`super::Appointments`]; Export ist ein eigener Befehl, nie Speichern (ADR 0046). Ein Anbieter ohne eine Richtung
/// sagt es in `formats` (`Level::No`) und antwortet mit [`super::not_possible`].
#[async_trait]
pub trait Exchange: Send + Sync {
    /// Was der Anbieter je Medientyp kann.
    fn formats(&self) -> Vec<FormatSupport>;
    /// `exchange.import`; eine unbekannte oder eigene Zone wird ersetzt (`zone_assumed`, `Loss`), nie geraten.
    async fn import(&self, source: FileRef, as_type: &MediaType, ctx: &ImportContext) -> Result<ImportReport>;
    /// `exchange.export`: Ziel = Quelle und unverändert = Original Byte für Byte; Ziel = Quelle, nur
    /// `zone_assumed`-Zeit unangetastet = wieder schwebend; sonst mit Zone (Rundlauf-Regel ADR 0085).
    async fn export(&self, ids: &[ObjectId], to: &ExportTarget) -> Result<ExportReport>;
}

/// VERBRAUCHT, nicht unser Thema: Personen-Verweis (ADR 0085 Abschnitt 3, Eigentümer fs-people /
/// fs-components-Baustein). Die Form, die fs-calendars braucht; erfüllt von fs-people (`person_directory_v1`) über einen
/// Adapter im Programm.
#[async_trait]
pub trait PersonDirectory: Send + Sync {
    /// Person zu einer Adresse (`mailto:…`), wenn bekannt.
    async fn find_by_address(&self, uri: &str) -> Result<Option<PersonRef>>;
    /// Unbekannte Adresse: Eintrag in der lokalen Ablage im Zustand "vorgeschlagen" (nie freier Name).
    async fn propose(&self, display_hint: &str, uri: &str) -> Result<PersonRef>;
    /// Anzeigename und Adresse für die Oberfläche (aktuell, nicht eingefroren).
    async fn display(&self, person: &PersonRef) -> Result<Option<(String, Option<String>)>>;
}
