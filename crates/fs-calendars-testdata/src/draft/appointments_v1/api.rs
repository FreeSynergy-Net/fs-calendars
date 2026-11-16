//! Der Trait `Appointments` und seine Fähigkeitskennungen.
//!
//! Pattern: Repository (Trait) + Port: jede Methode ist Pflicht, kein Standardverhalten; was ein Anbieter nicht kann,
//! sagt er vorher in `capabilities` und beim Aufruf ausdrücklich mit [`not_possible`] (`Unsupported`).

use super::{
    Answer, Appointment, AppointmentDraft, AppointmentPatch, AppointmentQuery, AppointmentSummary, Attendee, Busy,
    CalendarDeleted, CalendarInfo, CalendarPatch, Capabilities, Change, Deleted, Error, HistoryEntry, Invitee,
    ObjectId, Page, PersonRef, Place, Reference, Refresh, Refreshed, Result, Revision, SeriesScope, Utc, When, Window,
    Zone,
};
use async_trait::async_trait;

/// Das ausdrückliche "nicht möglich" eines Anbieters, der eine Fähigkeit nicht hat (er ruft es selbst auf; der
/// Trait hat keine Standardmethoden, damit kein Anbieter eine Methode still vergisst).
///
/// # Errors
/// Immer `Unsupported` mit der Fähigkeitskennung.
pub fn not_possible<T>(capability: &str) -> Result<T> {
    Err(Error::Unsupported { capability: capability.to_owned(), reason: "this provider does not offer it".to_owned() })
}

/// Fähigkeitskennungen dieses Themas (Wortlaut für `capabilities`, Manifest und `events.capnp` v1).
pub const CAPABILITIES: [&str; 18] = [
    "calendar.read", "calendar.create", "calendar.update", "calendar.delete", "calendar.restore", "calendar.subscribe",
    "calendar.refresh", "event.read", "event.create", "event.update", "event.delete", "event.restore", "event.search",
    "invite.create", "invite.answer", "event.history", "event.freebusy", "trash.purge",
];
/// Termine: Kalender, Termine anlegen/ändern/löschen, Serien, Einladungen, Erinnerungen, Zeitfenster.
///
/// Ziel v1 (ADR 0045). Erster Anbieter: lokal über fs-objects v5 (`LocalAppointments`). Weitere Anbieter
/// (`CalDAV`, Google, Outlook) implementieren DENSELBEN Trait; was sie nicht können, ist `Unsupported`.
#[async_trait]
pub trait Appointments: Send + Sync {
    /// Was dieser Anbieter kann, je Kennung aus [`CAPABILITIES`].
    fn capabilities(&self) -> Capabilities;

    /// `calendar.read`: alle Kalender, die die fragende Person sehen darf; `include_deleted` zeigt auch die im
    /// Papierkorb (`CalendarInfo.trash`).
    async fn calendars(&self, include_deleted: bool) -> Result<Vec<CalendarInfo>>;
    /// `calendar.create`: eigener Kalender (`CalendarSource::Own`), Eigentümer = fragende Person.
    async fn create_calendar(&self, name: &str, color: Option<&str>, default_zone: Option<Zone>) -> Result<CalendarInfo>;
    /// `calendar.update`: Name, Farbe, Vorgabe-Zone, bei einem Abo Adresse und Abrufregel; Vergleich-und-Setzen.
    async fn update_calendar(&self, id: ObjectId, expected: Revision, change: CalendarPatch) -> Result<CalendarInfo>;
    /// `calendar.delete`: Kalender samt seinen Terminen in den Papierkorb (eine Frist für alle, Verlauf geht mit).
    async fn delete_calendar(&self, id: ObjectId, expected: Revision) -> Result<CalendarDeleted>;
    /// `calendar.restore`: Kalender samt den Terminen zurück, die mit ihm gelöscht wurden (einzeln vorher gelöschte
    /// bleiben im Papierkorb mit ihrer eigenen Frist). `Changed`, wenn er seit dem Löschen geändert wurde.
    async fn restore_calendar(&self, id: ObjectId) -> Result<CalendarInfo>;
    /// `calendar.subscribe`: Abo-Kalender (`CalendarSource::Subscribed`) anlegen; ruft NICHT ab (`FetchState::Never`).
    async fn subscribe_calendar(&self, name: &str, color: Option<&str>, url: &Reference, refresh: Refresh) -> Result<CalendarInfo>;
    /// `calendar.refresh`: Abo jetzt abrufen (Klick). Ein gescheiterter Abruf ist kein `Err`, sondern
    /// `FetchState::Failed` im Ergebnis; `Err` nur, wenn es kein Abo ist (`Invalid`) oder der Speicher fehlt.
    async fn refresh_calendar(&self, id: ObjectId) -> Result<Refreshed>;

    /// `event.read`; ein gelöschter Termin ergibt `None` (sichtbar nur über `list` mit `include_deleted`).
    async fn get(&self, id: ObjectId) -> Result<Option<Appointment>>;
    /// `event.read` über die `uid`.
    async fn get_by_uid(&self, uid: &str) -> Result<Option<Appointment>>;
    /// `event.read`: Termine (Serien einmal), nach erstem Start sortiert, seitenweise.
    async fn list(&self, query: &AppointmentQuery, after: Option<&Place>, size: usize) -> Result<Page<AppointmentSummary>>;
    /// `event.read`: Vorkommen, die `[from, to)` überlappen, Serien aufgelöst (in der Zone des Termins
    /// gerechnet, ADR 0085), aus den gewählten Kalendern (leer = alle). Beschädigtes kostet nur sich.
    /// `view_zone`: Zone des Betrachters; legt `Instance.start/end` für `AllDay` und `Floating` fest. Eine
    /// eigene Zone (`ZoneKind::Custom`) gilt nur mit ihrem Objekt: als `view_zone` ist sie `Invalid` am Feld
    /// `view_zone` (Code der Zonenprüfung, siehe `Error`), ebenso eine IANA-Zone, die die tz-Datenbank nicht kennt.
    async fn window(&self, from: Utc, to: Utc, view_zone: &Zone, calendars: &[ObjectId]) -> Result<Window>;
    /// `event.search`: Volltext in Titel, Beschreibung, Ort.
    async fn search(&self, text: &str, after: Option<&Place>, size: usize) -> Result<Page<AppointmentSummary>>;

    /// `event.create`: prüfen und speichern. Abgelehnt, wenn die `uid` schon existiert; `ReadOnly` in einem Abo.
    async fn create(&self, draft: AppointmentDraft) -> Result<Appointment>;
    /// `event.update`, Vergleich-und-Setzen gegen `expected`; `scope` bei Serien (sonst `Whole`).
    /// Verschieben (A8), Erinnerung (A6), Zone (A7), ganztägig (A3) sind Felder dieser Änderung.
    async fn update(&self, id: ObjectId, expected: Revision, change: AppointmentPatch, scope: SeriesScope) -> Result<Appointment>;
    /// `event.delete`: in den Papierkorb; `This` legt eine Ausnahme `Excluded` an, `ThisAndFollowing`
    /// kürzt die Serie (`RuleEnd::Until`).
    async fn delete(&self, id: ObjectId, expected: Revision, scope: SeriesScope) -> Result<Deleted>;
    /// `event.restore`: Löschen rückgängig machen, solange `restorable` gilt. `scope` ist derselbe wie beim Löschen
    /// (`This` nimmt die Ausnahme `Excluded` zurück, `ThisAndFollowing` den gekürzten Serienschluss, `Whole` holt den
    /// Termin aus dem Papierkorb); so ist Teil-Löschen umkehrbar. Liegt sein Kalender im Papierkorb: `Invalid`
    /// (`calendar`), zuerst `restore_calendar`.
    async fn restore(&self, id: ObjectId, scope: SeriesScope) -> Result<Appointment>;
    /// `trash.purge`: einen Termin oder Kalender (samt Terminen) im Papierkorb endgültig löschen, mit Verlauf
    /// (ADR 0073 "wirklich löschen"; der Weg bei `Restorable::Manual`). Nicht im Papierkorb: `Invalid`.
    async fn purge(&self, id: ObjectId) -> Result<()>;

    /// `invite.create`: Personen einladen (Teilnehmer + Protokoll in einem Schritt). Versand (iMIP) ist
    /// ein `Exchange`-Anbieter, nicht dieser Trait.
    async fn invite(&self, id: ObjectId, expected: Revision, invitees: &[Invitee], scope: SeriesScope) -> Result<Appointment>;
    /// `invite.answer`: als `who` antworten (schreibt nur dessen Teilnehmerzeile), Vergleich-und-Setzen gegen
    /// `expected`; `proposal` = neuer Zeitpunkt vorschlagen (Dossier A11). Wer handelt, ist Eigenschaft des
    /// Anbieters (die fragende Person), nicht dieses Aufrufs.
    #[allow(clippy::too_many_arguments)]
    async fn respond(&self, id: ObjectId, expected: Revision, who: &PersonRef, answer: Answer, comment: Option<&str>, proposal: Option<When>, scope: SeriesScope) -> Result<Appointment>;
    /// `event.freebusy`: belegte Zeiten von Teilnehmern (Personen, Räume, Ressourcen) im Fenster.
    /// `view_zone`: Zone des Betrachters; legt die Lage von `AllDay` und `Floating` fest (wie bei `window`).
    /// Erweiterungspunkt Terminfindung (siehe `mod.rs`): eine spätere Abstimmung verbraucht genau diese Methode.
    async fn free_busy(&self, who: &[Attendee], from: Utc, to: Utc, view_zone: &Zone) -> Result<Vec<Busy>>;

    /// `event.history`: typisiertes Protokoll eines Termins oder Kalenders, älteste zuerst.
    async fn history(&self, id: ObjectId, after: Option<&Place>, size: usize) -> Result<Page<HistoryEntry>>;
    /// `event.read`: Änderungen seit `since` (`None` = ab jetzt), für Beobachter (Raster neu laden).
    async fn changes(&self, since: Option<&Place>, size: usize) -> Result<Page<Change>>;
}
