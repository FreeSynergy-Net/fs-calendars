//! Der Trait `Appointments` und seine Fähigkeitskennungen.

use super::*;
use async_trait::async_trait;

/// Das ausdrückliche "nicht möglich" einer nicht überschriebenen Methode.
pub fn not_possible<T>(capability: &str) -> Result<T> {
    Err(Error::Unsupported { capability: capability.to_owned(), reason: "this provider does not offer it".to_owned() })
}

/// Fähigkeitskennungen dieses Themas (Wortlaut für `capabilities`, Manifest und `events.capnp` v1).
pub const CAPABILITIES: [&str; 12] = [
    "calendar.read", "calendar.create", "event.read", "event.create", "event.update", "event.delete",
    "event.restore", "event.search", "invite.create", "invite.answer", "event.history", "event.freebusy",
];
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

    /// `event.read`; ein gelöschter Termin ergibt `None` (sichtbar nur über `list` mit `include_deleted`).
    async fn get(&self, id: ObjectId) -> Result<Option<Appointment>>;
    /// `event.read` über die `uid`.
    async fn get_by_uid(&self, uid: &str) -> Result<Option<Appointment>>;
    /// `event.read`: Termine (Serien einmal), nach erstem Start sortiert, seitenweise.
    async fn list(&self, query: &AppointmentQuery, after: Option<&Place>, size: usize) -> Result<Page<AppointmentSummary>>;
    /// `event.read`: Vorkommen, die `[from, to)` überlappen, Serien aufgelöst (in der Zone des Termins
    /// gerechnet, ADR 0085), aus den gewählten Kalendern (leer = alle). Beschädigtes kostet nur sich.
    /// `view_zone`: Zone des Betrachters; legt `Instance.start/end` für `AllDay` und `Floating` fest. Eine
    /// eigene Zone (`ZoneKind::Custom`) gilt nur mit ihrem Objekt und ergibt als `view_zone` `UnknownZone`.
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
    /// `event.restore`: Löschen rückgängig machen, solange `restorable_until` nicht vorbei ist. `scope` ist
    /// derselbe wie beim Löschen (`This` nimmt die Ausnahme `Excluded` zurück, `ThisAndFollowing` den
    /// gekürzten Serienschluss, `Whole` holt den Termin aus dem Papierkorb); so ist Teil-Löschen umkehrbar.
    async fn restore(&self, _id: ObjectId, _scope: SeriesScope) -> Result<Appointment> {
        not_possible("event.restore")
    }

    /// `invite.create`: Personen einladen (Teilnehmer + Protokoll in einem Schritt). Versand (iMIP) ist
    /// ein `Exchange`-Anbieter, nicht dieser Trait.
    async fn invite(&self, _id: ObjectId, _expected: Revision, _invitees: &[Invitee], _scope: SeriesScope) -> Result<Appointment> {
        not_possible("invite.create")
    }
    /// `invite.answer`: als `who` antworten (schreibt nur dessen Teilnehmerzeile), Vergleich-und-Setzen gegen
    /// `expected`; `proposal` = neuer Zeitpunkt vorschlagen (Dossier A11). Wer handelt, ist Eigenschaft des
    /// Anbieters (die fragende Person), nicht dieses Aufrufs.
    async fn respond(&self, _id: ObjectId, _expected: Revision, _who: &PersonRef, _answer: Answer, _comment: Option<&str>, _proposal: Option<When>, _scope: SeriesScope) -> Result<Appointment> {
        not_possible("invite.answer")
    }
    /// `event.freebusy`: belegte Zeiten von Teilnehmern (Personen, Räume, Ressourcen) im Fenster.
    /// `view_zone`: Zone des Betrachters; legt die Lage von `AllDay` und `Floating` fest (wie bei `window`).
    async fn free_busy(&self, _who: &[Attendee], _from: Utc, _to: Utc, _view_zone: &Zone) -> Result<Vec<Busy>> {
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
