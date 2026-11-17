//! The example data of the main view: typed, never stored, never sent, never in the core, the server or a client.
//!
//! The ONLY place of this program that holds example data (a test greps the other sources). The titles are `MessageFormat` 2 ids
//! (`example-1` ...), the names in them are "Example A/B", never a real person. The days are offsets from the Monday of the week of
//! "today", so the example always fills the week the window opens on; a test gives a fixed day. [`mode`] decides whether the window
//! shows these items or what the server delivers: as soon as the server offers the capability `appointments`, the example is gone.
//!
//! Pattern: Value object (the entries) + Strategy switch (`mode`): the one place that tells example from real.

use fs_components_mf2::Mf2Texts;
use fs_components_widgets::calendar_view::week_of;
use fs_components_widgets::{CalendarItem, Day, Moment};

/// Every id of an example item starts with this; a real item never does.
pub const ID_PREFIX: &str = "preview:";

/// The capability whose presence means the server delivers real appointments.
pub const CAPABILITY: &str = "appointments";

/// Whether the main view shows example data or what the server delivers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The server delivers no appointments yet: the example is shown and marked "Preview".
    Preview,
    /// The server delivers appointments: no example data, no preview marker.
    Live,
}

/// Preview until the server offers the capability [`CAPABILITY`].
#[must_use]
pub fn mode(capabilities: &[String]) -> Mode {
    if capabilities.iter().any(|c| c == CAPABILITY) {
        Mode::Live
    } else {
        Mode::Preview
    }
}

/// An example calendar: its id, the id of its name and its colour category (0 to 5 of the theme).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Calendar {
    /// The id (also the id of the screen in the navigation).
    pub id: &'static str,
    /// The id of the word that names it.
    pub word: &'static str,
    /// The colour category of its appointments.
    pub category: u8,
}

/// The style of the coloured dot before a calendar in the legend: the status colour of its category, the same as the appointment
/// blocks (`fs-calendar-item-cN` of fs-components), so a dot and its blocks always have one colour.
pub const DOT_STYLES: &str = r"
.fs-cal-dot-c0 { background: var(--success); }
.fs-cal-dot-c1 { background: var(--info); }
.fs-cal-dot-c2 { background: var(--border-hover); }
.fs-cal-dot-c3 { background: var(--warning); }
.fs-cal-dot-c4 { background: var(--error); }
.fs-cal-dot-c5 { background: var(--accent); }
.fs-cal-dot { border-radius: 6px; }
";

/// The class of the dot of a calendar with this colour category.
#[must_use]
pub fn dot_class(category: u8) -> String {
    format!("fs-cal-dot-c{}", category % 6)
}

/// The four example calendars, in the order of the navigation.
pub const CALENDARS: [Calendar; 4] = [
    Calendar {
        id: "private",
        word: "screen-private",
        category: 0,
    },
    Calendar {
        id: "work",
        word: "screen-work",
        category: 1,
    },
    Calendar {
        id: "family",
        word: "screen-family",
        category: 4,
    },
    Calendar {
        id: "holidays",
        word: "screen-holidays",
        category: 3,
    },
];

/// When an entry happens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    /// From minute to minute of the day (0 to 1440).
    Timed(u16, u16),
    /// All day, for this many days.
    AllDay(i64),
}

/// One example appointment: where, which day (from the Monday of the week), when, and the number of its title.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The calendar id.
    pub calendar: &'static str,
    /// Days after the Monday of the week of today (negative: before).
    pub day: i64,
    /// The time.
    pub when: When,
    /// The number of the title word `example-N`.
    pub title: u8,
}

const fn at(
    calendar: &'static str,
    day: i64,
    from: (u16, u16),
    to: (u16, u16),
    title: u8,
) -> Entry {
    Entry {
        calendar,
        day,
        when: When::Timed(from.0 * 60 + from.1, to.0 * 60 + to.1),
        title,
    }
}

const fn all_day(calendar: &'static str, day: i64, days: i64, title: u8) -> Entry {
    Entry {
        calendar,
        day,
        when: When::AllDay(days),
        title,
    }
}

/// The sixteen example appointments: this week (two overlap on Tuesday), the week before, the weeks after.
pub const ENTRIES: [Entry; 16] = [
    at("work", 0, (9, 0), (9, 30), 1),
    at("work", 1, (10, 0), (11, 30), 2),
    at("work", 1, (10, 30), (11, 0), 3),
    at("private", 1, (18, 30), (20, 0), 4),
    at("family", 2, (16, 30), (17, 30), 5),
    at("work", 2, (12, 30), (13, 30), 6),
    at("private", 3, (7, 0), (8, 0), 7),
    at("work", 3, (14, 0), (16, 0), 8),
    all_day("family", 4, 1, 9),
    all_day("holidays", 2, 1, 10),
    at("private", 5, (10, 0), (12, 0), 11),
    at("family", 6, (12, 0), (14, 0), 12),
    at("work", 7, (9, 0), (10, 0), 13),
    at("private", 12, (19, 0), (21, 0), 14),
    at("work", -3, (15, 0), (16, 0), 15),
    all_day("holidays", 14, 3, 16),
];

/// The id of entry `n` (counted from 1): `preview:3`.
#[must_use]
pub fn id_of(n: usize) -> String {
    format!("{ID_PREFIX}{n}")
}

/// One example item with the calendar it belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// The calendar id.
    pub calendar: &'static str,
    /// What the widget draws.
    pub item: CalendarItem,
}

/// The example as items around `today`, titles in the language of `texts`, colours by calendar.
#[must_use]
pub fn items(texts: &Mf2Texts, today: Day) -> Vec<Item> {
    let monday = week_of(today, 0);
    ENTRIES
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let day = monday.add_days(e.day);
            let title = texts.plain(&format!("example-{}", e.title));
            let key = id_of(i + 1);
            let item = match e.when {
                When::Timed(from, to) => CalendarItem::timed(
                    key,
                    title,
                    Moment::at(day, from / 60, from % 60),
                    Moment::at(day, to / 60, to % 60),
                ),
                When::AllDay(n) => CalendarItem::all_day(key, title, day, day.add_days(n - 1)),
            };
            let category = CALENDARS
                .iter()
                .find(|c| c.id == e.calendar)
                .map_or(0, |c| c.category);
            Item {
                calendar: e.calendar,
                item: item.in_category(category),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{DOT_STYLES, dot_class};

    #[test]
    fn every_calendar_has_a_dot_of_its_own_colour() {
        // the dot of a calendar and the blocks of its appointments use the same category, and the style gives it the colour of
        // the block (success, info, warning, error)
        let want = [
            (0, "--success"),
            (1, "--info"),
            (4, "--error"),
            (3, "--warning"),
        ];
        for (c, (category, var)) in super::CALENDARS.iter().zip(want) {
            assert_eq!(c.category, category, "{}", c.id);
            let rule = format!(".{} {{ background: var({var}); }}", dot_class(c.category));
            assert!(DOT_STYLES.contains(&rule), "{rule}");
        }
    }

    use super::{CALENDARS, ENTRIES, Mode, items, mode};
    use crate::words::words;
    use fs_components_widgets::Day;
    use fs_components_words::Locale;

    #[test]
    fn the_capability_of_the_server_ends_the_example() {
        assert_eq!(mode(&[]), Mode::Preview);
        assert_eq!(mode(&["history".to_owned()]), Mode::Preview);
        assert_eq!(mode(&["appointments".to_owned()]), Mode::Live);
    }

    #[test]
    fn every_entry_belongs_to_a_calendar_and_has_a_word() {
        let texts = words(&Locale::english());
        let list = items(&texts, Day::new(2026, 11, 18).unwrap());
        assert_eq!(list.len(), ENTRIES.len());
        for item in &list {
            assert!(CALENDARS.iter().any(|c| c.id == item.calendar));
            assert!(
                !item.item.title.starts_with("example-"),
                "{}",
                item.item.title
            );
            assert!(item.item.key.starts_with(super::ID_PREFIX));
        }
    }

    #[test]
    fn the_week_of_today_holds_the_first_entries() {
        let texts = words(&Locale::english());
        // Wednesday 2026-11-18: the Monday of its week is 2026-11-16.
        let list = items(&texts, Day::new(2026, 11, 18).unwrap());
        assert_eq!(list[0].item.start.day, Day::new(2026, 11, 16).unwrap());
        assert_eq!(list[0].item.title, "Stand-up");
        // the same week seen from Sunday gives the same Monday
        let sunday = items(&texts, Day::new(2026, 11, 22).unwrap());
        assert_eq!(sunday[0].item.start.day, list[0].item.start.day);
    }
}
