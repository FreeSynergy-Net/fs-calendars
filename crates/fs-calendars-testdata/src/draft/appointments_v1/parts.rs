//! Aufzählungen und Unterobjekte (je eine Untertabelle, PLAN.md Abschnitt 2).
//!
//! Pattern: Value Object (Unterobjekte und Aufzählungen, je eine Untertabelle).

use super::*;


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
/// Verweise auf fs-resources (ADR 0087). Ein Raum ist kein Akteur (ADR 0076 Regel 1). Der Export leitet die
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

/// Ort (Geografie). `place`: Verweis auf fs-maps (`places`, Strang D E17), sonst nur Text/Koordinaten. Ein
/// buchbarer Raum ist kein Ort, sondern `Attendee::Location`/`Resource` (fs-resources).
#[derive(Clone, Debug, PartialEq)]
pub struct Location {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub types: Vec<String>,
    pub coordinates: Option<(f64, f64)>,
    pub zone: Option<Zone>,
    pub relative_to: Option<RelativeTo>,
    pub place: Option<Reference>,
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
