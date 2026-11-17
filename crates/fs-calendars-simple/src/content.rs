//! What the window says besides the calendar, as blocks of the shared text view: what works today and what comes.
//!
//! Pattern: Builder: a pure function from the words to blocks, so a test can read the window.

use fs_components_mf2::Mf2Texts;
use fs_components_widgets::{Block, Inline, ListItem};

/// How many entries the lists `start-*` and `coming-*` have; a test checks that every id exists in both languages.
pub const START: usize = 3;
/// See [`START`].
pub const COMING: usize = 7;

fn list(texts: &Mf2Texts, prefix: &str, count: usize) -> Block {
    Block::List {
        ordered: false,
        start: 1,
        items: (1..=count)
            .map(|n| {
                ListItem::new(vec![Block::Paragraph(Inline::plain(
                    texts.plain(&format!("{prefix}-{n}")),
                ))])
            })
            .collect(),
    }
}

fn heading(texts: &Mf2Texts, id: &str) -> Block {
    Block::Heading {
        level: 2,
        text: Inline::plain(texts.plain(id)),
    }
}

/// The start page below the header: what works today.
#[must_use]
pub fn start(texts: &Mf2Texts) -> Vec<Block> {
    vec![heading(texts, "start-title"), list(texts, "start", START)]
}

/// The page "what comes".
#[must_use]
pub fn coming(texts: &Mf2Texts) -> Vec<Block> {
    vec![
        heading(texts, "coming-title"),
        list(texts, "coming", COMING),
    ]
}
