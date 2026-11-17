//! The main view and its example data: the example lives only in `preview.rs`, is switched off as soon as the server offers real
//! data, carries the "Preview" marker while it is shown, and the calendars of the navigation narrow it.
//!
//! Pattern: example-based tests over the public API, plus one grep over the sources of this crate.
//! Tests: preview mode, live mode, marker, calendars, state shared by the screens, example only in `preview.rs`.

use std::path::Path;

use fs_calendars_simple::{CALENDARS, ENTRIES, ID_PREFIX, Main, Mode, Notice, words};
use fs_components_widgets::{Day, Moment, ViewRange};
use fs_components_words::Locale;

fn main_view(caps: &[&str], example: bool) -> Main {
    let caps: Vec<String> = caps.iter().map(|s| (*s).to_owned()).collect();
    let today = Day::new(2026, 11, 18).unwrap();
    Main::new(
        &words(&Locale::english()),
        &caps,
        (today, Moment::at(today, 10, 30)),
        example,
        ViewRange::Month,
    )
}

#[test]
fn without_the_capability_the_example_shows_with_the_marker() {
    let view = main_view(&[], true);
    assert_eq!(view.mode(), Mode::Preview);
    assert_eq!(view.notice(), Some(Notice::Shown));
    assert_eq!(view.items().len(), ENTRIES.len());
    assert!(view.items().iter().all(|i| i.key.starts_with(ID_PREFIX)));
}

#[test]
fn with_the_capability_the_example_is_gone_and_so_is_the_marker() {
    let view = main_view(&["appointments"], true);
    assert_eq!(view.mode(), Mode::Live);
    assert_eq!(view.notice(), None);
    assert_eq!(view.items().len(), 0);
}

#[test]
fn hiding_the_example_shows_the_empty_state_and_the_marker_stays() {
    let view = main_view(&[], true);
    view.shared().lock().example = false;
    assert_eq!(view.items().len(), 0);
    assert_eq!(view.notice(), Some(Notice::Hidden));
    view.shared().lock().example = true;
    assert_eq!(view.items().len(), ENTRIES.len());
}

#[test]
fn every_calendar_screen_shows_its_own_items_and_together_they_are_all() {
    let all = main_view(&[], true);
    let mut sum = 0;
    for c in CALENDARS {
        let screen = all.sibling(Some(c.id));
        let n = screen.items().len();
        assert!(n > 0, "{} has no example item", c.id);
        assert!(n < ENTRIES.len());
        sum += n;
    }
    assert_eq!(
        sum,
        ENTRIES.len(),
        "every entry belongs to exactly one calendar"
    );
}

#[test]
fn the_screens_share_the_range_and_the_day() {
    let all = main_view(&[], true);
    let work = all.sibling(Some("work"));
    work.shared().lock().range = ViewRange::Week;
    assert_eq!(all.shared().snapshot().range, ViewRange::Week);
    assert_eq!(all.title(), "16\u{2013}22 November 2026");
}

#[test]
fn the_agenda_title_names_its_first_day_and_the_month_title_the_month() {
    let view = main_view(&[], true);
    assert_eq!(view.title(), "November 2026");
    view.shared().lock().range = ViewRange::Agenda;
    assert_eq!(view.title(), "from 18 November 2026");
}

#[test]
fn example_data_stand_only_in_preview_rs() {
    let id = ["preview", ":"].concat();
    let name = ["Example", " A"].concat();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut checked = 0;
    for entry in std::fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "rs") || path.ends_with("preview.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        // An example id is the prefix and a number ("preview::" in a `use` line is not one).
        let has_id = text
            .match_indices(&id)
            .any(|(at, _)| text[at + id.len()..].starts_with(|c: char| c.is_ascii_digit()));
        assert!(!has_id, "{} holds example ids", path.display());
        assert!(
            !text.contains(&name),
            "{} holds example names",
            path.display()
        );
        checked += 1;
    }
    assert!(checked >= 10, "only {checked} sources looked at");
}
