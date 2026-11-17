//! The context menu of the calendar (right click, menu key, Shift+F10): the commands, their arrangement and the words.
//!
//! Pattern: Builder over the command model of fs-components (`Commands` + `Arrangement`, resolved once): the title-bar menu and this
//! menu would read the same commands. Every command that needs a server that does not exist yet is locked WITH its reason (never
//! greyed out silently); only "Today" works.

use std::collections::HashMap;
use std::sync::Arc;

use fs_components_app::commands::{
    Arrangement, Command, Commands, MenuItem, Resolved, TextKey, resolve,
};
use fs_components_mf2::Mf2Texts;

/// The kind of target the menu acts on (the arrangement is `context:calendar`).
pub const KIND: &str = "calendar";

/// The ids of the texts the menu uses; a test checks that they exist in both languages.
pub const WORDS: [&str; 5] = ["ctx-new", "ctx-edit", "ctx-delete", "ctx-today", "ctx-why"];

/// The commands of the calendar view.
#[must_use]
pub fn commands() -> Commands {
    Commands::new()
        .with(
            Command::new("new", "ctx-new")
                .icon("list-add")
                .unsupported("ctx-why"),
        )
        .with(
            Command::new("edit", "ctx-edit")
                .icon("document-edit")
                .unsupported("ctx-why"),
        )
        .with(
            Command::new("delete", "ctx-delete")
                .icon("edit-delete")
                .unsupported("ctx-why"),
        )
        .with(Command::new("today", "ctx-today").icon("go-today"))
}

/// The resolved lines of the menu: New, Edit, Delete, a separator, Today.
///
#[must_use]
pub fn items() -> Vec<Resolved> {
    let tree = Arrangement::new(
        "context:calendar",
        vec![
            MenuItem::command("new"),
            MenuItem::command("edit"),
            MenuItem::command("delete"),
            MenuItem::Separator,
            MenuItem::command("today"),
        ],
    );
    resolve(&tree, &commands()).unwrap_or_default()
}

/// The words of [`WORDS`] in the language of `texts`, by id.
#[must_use]
pub fn words(texts: &Mf2Texts) -> Arc<HashMap<String, String>> {
    Arc::new(
        WORDS
            .iter()
            .map(|id| ((*id).to_owned(), texts.plain(id)))
            .collect(),
    )
}

/// The text of a key of the model: the word, or the key itself so a missing word is seen.
#[must_use]
pub fn say(words: &HashMap<String, String>, key: &TextKey) -> String {
    words
        .get(key.as_str())
        .cloned()
        .unwrap_or_else(|| key.as_str().to_owned())
}

#[cfg(test)]
mod tests {
    use super::{items, say, words};
    use crate::words::words as texts;
    use fs_components_app::commands::TextKey;
    use fs_components_words::Locale;

    #[test]
    fn the_fixed_arrangement_resolves_to_five_lines() {
        assert_eq!(items().len(), 5);
    }

    #[test]
    fn a_known_key_is_the_word_and_an_unknown_key_shows_itself() {
        let w = words(&texts(&Locale::english()));
        assert_eq!(say(&w, &TextKey::new("ctx-today")), "Go to today");
        assert_eq!(say(&w, &TextKey::new("nope")), "nope");
    }
}
