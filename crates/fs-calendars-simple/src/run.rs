//! Running a command: the exit codes of the frontend (0 done, 1 failed, 2 wrong words).
//!
//! Pattern: Facade over the window and the shell.

use std::process::ExitCode;

use chrono::{Datelike, Local, Timelike};
use fs_components_app::{WindowOptions, launch};
use fs_components_widgets::{Day, Moment};
use fs_components_words::Locale;

use crate::cli::{Command, HELP, Options, parse};
use crate::window::app;
use crate::words::{session_locale, words};

/// Today and now by the clock of this computer: the first is the day, the second the day with the minute.
#[must_use]
pub fn clock_now() -> (Day, Moment) {
    let now = Local::now();
    let day = Day::new(
        now.year(),
        u8::try_from(now.month()).unwrap_or(1),
        u8::try_from(now.day()).unwrap_or(1),
    )
    .unwrap_or_else(|| Day::from_number(0));
    let minute = u16::try_from(now.hour() * 60 + now.minute()).unwrap_or(0);
    (day, Moment::at(day, minute / 60, minute % 60))
}

/// The clock of the window: the clock of the computer, or 10:30 of the day a picture must show.
#[must_use]
pub fn clock_of(date: Option<Day>) -> (Day, Moment) {
    date.map_or_else(clock_now, |day| (day, Moment::at(day, 10, 30)))
}

/// Run the command line (the words after the program name) and return the exit code.
#[must_use]
pub fn run(args: &[String]) -> ExitCode {
    match parse(args) {
        Err(message) => {
            eprintln!("fs-calendars-simple: {message}\nTry `fs-calendars-simple --help`.");
            ExitCode::from(2)
        }
        Ok(Command::Version) => {
            println!("fs-calendars-simple {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(Command::Help) => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
        Ok(Command::Window(options)) => window(&options),
    }
}

fn locale_of(options: &Options) -> Locale {
    options
        .lang
        .as_deref()
        .and_then(|l| Locale::parse(l).ok())
        .unwrap_or_else(session_locale)
}

fn window(options: &Options) -> ExitCode {
    // No server speaks appointments yet: the window shows the example and says so (the capability list is empty).
    let widget = match app(
        &words(&locale_of(options)),
        &[],
        clock_of(options.date),
        !options.empty,
        options.view,
    ) {
        Ok(widget) => widget,
        Err(e) => {
            eprintln!("fs-calendars-simple: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Some(id) = &options.screen {
        widget.select(id);
    }
    let done = match &options.picture {
        Some((path, (w, h))) => widget.picture(*w, *h, path),
        None => launch(WindowOptions::default(), widget),
    };
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("fs-calendars-simple: {e}");
            ExitCode::FAILURE
        }
    }
}
