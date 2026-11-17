//! `fs-calendars`: the one program of FreeSynergy.Calendars (ADR 0063).
//!
//! Pattern: Command — the first word picks the subcommand and each one is a library of its own:
//! `serve` (`fs-calendars-server`), `app` or no word at all (the window: the frontend `simple`, started as a process of its own), anything else is a command of the command line (`fs-calendars-cli`).

use std::ffi::OsString;

mod starter;

fn main() {
    let words: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(dispatch(&words));
}

fn dispatch(words: &[String]) -> i32 {
    match words.split_first() {
        // The smoke test of the release workflow runs this: no window, no store, no socket.
        Some((first, _)) if first == "--version" || first == "-V" => {
            println!("fs-calendars {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some((first, rest)) if first == "serve" => {
            fs_calendars_server::serve_command(rest.iter().map(OsString::from))
        }
        Some((first, rest)) if first == "app" => window(rest),
        None => window(&[]),
        Some(_) => fs_calendars_cli::cli_command(words),
    }
}

/// The window: the frontend `simple` next to this program (see `starter`).
fn window(args: &[String]) -> i32 {
    starter::open(args)
}
