//! The words of the calendar view and the title of the shown range, from the `MessageFormat` 2 texts.
//!
//! Pattern: Adapter (`Mf2Texts` in, `CalendarNames` out) plus pure functions for the title: the same pair of numbers always gives the
//! same words, so a test can read them without a window.

use fs_components_mf2::Mf2Texts;
use fs_components_widgets::calendar_view::{AGENDA_DAYS, span_of};
use fs_components_widgets::{CalendarNames, Day, ViewRange};

/// The widget's words in the language of `texts`: weekdays, months, "all day", the empty line and the date order.
#[must_use]
pub fn names(texts: &Mf2Texts) -> CalendarNames {
    let weekdays = std::array::from_fn(|i| texts.plain(&format!("weekday-{}", i + 1)));
    let months = std::array::from_fn(|i| texts.plain(&format!("month-{}", i + 1)));
    let mut names = CalendarNames::new(
        weekdays,
        months,
        texts.plain("all-day"),
        texts.plain("grid-empty"),
    );
    if german(texts) {
        names.date = "{weekday}, {day}. {month} {year}".into();
    }
    names
}

/// Whether the dates read day-month-year (the word `date-style` of the texts says `de`).
#[must_use]
pub fn german(texts: &Mf2Texts) -> bool {
    texts.plain("date-style") == "de"
}

/// The title of the shown range: "November 2026" for a month, "17-23 November 2026" for a week, the date for a day, the span for the
/// agenda. `dot` puts a point after the day of month (German).
#[must_use]
pub fn title(
    range: ViewRange,
    anchor: Day,
    week_start: u8,
    names: &CalendarNames,
    dot: bool,
) -> String {
    match range {
        ViewRange::Month => format!("{} {}", names.month_of(anchor), anchor.year()),
        ViewRange::Day => names.date_text(anchor),
        _ => {
            let (first, last) = span_of(range, anchor, week_start, AGENDA_DAYS);
            span(first, last, names, dot)
        }
    }
}

fn span(first: Day, last: Day, names: &CalendarNames, dot: bool) -> String {
    let point = if dot { "." } else { "" };
    let (a, b) = (first, last);
    if a.year() == b.year() && a.month() == b.month() {
        format!(
            "{}{point}\u{2013}{}{point} {} {}",
            a.day(),
            b.day(),
            names.month_of(b),
            b.year()
        )
    } else if a.year() == b.year() {
        format!(
            "{}{point} {} \u{2013} {}{point} {} {}",
            a.day(),
            names.month_of(a),
            b.day(),
            names.month_of(b),
            b.year()
        )
    } else {
        format!(
            "{}{point} {} {} \u{2013} {}{point} {} {}",
            a.day(),
            names.month_of(a),
            a.year(),
            b.day(),
            names.month_of(b),
            b.year()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{names, title};
    use crate::words::words;
    use fs_components_widgets::{Day, ViewRange};
    use fs_components_words::Locale;

    fn day(y: i32, m: u8, d: u8) -> Day {
        Day::new(y, m, d).unwrap()
    }

    #[test]
    fn the_titles_name_the_shown_range() {
        let en = names(&words(&Locale::english()));
        let de = names(&words(&Locale::parse("de").unwrap()));
        let wed = day(2026, 11, 18);
        assert_eq!(title(ViewRange::Month, wed, 0, &en, false), "November 2026");
        assert_eq!(
            title(ViewRange::Week, wed, 0, &en, false),
            "16\u{2013}22 November 2026"
        );
        assert_eq!(
            title(ViewRange::Week, wed, 0, &de, true),
            "16.\u{2013}22. November 2026"
        );
        assert_eq!(
            title(ViewRange::Day, wed, 0, &en, false),
            "Wed 18 November 2026"
        );
        assert_eq!(
            title(ViewRange::Day, wed, 0, &de, true),
            "Mi, 18. November 2026"
        );
    }

    #[test]
    fn a_week_over_the_turn_of_a_month_or_a_year_names_both_ends() {
        let en = names(&words(&Locale::english()));
        assert_eq!(
            title(ViewRange::Week, day(2026, 11, 30), 0, &en, false),
            "30 November \u{2013} 6 December 2026"
        );
        assert_eq!(
            title(ViewRange::Week, day(2026, 12, 31), 0, &en, false),
            "28 December 2026 \u{2013} 3 January 2027"
        );
    }
}
