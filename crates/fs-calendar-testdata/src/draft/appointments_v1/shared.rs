//! Gemeinsame Typen (Stand-in). EIGENTÜMER: fs-interfaces / fs-types (ADR 0085 "Folgen").
//! Stufe 3 ersetzt GENAU DIESE DATEI durch `pub use` der echten Typen (siehe `mod.rs`, Sperrbereich).

/// Fehler beim Lesen eines Basistyps (Stand-in für `fs_types::TypeProblem`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeProblem { Empty, BadDate, BadTime, BadReference, BadZone, OutOfRange }
/// Stand-in für `fs_types::BasicValue`: Basistypen entstehen nur über `parse`.
pub trait BasicValue: Sized {
    fn parse(typed: &str) -> std::result::Result<Self, TypeProblem>;
    fn canonical(&self) -> String;
}
/// Objekt-Id, UUIDv7 (ADR 0027).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId(pub [u8; 16]);
/// Zeilenstand für Vergleich-und-Setzen; steigt mit jedem Schreiben.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Revision(pub u64);
/// Zeitpunkt in UTC, Millisekunden seit 1970-01-01T00:00:00Z. NEU in fs-types (Name `Utc`, ADR 0085).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Utc(i64);
impl Utc {
    pub fn from_millis(ms: i64) -> Self { Self(ms) }
    pub fn millis(&self) -> i64 { self.0 }
}
/// Datum ohne Uhrzeit (proleptisch gregorianisch) = `fs_types::Date`: privates Feld, nur über `parse`
/// (`YYYY-MM-DD`, muss existieren). Stufe 3: `pub use fs_types::Date`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date { year: i32, month: u8, day: u8 }
impl BasicValue for Date {
    fn parse(typed: &str) -> std::result::Result<Self, TypeProblem> {
        let t = typed.trim();
        if t.is_empty() { return Err(TypeProblem::Empty); }
        let b = t.as_bytes();
        let shape = b.len() == 10 && b[4] == b'-' && b[7] == b'-'
            && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
        if !shape { return Err(TypeProblem::BadDate); }
        let year: i32 = t[0..4].parse().map_err(|_| TypeProblem::BadDate)?;
        let month: u8 = t[5..7].parse().map_err(|_| TypeProblem::BadDate)?;
        let day: u8 = t[8..10].parse().map_err(|_| TypeProblem::BadDate)?;
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let max = match month { 1 | 3 | 5 | 7 | 8 | 10 | 12 => 31, 4 | 6 | 9 | 11 => 30, 2 if leap => 29, 2 => 28, _ => 0 };
        if day == 0 || day > max { return Err(TypeProblem::BadDate); }
        Ok(Self { year, month, day })
    }
    fn canonical(&self) -> String { format!("{:04}-{:02}-{:02}", self.year, self.month, self.day) }
}
/// Wanduhrzeit ohne Zone (JSCalendar `LocalDateTime`): Datum, Stunde, Minute, Sekunde; keine Zone, keine
/// Bruchsekunden. NEU in fs-types. Entsteht über `new` oder `parse` (`YYYY-MM-DDTHH:MM:SS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalDateTime { date: Date, hour: u8, minute: u8, second: u8 }
impl LocalDateTime {
    pub fn new(date: Date, hour: u8, minute: u8, second: u8) -> std::result::Result<Self, TypeProblem> {
        if hour > 23 || minute > 59 || second > 59 { return Err(TypeProblem::BadTime); }
        Ok(Self { date, hour, minute, second })
    }
    pub fn date(&self) -> Date { self.date }
    pub fn hour(&self) -> u8 { self.hour }
    pub fn minute(&self) -> u8 { self.minute }
    pub fn second(&self) -> u8 { self.second }
}
impl BasicValue for LocalDateTime {
    fn parse(typed: &str) -> std::result::Result<Self, TypeProblem> {
        let t = typed.trim();
        if t.len() != 19 || t.as_bytes()[10] != b'T' { return Err(TypeProblem::BadTime); }
        let date = Date::parse(&t[0..10])?;
        let num = |a: usize, z: usize| t[a..z].parse::<u8>().map_err(|_| TypeProblem::BadTime);
        if &t[13..14] != ":" || &t[16..17] != ":" { return Err(TypeProblem::BadTime); }
        Self::new(date, num(11, 13)?, num(14, 16)?, num(17, 19)?)
    }
    fn canonical(&self) -> String { format!("{}T{:02}:{:02}:{:02}", self.date.canonical(), self.hour, self.minute, self.second) }
}
/// Art einer Zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ZoneKind { Iana, Custom }
/// Zeitzone: IANA-Name oder eigene Zone aus einer `VTIMEZONE` (Kennung beginnt mit `/`, RFC 8984 4.7.2;
/// nur zusammen mit ihrem Objekt gültig, sonst `UnknownZone`). NEU in fs-types.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Zone { kind: ZoneKind, id: String }
impl Zone {
    pub fn iana(id: &str) -> std::result::Result<Self, TypeProblem> {
        if id.is_empty() || id.starts_with('/') { return Err(TypeProblem::BadZone); }
        Ok(Self { kind: ZoneKind::Iana, id: id.to_owned() })
    }
    pub fn custom(id: &str) -> std::result::Result<Self, TypeProblem> {
        if id.len() < 2 || !id.starts_with('/') { return Err(TypeProblem::BadZone); }
        Ok(Self { kind: ZoneKind::Custom, id: id.to_owned() })
    }
    pub fn kind(&self) -> ZoneKind { self.kind }
    pub fn id(&self) -> &str { &self.id }
}
/// Absolute IRI wie geschrieben = `fs_types::Reference`: privates Feld, nur über `parse` (muss absolut
/// sein, also `Schema:Rest`). Lokal `urn:uuid:<UUIDv7>`, fremd `https://…`. Wird nie umgeschrieben
/// (ADR 0085 Abschnitt 3). Stufe 3: `pub use fs_types::Reference`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Reference(String);
impl BasicValue for Reference {
    fn parse(typed: &str) -> std::result::Result<Self, TypeProblem> {
        let t = typed.trim();
        if t.is_empty() { return Err(TypeProblem::Empty); }
        match t.split_once(':') {
            Some((scheme, rest)) if !rest.is_empty()
                && scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                && scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)) => Ok(Self(t.to_owned())),
            _ => Err(TypeProblem::BadReference),
        }
    }
    fn canonical(&self) -> String { self.0.clone() }
}
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
    /// Nicht erlaubt (z. B. fremder Kalender, den man nur lesen darf), mit Fähigkeitskennung. Nicht
    /// `Unsupported`: der Anbieter kann es, die fragende Person darf es nicht.
    Forbidden { capability: String },
    /// Prüfung gescheitert; nichts geschrieben.
    Invalid(Vec<Problem>),
    /// Zone unbekannt (weder IANA noch eigene Zone dieses Termins); nie raten (ADR 0085).
    UnknownZone(String),
    /// Speicher oder Anbieter nicht erreichbar.
    Unavailable(String),
}
pub type Result<T> = std::result::Result<T, Error>;
