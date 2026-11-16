//! Thema `events`, Ziel v1 (Sprung von "events 0.1 (draft)"). Eigentümer: fs-calendar (ADR 0050).
//!
//! FESTER Trait des Piloten `programm-grundgeruest` (Stufe 2). Nach `strategie-ok` wird dieser Ordner per
//! SHA-256 gesperrt; jede Änderung ist ein Sprung (ADR 0045) und ein neuer Plan. Stufe 3 kopiert den Ordner
//! nach `fs-calendar-core/src/interface/appointments_v1/`.
//!
//! SPERRBEREICH: Gesperrt wird der ORDNER `appointments_v1/` als Ganzes (ADR 0043: kleine Dateien, jede
//! <= 300 Zeilen). Hash = `tools/hash-ordner.sh appointments_v1` (SHA-256 über sortierte relative Pfade und
//! Inhalte, `LC_ALL=C`). Dateien: `mod.rs` (dieser Kopf), `shared.rs` (Stand-in der Basistypen), `time.rs`,
//! `parts.rs`, `appointment.rs`, `query.rs`, `api.rs` (Trait `Appointments`), `exchange.rs` (Grenze,
//! `Exchange`, `PersonDirectory`). Stufe 3 ersetzt NUR `shared.rs` durch `pub use` der echten Typen aus
//! fs-types/fs-interfaces; alle anderen Dateien bleiben byte-gleich (prüfbar mit `sha256sum` je Datei). Das
//! ist KEIN Sprung, solange Namen, Konstruktoren (`parse`/`new`) und Zugriffsmethoden der echten Typen den
//! Stand-ins entsprechen. `Reference` und `Date` haben wie in fs-types private Felder und entstehen nur über
//! `parse` (Trait `BasicValue`); `Utc`, `LocalDateTime`, `Zone` gibt es in fs-types noch nicht (ADR 0085
//! "neu") und haben hier private Felder mit Konstruktoren, die fs-types so übernimmt. `PlainDate` heißt jetzt
//! `Date` (= `fs_types::Date`).
//!
//! Pattern: Repository (interface) + Null Object (ADR 0043/0045) - jeder Anbieter hat jede Methode; was er
//! nicht kann, antwortet `Unsupported` (Standardmethode). Formate (ICS, JSCalendar-JSON, iTIP) stehen NICHT
//! in diesem Trait: sie sind `Exchange`-Anbieter. Konten (CalDAV, Google, Outlook) sind eigene Anbieter
//! DIESES Traits in eigenen Crates (`fs-calendar-caldav`, `-google`, `-outlook`), nicht Teil der Grundsachen.
//!
//! Zeiten (ADR 0085): Zeitpunkte UTC (`Utc`, ms); Termine mit lokaler Bedeutung = `LocalDateTime` + `Zone`;
//! ganztägig = `Date` ohne Zone, nie umgerechnet; `When::Floating` ("überall 8 Uhr") ist KEIN Zeitpunkt,
//! sondern eine eigene Art wie das Datum ohne Uhrzeit: nie umgerechnet; entsteht nur durch Menschen, durch die
//! eigene Marke (`X-FS-FLOATING` / `freesynergy.net:floating`) oder über den Kanal `Hub`, nie durch eine Zeit ohne
//! Zone allein (Konformitätstest: Datei mit Zeit ohne Zone und ohne Marke ergibt `Timed { zone_assumed: true }`,
//! nie `Floating`; ADR 0085 Abschnitt 2). Schwebende
//! Zeit aus Fremdquellen wird an der Grenze aufgelöst und trägt `zone_assumed = true`. Menschen (ADR 0085):
//! nur `PersonRef` bzw. `Attendee`, nie ein freier Name. Stand: ADR 0085 akzeptiert (Abschluss 2026-11-16).
//! Speicher (ADR 0086): jedes Feld unten hat eine typisierte Spalte (siehe PLAN.md Abschnitt 2).


mod api;
mod appointment;
mod exchange;
mod parts;
mod query;
pub mod shared;
mod time;

pub use api::*;
pub use appointment::*;
pub use exchange::*;
pub use parts::*;
pub use query::*;
pub use shared::*;
pub use time::*;
