//! Zeit und Herkunft eines Termins.
//!
//! Pattern: Value Object (Zeit und Herkunft eines Termins).

use super::*;

/// Länge nach JSCalendar `Duration`: Tage sind nominal (ein "Tag" über die Zeitumstellung bleibt ein Tag).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span { pub days: u32, pub seconds: u32 }

/// Wann ein Termin liegt. Genau eine der drei Formen (ADR 0085 Abschnitt 2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum When {
    /// Mit Uhrzeit: lokale Zeit in `zone`. `end_zone` nur, wenn das Ende in einer anderen Zone liegt
    /// (Flug). `zone_assumed`: Zone wurde an der Grenze für eine schwebende Zeit eingesetzt (ADR 0085).
    Timed { start: LocalDateTime, zone: Zone, length: Span, end_zone: Option<Zone>, zone_assumed: bool },
    /// Überall dieselbe Wanduhrzeit (RFC 8984 1.4.5): keine Zone, nie `zone_assumed`. KEIN Zeitpunkt; wird nie
    /// umgerechnet und in der Zone des Betrachters gezeigt. Entsteht durch Menschen, durch die eigene Marke
    /// oder über den Kanal `Hub`, nie durch eine Zeit ohne Zone allein (Konformitätstest, siehe Dateikopf).
    Floating { start: LocalDateTime, length: Span },
    /// Ganztägig: erster Tag und Zahl der Tage (>= 1). Keine Zone, nie umgerechnet.
    AllDay { first: Date, days: u32 },
}

/// Herkunft eines importierten Objekts (fs-objects `object_origin`), nur lesend. "Unverändert seit Import"
/// wird berechnet: `revision == rev_at_import`. Der Dateiname zählt nie (ADR 0085 Abschnitt 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    pub channel: OriginChannel,
    /// Konten: IRI der Sammlung; Dateien: `urn:sha256:<hex>` der Datei.
    pub source: Reference,
    pub media_type: MediaType,
    pub foreign_uid: String,
    pub foreign_recurrence_id: Option<LocalDateTime>,
    pub foreign_etag: Option<String>,
    pub prod_id: Option<String>,
    pub imported: Utc,
    /// Name und Version des Importers.
    pub importer: String,
    pub rev_at_import: Revision,
    /// Das Original (Ort und Prüfsumme), wenn aufbewahrt; Grundlage für "Byte für Byte" im Rundlauf
    /// (ADR 0085 Abschnitt 1, ADR 0086 Punkt 4).
    pub original: Option<FileRef>,
}
/// Kanal, über den ein Objekt hereinkam.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OriginChannel {
    File, CalDav, Google, Outlook, Itip, Hub,
    /// Abo per URL (`webcal:`/`https:`-Feed): `Origin.source` ist die Feed-IRI, nicht der Dateihash, damit
    /// wiederholte Abrufe dieselbe Quelle haben (Dubletten, Rundlauf). JMAP-Kalender (RFC 8620) gibt es
    /// bewusst nicht in v1.
    Subscription,
}
