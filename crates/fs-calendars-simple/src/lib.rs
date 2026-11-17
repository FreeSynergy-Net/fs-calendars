//! `fs-calendars-simple`: the standard frontend `simple` of FreeSynergy Calendars, as a process of its own.
//!
//! It opens a window with the calendars on the left and, on the right, the month, the week, the day or the agenda, in the look of GNOME
//! Calendar, Thunderbird and Nextcloud Calendar, built from the blocks of fs-components. No server speaks the contract for appointments
//! yet, so the window shows example data and says so once at the top (a preview); the core exists as a library and is not wired to this
//! window. Every element that does not work yet is locked with "coming soon" (ADR 0066).
//!
//! Pattern: Facade: this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod bar;
mod cli;
mod content;
mod context;
mod filter;
mod labels;
mod main_view;
mod names;
mod preview;
mod run;
mod state;
mod window;
mod words;

pub use cli::{Command, HELP, Options, SCREENS, parse};
pub use content::{COMING, START, coming, start};
pub use context::{KIND, WORDS, commands, items as context_items};
pub use filter::{Notice, notice, visible};
pub use main_view::Main;
pub use names::{german, names, title};
pub use preview::{
    CALENDARS, CAPABILITY, Calendar, ENTRIES, Entry, ID_PREFIX, Item, Mode, items, mode,
};
pub use run::{clock_now, clock_of, run};
pub use state::{RANGES, Shared, State, Step, range_key, range_of};
pub use window::app;
pub use words::{TEXTS, session_locale, words};
