//! Thema `events`, Ziel v1 (Sprung von "events 0.1 (draft)"). Eigentümer: fs-calendars (ADR 0050). Trait `Appointments`.
//!
//! Ordner (ADR 0043, jede Datei <= 300 Zeilen), ARBEITS-FIXPUNKT per `tools/hash-ordner.sh appointments_v1` (SHA-256
//! über sortierte relative Pfade und Inhalte, `LC_ALL=C`). Vorgänger-Fixpunkt `2b7fc98b…19b8` (Stand-in-Typen). Dieser
//! Stand = Typ-Tausch (Entscheidung 21, 2026-11-16: vor dem ersten Release IN v1 geändert, kein Sprung auf v2) plus
//! Kalender verwalten, Papierkorb-Frist und Abo-Kalender. Feature-Freeze: offen (Nutzer).
//! Dateien: `mod.rs` (dieser Kopf), `shared.rs` (`pub use` aus fs-types), `errors.rs` (Fehler des Themas), `time.rs`
//! (`Span`, `When`; `Origin` aus fs-jscalendar), `parts.rs` (Unterobjekte; Regel und Erinnerung aus fs-jscalendar),
//! `calendar.rs` (Kalender, Abo, Papierkorb-Frist), `appointment.rs`, `query.rs`, `api.rs` (Trait `Appointments`),
//! `exchange.rs` (Grenze `Exchange`, verbrauchtes `PersonDirectory`).
//!
//! GEMEINSAME TYPEN (Pins: fs-types = fs-interfaces Tag v0.0.22, fs-jscalendar Tag v0.0.2, der fs-types v0.0.22
//! verwendet; EINE fs-types-Quelle im Lock, sonst sind die Typen verschieden): `Utc`, `LocalDateTime`, `Date`, `Zone`,
//! `Reference`, `PersonRef`, `Attendee`, `ObjectId`, `Revision`, `MediaType`, `FileRef`, `Page`, `Capabilities`,
//! `Problem` aus fs-types (`Place` = `PageCursor`); `RecurrenceRule` samt Teilen, `Alert` samt Teilen, `Origin` aus
//! fs-jscalendar. Ein anderer Name, Konstruktor oder Zugriff dort ist ein Sprung dieses Traits (ADR 0045).
//!
//! Pattern: Repository (Trait) + Port (ADR 0043/0045). Jede Methode ist Pflicht, keine Standardmethode (Lehre aus
//! `groups_v1`/`packages_v1`: nichts versteckt Verhalten); was ein Anbieter nicht kann, sagt `capabilities` vorher,
//! damit kein Frontend einen toten Knopf zeigt, und der Aufruf antwortet ausdrücklich `Unsupported`
//! ([`not_possible`]). Formate (ICS, JSCalendar-JSON, iTIP) stehen NICHT in diesem Trait: sie sind
//! `Exchange`-Anbieter. Konten (`CalDAV`, Google, Outlook) sind eigene Anbieter DIESES Traits in eigenen Crates.
//! `#[non_exhaustive]` nur an Fehler-/Ergebnis-Enums, nie an Structs (der Fake baut sie außerhalb des Crates).
//!
//! Regeln des Vertrags: Wer handelt, ist die fragende Person des Anbieters, nie ein Parameter (außer `respond.who`,
//! geprüft); die Uhr ebenso (Papierkorb-Frist, Abruf). Jedes Schreiben auf Bestehendes ist Vergleich-und-Setzen gegen
//! `expected: Revision`; ausgenommen Anlegen, `restore`, `restore_calendar`, `refresh_calendar`, `purge`.
//! Papierkorb (ADR 0073 Nachtrag 2026-11-16): Löschen = Papierkorb mit Frist ([`Restorable`]); die Frist ist eine
//! Pflicht-Einstellung des Programms (Standard 30 Tage, mindestens 1 Tag, "nie automatisch" erlaubt), keine
//! Trait-Sache; eine Aufbewahrung (`keep_until`) geht vor; nach Ablauf geht das Objekt samt Verlauf (Kaskade).
//!
//! Zeiten (ADR 0085): Zeitpunkte UTC (`Utc`, ms); Termine mit lokaler Bedeutung = `LocalDateTime` + `Zone`;
//! ganztägig = `Date` ohne Zone, nie umgerechnet; `When::Floating` ("überall 8 Uhr") ist KEIN Zeitpunkt, sondern eine
//! eigene Art: nie umgerechnet; entsteht nur durch Menschen, durch die eigene Marke (`X-FS-FLOATING` /
//! `freesynergy.net:floating`) oder über den Kanal `Hub`, nie durch eine Zeit ohne Zone allein (Konformitätstest:
//! Datei mit Zeit ohne Zone und ohne Marke ergibt `Timed { zone_assumed: true }`, nie `Floating`). Zonen: EINE Quelle
//! der Wahrheit (Entscheidung 2a): die Form prüft `fs_types::Zone`, ob es eine IANA-Zone gibt, die tz-Datenbank
//! (fs-jscalendar); eine unbekannte Zone ist `Error::Invalid` mit dem Zonen-Code von fs-types, kein eigener Fehler.
//! Menschen (ADR 0085): nur `PersonRef` bzw. `Attendee`, nie ein freier Name. Speicher (ADR 0086): jedes Feld hat
//! eine typisierte Spalte (PLAN.md Abschnitt 2).
//!
//! ERWEITERUNGSPUNKT Terminfindung ("Doodle"): KEIN Feature dieses Traits. Vorbereitet ist nur, was eine spätere
//! Abstimmung verbraucht: `free_busy` (auch für Räume/Ressourcen), `create` + `invite` für den gewählten Termin,
//! `respond(.., proposal)` für Gegenvorschläge. Die Abstimmung selbst (Vorschläge, Stimmen, Gäste) wird ein eigener
//! Trait desselben Eigentümers mit eigenem Fixpunkt; dieser Trait ändert sich dafür nicht.
//!
//! OFFEN / ABHÄNGIG VON NUTZERFRAGEN (nicht festgeschrieben; Optionen mit Empfehlung, `appointments_v1-delta.md`):
//! - Frage 22 (Gruppe als Eigentümerin): `CalendarInfo.owner` ist `PersonRef` und kann eine Gruppe ausdrücken; ob
//!   `create_calendar` einen Eigentümer annimmt, ist offen. Empfehlung: ja, Parameter `owner` (Person oder Gruppe),
//!   Rechte über fs-access; bis zur Antwort ist Eigentümer immer die fragende Person.
//! - Frage 24 (Erinnerung per Mail/Nachricht): kein Trait-Thema; `Alert` speichert nur, das Programm löst aus.
//! - Frage 25 (Abo per Link; Abstimmung ohne Konto): Form `CalendarSource::Subscribed` steht drin, Fähigkeiten
//!   `calendar.subscribe`/`calendar.refresh` melden `false`, bis der Nutzer zustimmt; Gäste ohne Konto betreffen
//!   nur den späteren Abstimmungs-Trait.

// Die Formen folgen JSCalendar Feld für Feld (Wahrheitswerte von `Roles` und `Appointment`, `OverrideChange::Changed` =
// `PatchObject`); ein Umbau nur für die Lints wäre ein Formwechsel gegen RFC 8984 und gegen den Fake.
#![allow(clippy::struct_excessive_bools, clippy::large_enum_variant)]

mod api;
mod appointment;
mod calendar;
mod errors;
mod exchange;
mod parts;
mod query;
pub mod shared;
mod time;

pub use api::*;
pub use appointment::*;
pub use calendar::*;
pub use errors::*;
pub use exchange::*;
pub use parts::*;
pub use query::*;
pub use shared::*;
pub use time::*;
