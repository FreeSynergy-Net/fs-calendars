//! The words the main view needs, read once from the texts.
//!
//! Pattern: Value object — a plain struct of sentences, so building a screen asks no texts again.

use fs_components_mf2::Mf2Texts;

/// The sentences of the main view.
#[derive(Clone, Debug)]
pub struct Labels {
    pub(crate) previous: String,
    pub(crate) today: String,
    pub(crate) next: String,
    pub(crate) month: String,
    pub(crate) week: String,
    pub(crate) day: String,
    pub(crate) agenda: String,
    pub(crate) calendars: Vec<(u8, String)>,
    pub(crate) agenda_from: String,
    pub(crate) preview_title: String,
    pub(crate) preview_text: String,
    pub(crate) preview_hidden: String,
    pub(crate) hide: String,
    pub(crate) show: String,
    pub(crate) empty_title: String,
    pub(crate) empty_text: String,
}

impl Labels {
    /// Read the sentences from `texts`.
    #[must_use]
    pub fn read(texts: &Mf2Texts) -> Self {
        let p = |id: &str| texts.plain(id);
        Self {
            previous: p("nav-previous"),
            today: p("nav-today"),
            next: p("nav-next"),
            month: p("range-month"),
            week: p("range-week"),
            day: p("range-day"),
            agenda: p("range-agenda"),
            calendars: crate::preview::CALENDARS
                .iter()
                .map(|c| (c.category, p(c.word)))
                .collect(),
            agenda_from: p("agenda-from"),
            preview_title: p("preview-title"),
            preview_text: p("preview-text"),
            preview_hidden: p("preview-hidden-text"),
            hide: p("preview-hide"),
            show: p("preview-show"),
            empty_title: p("empty-title"),
            empty_text: p("empty-text"),
        }
    }
}
