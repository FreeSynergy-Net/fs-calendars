//! Which items a screen shows, and which marker stands at the top.
//!
//! Pattern: Strategy (pure functions): the mode, the example switch and the chosen calendar decide; no Blinc, so tests read them directly.

use fs_components_widgets::CalendarItem;

use crate::preview::{Item, Mode};

/// The marker at the top of the main view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    /// The example is shown: say it is a preview, offer to hide it.
    Shown,
    /// The example is hidden: say so, offer to show it.
    Hidden,
}

/// The marker: only in preview mode, and exactly one of the two.
#[must_use]
pub fn notice(mode: Mode, example: bool) -> Option<Notice> {
    match (mode, example) {
        (Mode::Live, _) => None,
        (Mode::Preview, true) => Some(Notice::Shown),
        (Mode::Preview, false) => Some(Notice::Hidden),
    }
}

/// The items of the screen of `calendar` (`None` is "all calendars"): the example while it is shown in preview mode, else nothing
/// (the client cannot read appointments yet, so real mode has none to show).
#[must_use]
pub fn visible(
    mode: Mode,
    example: bool,
    items: &[Item],
    calendar: Option<&str>,
) -> Vec<CalendarItem> {
    if mode == Mode::Live || !example {
        return Vec::new();
    }
    items
        .iter()
        .filter(|i| calendar.is_none_or(|c| i.calendar == c))
        .map(|i| i.item.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Notice, notice, visible};
    use crate::preview::{Item, Mode, items};
    use crate::words::words;
    use fs_components_widgets::Day;
    use fs_components_words::Locale;

    fn list() -> Vec<Item> {
        items(&words(&Locale::english()), Day::new(2026, 11, 18).unwrap())
    }

    #[test]
    fn the_marker_is_there_once_and_only_in_preview() {
        assert_eq!(notice(Mode::Preview, true), Some(Notice::Shown));
        assert_eq!(notice(Mode::Preview, false), Some(Notice::Hidden));
        assert_eq!(notice(Mode::Live, true), None);
        assert_eq!(notice(Mode::Live, false), None);
    }

    #[test]
    fn a_calendar_screen_shows_only_its_own_items() {
        let all = list();
        let work = visible(Mode::Preview, true, &all, Some("work"));
        assert!(!work.is_empty() && work.len() < all.len());
        let everything = visible(Mode::Preview, true, &all, None);
        assert_eq!(everything.len(), all.len());
        assert!(
            work.iter()
                .all(|w| all.iter().any(|a| a.calendar == "work" && a.item == *w))
        );
    }

    #[test]
    fn real_mode_and_a_hidden_example_show_no_example_item() {
        let all = list();
        assert!(visible(Mode::Live, true, &all, None).is_empty());
        assert!(visible(Mode::Preview, false, &all, None).is_empty());
    }
}
