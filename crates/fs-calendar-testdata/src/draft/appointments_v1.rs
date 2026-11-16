//! Thema `events`, Ziel v1 (Sprung von "events 0.1 (draft)"). Eigentümer: fs-calendar (ADR 0050).
//!
//! FESTER Trait des Piloten `programm-grundgeruest` (Stufe 2). Nach `strategie-ok` wird diese Datei per
//! SHA-256 gesperrt; jede Änderung ist ein Sprung (ADR 0045) und ein neuer Plan. Stufe 3 kopiert sie
//! byte-gleich nach `fs-calendar-core/src/interface/appointments.rs`.
//!
//! Pattern: Repository (interface) + Null Object (ADR 0043/0045) - jeder Anbieter hat jede Methode; was er
//! nicht kann, antwortet `Unsupported` (Standardmethode). Formate (ICS, JSCalendar-JSON, iTIP) stehen NICHT
//! in diesem Trait: sie sind `Exchange`-Anbieter. Konten (CalDAV, Google, Outlook) sind eigene Anbieter
//! DIESES Traits in eigenen Crates (`fs-calendar-caldav`, `-google`, `-outlook`), nicht Teil der Grundsachen.
//!
//! Zeiten (ADR 0085): Zeitpunkte UTC (`Utc`, ms); Termine mit lokaler Bedeutung = `LocalDateTime` + `Zone`;
//! ganztägig = `PlainDate` ohne Zone, nie umgerechnet; `When::Floating` ("überall 8 Uhr") ist KEIN Zeitpunkt,
//! sondern eine eigene Art wie das Datum ohne Uhrzeit: nie umgerechnet, nur durch Menschen gewählt. Schwebende
//! Zeit aus Fremdquellen wird an der Grenze aufgelöst und trägt `zone_assumed = true`. Menschen (ADR 0085):
//! nur `PersonRef` bzw. `Attendee`, nie ein freier Name. Stand: ADR 0085 akzeptiert (Abschluss 2026-11-16).
//! Speicher (ADR 0086): jedes Feld unten hat eine typisierte Spalte (siehe PLAN.md Abschnitt 2).

#![allow(dead_code)]

use async_trait::async_trait;

/// Gemeinsame Typen. EIGENTÜMER: fs-interfaces / fs-types (ADR 0085 "Folgen"). Hier nur als Stand-in, damit
/// die Signaturen allein prüfbar sind; Stufe 3 ersetzt das Modul durch `pub use` derselben Formen aus
/// fs-types. Weicht die Form dort ab, ist das ein Sprung (neuer Plan), kein stilles Anpassen.
pub mod shared {
    /// Objekt-Id, UUIDv7 (ADR 0027).
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct ObjectId(pub [u8; 16]);
    /// Zeilenstand für Vergleich-und-Setzen; steigt mit jedem Schreiben.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct Revision(pub u64);
    /// Zeitpunkt in UTC, Millisekunden seit 1970-01-01T00:00:00Z.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Utc(pub i64);
    /// Datum ohne Uhrzeit (proleptisch gregorianisch). Stufe 3: `pub use fs_types::Date as PlainDate`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct PlainDate { pub year: i32, pub month: u8, pub day: u8 }
    /// Wanduhrzeit ohne Zone (JSCalendar `LocalDateTime`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct LocalDateTime { pub date: PlainDate, pub hour: u8, pub minute: u8, pub second: u8 }
    /// Zeitzone: IANA-Name oder eigene Zone aus einer `VTIMEZONE` (Kennung beginnt mit `/`, RFC 8984 4.7.2).
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub enum Zone { Iana(String), Custom(String) }
    /// Absolute IRI wie geschrieben (= `fs_types::Reference`): lokal `urn:uuid:<UUIDv7>`, fremd `https://…`.
    /// Wird nie umgeschrieben (ADR 0085 Abschnitt 3).
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub struct Reference(pub String);
    /// Art des Ziels eines Personen-Verweises, AS2-Namen (ADR 0085 Abschnitt 3). Zwischenspeicher: bei
    /// Abweichung gilt das Ziel.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub enum PersonArt { Profile, Person, Group, Organization }
    /// Personen-Verweis (ADR 0085). `abzug` nur bei ausgestellten Dokumenten (z. B. Buchung), sonst `None`.
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub struct PersonRef { pub id: Reference, pub art: PersonArt, pub abzug: Option<ObjectId> }
    /// Medientyp (IANA).
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub struct MediaType(pub String);
    /// Datei in fs-files mit Prüfsumme (ADR 0086 Nachtrag).
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub struct FileRef { pub iri: Reference, pub sha256: [u8; 32], pub media_type: MediaType }
    /// Undurchsichtige Stelle zum Weiterblättern.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Place(pub String);
    /// Eine Seite eines Ergebnisses.
    #[derive(Clone, Debug)]
    pub struct Page<T> { pub items: Vec<T>, pub next: Option<Place> }
    /// Fähigkeit je Kennung (`event.create` …): `true` = angeboten.
    #[derive(Clone, Debug, Default)]
    pub struct Capabilities(pub Vec<(String, bool)>);
    /// Ein Prüffehler an einem Feld.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Problem { pub field: String, pub reason: String }
    /// Fehler des Themas.
    #[derive(Clone, Debug)]
    pub enum Error {
        /// Der Anbieter kann es ausdrücklich nicht (ADR 0045), mit Fähigkeitskennung.
        Unsupported { capability: String, reason: String },
        /// Vergleich-und-Setzen gescheitert: jemand hat vorher geschrieben.
        Changed { current: Revision },
        NotFound(ObjectId),
        /// Prüfung gescheitert; nichts geschrieben.
        Invalid(Vec<Problem>),
        /// Zone unbekannt (weder IANA noch eigene Zone dieses Termins); nie raten (ADR 0085).
        UnknownZone(String),
        /// Speicher oder Anbieter nicht erreichbar.
        Unavailable(String),
    }
    pub type Result<T> = std::result::Result<T, Error>;
}

use shared::*;

/// Das ausdrückliche "nicht möglich" einer nicht überschriebenen Methode.
pub fn not_possible<T>(capability: &str) -> Result<T> {
    Err(Error::Unsupported { capability: capability.to_owned(), reason: "this provider does not offer it".to_owned() })
}

/// Fähigkeitskennungen dieses Themas (Wortlaut für `capabilities`, Manifest und `events.capnp` v1).
pub const CAPABILITIES: [&str; 12] = [
    "calendar.read", "calendar.create", "event.read", "event.create", "event.update", "event.delete",
    "event.restore", "event.search", "invite.create", "invite.answer", "event.history", "event.freebusy",
];

// ---------------------------------------------------------------------------------------------------------
// Zeit eines Termins
// ---------------------------------------------------------------------------------------------------------

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
    /// umgerechnet und in der Zone des Betrachters gezeigt. Nur durch Menschen gewählt (ADR 0085 Abschnitt 2).
    Floating { start: LocalDateTime, length: Span },
    /// Ganztägig: erster Tag und Zahl der Tage (>= 1). Keine Zone, nie umgerechnet.
    AllDay { first: PlainDate, days: u32 },
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
}
/// Kanal, über den ein Objekt hereinkam.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OriginChannel { File, CalDav, Google, Outlook, Itip, Hub }

// ---------------------------------------------------------------------------------------------------------
// Aufzählungen (Wortlaut RFC 8984 / JSCalendar 2.0)
// ---------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum Status { Confirmed, Tentative, Cancelled }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum FreeBusy { Busy, Free }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum Privacy { Public, Private, Secret }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum TextType { Plain, Html }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum Weekday { Mo, Tu, We, Th, Fr, Sa, Su }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum Frequency { Yearly, Monthly, Weekly, Daily, Hourly, Minutely, Secondly }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum Skip { Omit, Backward, Forward }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum ParticipationStatus { NeedsAction, Accepted, Declined, Tentative, Delegated }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum Answer { Accept, Tentative, Decline }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum RelativeTo { Start, End }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum AlertAction { Display, Email }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum Relation { First, Next, Child, Parent }

// ---------------------------------------------------------------------------------------------------------
// Unterobjekte (je eine Untertabelle, PLAN.md Abschnitt 2)
// ---------------------------------------------------------------------------------------------------------

/// Wochentag mit optionaler Stelle im Zeitraum (`-1` = letzter).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NDay { pub day: Weekday, pub nth: Option<i16> }
/// Monat mit Schaltmonat-Marke (RFC 7529 `L`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ByMonth { pub month: u8, pub leap: bool }
/// Ende einer Regel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RuleEnd { Never, Count(u32), Until(LocalDateTime) }

/// Wiederholungsregel (RFC 8984 `RecurrenceRule`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecurrenceRule {
    pub frequency: Frequency,
    pub interval: u32,
    pub rscale: Option<String>,
    pub skip: Skip,
    pub first_day_of_week: Weekday,
    pub by_day: Vec<NDay>,
    pub by_month_day: Vec<i16>,
    pub by_month: Vec<ByMonth>,
    pub by_year_day: Vec<i16>,
    pub by_week_no: Vec<i16>,
    pub by_hour: Vec<u8>,
    pub by_minute: Vec<u8>,
    pub by_second: Vec<u8>,
    pub by_set_position: Vec<i32>,
    pub end: RuleEnd,
}

/// Wohin eine Einladung oder Antwort geht (`sendTo` / `replyTo`): Methode (`imip`, `other`) und URI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Channel { pub method: String, pub uri: String }

/// Rollen eines Teilnehmers (`roles`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Roles { pub owner: bool, pub attendee: bool, pub optional: bool, pub informational: bool, pub chair: bool, pub contact: bool }

/// Wer teilnimmt: Mensch, KI oder Gruppe (`PersonRef`), Ressource (Gerät, Raum als Ressource) oder Ort, beide
/// Verweise auf fs-inventory (ADR 0087). Ein Raum ist kein Akteur (ADR 0076 Regel 1). Der Export leitet die
/// JSCalendar-`kind` ab: Actor+Person/Profile = individual, Group/Organization = group, Resource, Location.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Attendee { Actor(PersonRef), Resource(Reference), Location(Reference) }

/// Teilnehmer. Kein Name-Feld: der Name kommt aus `who` (ADR 0085).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Participant {
    /// Schlüssel im Termin (JSCalendar `Id`).
    pub key: String,
    pub who: Attendee,
    pub roles: Roles,
    pub status: ParticipationStatus,
    pub expect_reply: bool,
    pub comment: Option<String>,
    pub send_to: Vec<Channel>,
    pub invited_by: Option<String>,
    pub delegated_to: Vec<String>,
    pub delegated_from: Vec<String>,
    pub schedule_sequence: u32,
    pub schedule_updated: Option<Utc>,
}

/// Auslöser einer Erinnerung.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger { Offset { seconds: i64, relative_to: RelativeTo }, Absolute { at: Utc } }

/// Erinnerung (`alerts`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alert { pub key: String, pub trigger: Trigger, pub action: AlertAction, pub acknowledged: Option<Utc> }

/// Ort. `place`: Verweis auf fs-maps (`places`, Strang D E17), sonst nur Text/Koordinaten.
#[derive(Clone, Debug, PartialEq)]
pub struct Location {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub types: Vec<String>,
    pub coordinates: Option<(f64, f64)>,
    pub zone: Option<Zone>,
    pub relative_to: Option<RelativeTo>,
    pub place: Option<ObjectId>,
}

/// Virtueller Ort (Videoanruf).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VirtualLocation { pub key: String, pub name: String, pub uri: String, pub description: Option<String>, pub features: Vec<String> }

/// Verweis (Anhang, URL). Eine Datei bei uns ist `file`, eine fremde Adresse `href`; genau eins von beiden.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link { pub key: String, pub href: Option<String>, pub file: Option<FileRef>, pub content_type: Option<MediaType>, pub size: Option<u64>, pub rel: Option<String>, pub title: Option<String> }

/// Beziehung zu einem anderen Termin per `uid` (z. B. Serie geteilt: `First`/`Next`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Related { pub uid: String, pub relation: Relation }

/// Übersetzung eines Textfelds (`localizations`, nur Textfelder).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Localization { pub language: String, pub field: TextField, pub text: String }
/// Übersetzbare Textfelder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextField { Title, Description, LocationName(String), LocationDescription(String) }

// ---------------------------------------------------------------------------------------------------------
// Termin, Entwurf, Änderung, Ausnahme
// ---------------------------------------------------------------------------------------------------------

/// Ein Kalender (Sammlung von Terminen). Sichtbarkeit und Farbe in der Seitenleiste sind Einstellung der
/// Person, nicht Daten; `color` hier ist die Vorgabe des Kalenders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarInfo { pub id: ObjectId, pub name: String, pub color: Option<String>, pub default_zone: Option<Zone>, pub owner: PersonRef, pub revision: Revision }

/// Der Termin (JSCalendar `Event`), vollständig typisiert.
#[derive(Clone, Debug, PartialEq)]
pub struct Appointment {
    pub id: ObjectId,
    pub calendar: ObjectId,
    pub uid: String,
    pub revision: Revision,
    pub sequence: u32,
    pub created: Utc,
    pub updated: Utc,
    pub title: String,
    pub description: String,
    pub description_type: TextType,
    pub locale: Option<String>,
    pub when: When,
    pub status: Status,
    pub free_busy: FreeBusy,
    pub privacy: Privacy,
    pub priority: u8,
    pub color: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
    pub locations: Vec<Location>,
    pub virtual_locations: Vec<VirtualLocation>,
    pub links: Vec<Link>,
    pub participants: Vec<Participant>,
    pub reply_to: Vec<Channel>,
    pub alerts: Vec<Alert>,
    pub use_default_alerts: bool,
    pub recurrence: Vec<RecurrenceRule>,
    pub excluded_recurrence: Vec<RecurrenceRule>,
    pub overrides: Vec<Override>,
    pub related: Vec<Related>,
    pub localizations: Vec<Localization>,
    pub hide_attendees: bool,
    pub may_invite_self: bool,
    pub may_invite_others: bool,
    /// Herkunft, wenn importiert (nur lesend; `create`/`update` ignorieren es).
    pub origin: Option<Origin>,
}

/// Neuer Termin. `uid = None`: der Anbieter vergibt eine.
#[derive(Clone, Debug, PartialEq)]
pub struct AppointmentDraft {
    pub calendar: ObjectId,
    pub uid: Option<String>,
    pub title: String,
    pub when: When,
    pub rest: AppointmentPatch,
}

/// Änderung: `Some` = Feld wird gesetzt (Listen ganz ersetzt), `None` = bleibt. Dieselbe Form beschreibt eine
/// Ausnahme einer Serie (JSCalendar `PatchObject`, typisiert).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct AppointmentPatch {
    pub title: Option<String>,
    pub description: Option<(String, TextType)>,
    pub locale: Option<Option<String>>,
    pub when: Option<When>,
    pub status: Option<Status>,
    pub free_busy: Option<FreeBusy>,
    pub privacy: Option<Privacy>,
    pub priority: Option<u8>,
    pub color: Option<Option<String>>,
    pub keywords: Option<Vec<String>>,
    pub categories: Option<Vec<String>>,
    pub locations: Option<Vec<Location>>,
    pub virtual_locations: Option<Vec<VirtualLocation>>,
    pub links: Option<Vec<Link>>,
    pub participants: Option<Vec<Participant>>,
    pub alerts: Option<Vec<Alert>>,
    pub use_default_alerts: Option<bool>,
    pub recurrence: Option<Vec<RecurrenceRule>>,
    pub excluded_recurrence: Option<Vec<RecurrenceRule>>,
    pub localizations: Option<Vec<Localization>>,
}

/// Eine Ausnahme einer Serie, benannt durch die ursprüngliche lokale Startzeit des Vorkommens.
#[derive(Clone, Debug, PartialEq)]
pub struct Override { pub recurrence_id: LocalDateTime, pub change: OverrideChange }
/// Ausnahme: Vorkommen fällt aus, oder es weicht ab.
#[derive(Clone, Debug, PartialEq)]
pub enum OverrideChange { Excluded, Changed(AppointmentPatch) }

/// Umfang einer Änderung an einer Serie (Serienfrage; Dossier A9/A10). Für Einzeltermine nur `Whole`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesScope {
    /// Der ganze Termin bzw. die ganze Serie.
    Whole,
    /// Nur dieses Vorkommen (wird eine Ausnahme).
    This { recurrence_id: LocalDateTime },
    /// Dieses und alle folgenden: die Serie wird geteilt (neue `uid`, `Related::Next`/`First`).
    ThisAndFollowing { recurrence_id: LocalDateTime },
}

// ---------------------------------------------------------------------------------------------------------
// Abfragen und Antworten
// ---------------------------------------------------------------------------------------------------------

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

/// Kurzform für Listen und Suche.
#[derive(Clone, Debug, PartialEq)]
pub struct AppointmentSummary { pub id: ObjectId, pub calendar: ObjectId, pub uid: String, pub title: String, pub when: When, pub recurring: bool, pub revision: Revision }

/// Filter für `list`.
#[derive(Clone, Debug, Default)]
pub struct AppointmentQuery { pub calendars: Vec<ObjectId>, pub text: Option<String>, pub from: Option<Utc>, pub to: Option<Utc>, pub participant: Option<PersonRef>, pub include_deleted: bool }

/// Wen einladen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invitee { pub person: PersonRef, pub roles: Roles, pub expect_reply: bool }

/// Ergebnis eines Löschens (Papierkorb; `restore` holt zurück, bis die Frist endet).
#[derive(Clone, Debug, PartialEq)]
pub struct Deleted { pub id: ObjectId, pub scope: SeriesScope, pub restorable_until: Option<Utc> }

/// Belegte Zeit einer Person (frei/belegt, Dossier A5 "Verfügbarkeit").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Busy { pub person: PersonRef, pub start: Utc, pub end: Utc, pub tentative: bool }

/// Ein typisierter Eintrag im Aktivitätsprotokoll (ADR 0086 Punkt 2).
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryEntry { pub at: Utc, pub actor: PersonRef, pub kind: HistoryKind, pub changes: Vec<FieldChange> }
/// Art der Aktivität (AS2-Name).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryKind { Create, Update, Delete, Undo, Invite, Accept, TentativeAccept, Reject }
/// Feld, alter Wert, neuer Wert.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldChange { pub field: String, pub row_key: Option<String>, pub old: Option<Value>, pub new: Option<Value> }
/// Typisierter Wert im Protokoll.
#[derive(Clone, Debug, PartialEq)]
pub enum Value { Int(i64), Real(f64), Text(String), Bool(bool), Utc(Utc), Local(LocalDateTime), Date(PlainDate), Zone(Zone), Ref(ObjectId), Person(PersonRef) }

/// Änderung für Beobachter (Raster neu laden).
#[derive(Clone, Debug, PartialEq)]
pub struct Change { pub appointment: ObjectId, pub calendar: ObjectId, pub kind: HistoryKind, pub at: Utc }

// ---------------------------------------------------------------------------------------------------------
// Der Trait
// ---------------------------------------------------------------------------------------------------------

/// Termine: Kalender, Termine anlegen/ändern/löschen, Serien, Einladungen, Erinnerungen, Zeitfenster.
///
/// Ziel v1 (ADR 0045). Erster Anbieter: lokal über fs-objects v5 (`LocalAppointments`). Weitere Anbieter
/// (CalDAV, Google, Outlook) implementieren DENSELBEN Trait; was sie nicht können, ist `Unsupported`.
#[async_trait]
pub trait Appointments: Send + Sync {
    /// Was dieser Anbieter kann, je Kennung aus [`CAPABILITIES`].
    fn capabilities(&self) -> Capabilities;

    /// `calendar.read`: alle Kalender, die die fragende Person sehen darf.
    async fn calendars(&self) -> Result<Vec<CalendarInfo>>;
    /// `calendar.create`
    async fn create_calendar(&self, _name: &str, _color: Option<&str>, _default_zone: Option<Zone>) -> Result<CalendarInfo> {
        not_possible("calendar.create")
    }

    /// `event.read`
    async fn get(&self, id: ObjectId) -> Result<Option<Appointment>>;
    /// `event.read` über die `uid`.
    async fn get_by_uid(&self, uid: &str) -> Result<Option<Appointment>>;
    /// `event.read`: Termine (Serien einmal), nach erstem Start sortiert, seitenweise.
    async fn list(&self, query: &AppointmentQuery, after: Option<&Place>, size: usize) -> Result<Page<AppointmentSummary>>;
    /// `event.read`: Vorkommen, die `[from, to)` überlappen, Serien aufgelöst (in der Zone des Termins
    /// gerechnet, ADR 0085), aus den gewählten Kalendern (leer = alle). Beschädigtes kostet nur sich.
    /// `view_zone`: Zone des Betrachters; legt `Instance.start/end` für `AllDay` und `Floating` fest.
    async fn window(&self, from: Utc, to: Utc, view_zone: &Zone, calendars: &[ObjectId]) -> Result<Window>;
    /// `event.search`: Volltext in Titel, Beschreibung, Ort.
    async fn search(&self, _text: &str, _after: Option<&Place>, _size: usize) -> Result<Page<AppointmentSummary>> {
        not_possible("event.search")
    }

    /// `event.create`: prüfen und speichern. Abgelehnt, wenn die `uid` schon existiert.
    async fn create(&self, _draft: AppointmentDraft) -> Result<Appointment> {
        not_possible("event.create")
    }
    /// `event.update`, Vergleich-und-Setzen gegen `expected`; `scope` bei Serien (sonst `Whole`).
    /// Verschieben (A8), Erinnerung (A6), Zone (A7), ganztägig (A3) sind Felder dieser Änderung.
    async fn update(&self, _id: ObjectId, _expected: Revision, _change: AppointmentPatch, _scope: SeriesScope) -> Result<Appointment> {
        not_possible("event.update")
    }
    /// `event.delete`: in den Papierkorb; `This` legt eine Ausnahme `Excluded` an, `ThisAndFollowing`
    /// kürzt die Serie (`RuleEnd::Until`).
    async fn delete(&self, _id: ObjectId, _expected: Revision, _scope: SeriesScope) -> Result<Deleted> {
        not_possible("event.delete")
    }
    /// `event.restore`: Löschen rückgängig machen, solange `restorable_until` nicht vorbei ist.
    async fn restore(&self, _id: ObjectId) -> Result<Appointment> {
        not_possible("event.restore")
    }

    /// `invite.create`: Personen einladen (Teilnehmer + Protokoll in einem Schritt). Versand (iMIP) ist
    /// ein `Exchange`-Anbieter, nicht dieser Trait.
    async fn invite(&self, _id: ObjectId, _expected: Revision, _invitees: &[Invitee], _scope: SeriesScope) -> Result<Appointment> {
        not_possible("invite.create")
    }
    /// `invite.answer`: als `who` antworten; `proposal` = neuer Zeitpunkt vorschlagen (Dossier A11).
    async fn respond(&self, _id: ObjectId, _who: &PersonRef, _answer: Answer, _comment: Option<&str>, _proposal: Option<When>, _scope: SeriesScope) -> Result<Appointment> {
        not_possible("invite.answer")
    }
    /// `event.freebusy`: belegte Zeiten von Personen im Fenster.
    async fn free_busy(&self, _people: &[PersonRef], _from: Utc, _to: Utc) -> Result<Vec<Busy>> {
        not_possible("event.freebusy")
    }

    /// `event.history`: typisiertes Protokoll eines Termins, älteste zuerst.
    async fn history(&self, _id: ObjectId, _after: Option<&Place>, _size: usize) -> Result<Page<HistoryEntry>> {
        not_possible("event.history")
    }
    /// `event.read`: Änderungen seit `since` (`None` = ab jetzt), für Beobachter (Raster neu laden).
    async fn changes(&self, _since: Option<&Place>, _size: usize) -> Result<Page<Change>> {
        not_possible("event.read")
    }
}

// ---------------------------------------------------------------------------------------------------------
// Grenze (ADR 0085 Abschnitt 1, Strang D E15): Formate liefern Anbieter außerhalb des Fach-Traits.
// ---------------------------------------------------------------------------------------------------------

/// Fähigkeit eines Formats je Richtung.
#[derive(Clone, Debug, PartialEq, Eq)]
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
/// [`Appointments`]; Export ist ein eigener Befehl, nie Speichern (ADR 0046).
#[async_trait]
pub trait Exchange: Send + Sync {
    fn formats(&self) -> Vec<FormatSupport>;
    /// `exchange.import`
    async fn import(&self, _source: FileRef, _as_type: &MediaType, _ctx: &ImportContext) -> Result<ImportReport> {
        not_possible("exchange.import")
    }
    /// `exchange.export`: Ziel = Quelle und unverändert = Original Byte für Byte; Ziel = Quelle, nur
    /// `zone_assumed`-Zeit unangetastet = wieder schwebend; sonst mit Zone (Rundlauf-Regel ADR 0085).
    async fn export(&self, _ids: &[ObjectId], _to: &ExportTarget) -> Result<ExportReport> {
        not_possible("exchange.export")
    }
}

/// VERBRAUCHT, nicht unser Thema: Personen-Verweis (ADR 0085 Abschnitt 3, Eigentümer fs-people /
/// fs-components-Baustein). Stand-in der Form, die fs-calendar braucht.
#[async_trait]
pub trait PersonDirectory: Send + Sync {
    /// Person zu einer Adresse (`mailto:…`), wenn bekannt.
    async fn find_by_address(&self, uri: &str) -> Result<Option<PersonRef>>;
    /// Unbekannte Adresse: Eintrag in der lokalen Ablage im Zustand "vorgeschlagen" (nie freier Name).
    async fn propose(&self, display_hint: &str, uri: &str) -> Result<PersonRef>;
    /// Anzeigename und Adresse für die Oberfläche (aktuell, nicht eingefroren).
    async fn display(&self, person: &PersonRef) -> Result<Option<(String, Option<String>)>>;
}
