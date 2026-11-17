//! The pages of the window besides the calendar: every sentence exists in both languages, the lists have their count, and what is
//! not there yet is said so.
//!
//! Pattern: example-based tests over the public API.
//! Tests: ids in both languages, counts, honest sentence about the missing server.

use fs_calendars_simple::{COMING, START, TEXTS, coming, start, words};
use fs_components_mf2::check_pairs;
use fs_components_widgets::Block;
use fs_components_words::Locale;

#[test]
fn both_languages_have_the_same_ids_and_variables() {
    assert!(check_pairs(TEXTS).is_empty(), "{:?}", check_pairs(TEXTS));
}

#[test]
fn every_list_entry_and_name_exists_in_both_languages() {
    for locale in [Locale::english(), Locale::parse("de").unwrap()] {
        let texts = words(&locale);
        let mut ids: Vec<String> = (1..=START).map(|n| format!("start-{n}")).collect();
        ids.extend((1..=COMING).map(|n| format!("coming-{n}")));
        ids.extend((1..=7).map(|n| format!("weekday-{n}")));
        ids.extend((1..=12).map(|n| format!("month-{n}")));
        ids.extend((1..=16).map(|n| format!("example-{n}")));
        ids.extend(fs_calendars_simple::WORDS.iter().map(|w| (*w).to_owned()));
        for id in ids {
            let said = texts.plain(&id);
            assert!(!said.is_empty() && said != id, "{id} is missing: {said:?}");
        }
    }
}

#[test]
fn the_pages_have_a_heading_and_a_list_of_the_counted_length() {
    let texts = words(&Locale::english());
    for (blocks, count) in [(start(&texts), START), (coming(&texts), COMING)] {
        assert!(matches!(blocks[0], Block::Heading { level: 2, .. }));
        match &blocks[1] {
            Block::List { items, .. } => assert_eq!(items.len(), count),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn the_start_page_says_plainly_that_no_real_appointment_is_shown() {
    let en = words(&Locale::english()).plain("start-3");
    let de = words(&Locale::parse("de").unwrap()).plain("start-3");
    assert!(en.contains("no real appointment"), "{en}");
    assert!(de.contains("keinen echten Termin"), "{de}");
}
