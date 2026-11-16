//! `fs-calendar`: the one program of FreeSynergy.Calendar (ADR 0063).
//!
//! Pattern: Command — the first word picks the subcommand and each one is a library of its own:
//! `serve` (`fs-calendar-server`), `app` or no word at all (the window: not available until the port, a message with exit
//! code 2), anything else is a command of the command line (`fs-calendar-cli`).

use std::ffi::OsString;

fn main() {
    let words: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(dispatch(&words));
}

fn dispatch(words: &[String]) -> i32 {
    match words.split_first() {
        // The smoke test of the release workflow runs this: no window, no store, no socket.
        Some((first, _)) if first == "--version" || first == "-V" => {
            println!("fs-calendar {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some((first, rest)) if first == "serve" => {
            fs_calendar_server::serve_command(rest.iter().map(OsString::from))
        }
        Some((first, rest)) if first == "app" => window(rest),
        None => window(&[]),
        Some(_) => fs_calendar_cli::cli_command(words),
    }
}

/// The window is not in this version: it is being ported to the new frontend library (DESIGN.md, "Admin window").
/// Say so in both languages, with the way on, and refuse with the usage exit code 2.
fn window(_args: &[String]) -> i32 {
    eprintln!("{WINDOW_UNAVAILABLE_DE}");
    eprintln!("{WINDOW_UNAVAILABLE_EN}");
    2
}

const WINDOW_UNAVAILABLE_DE: &str = "fs-calendar: Die Kalender-Oberfläche ist in dieser Version noch nicht verfügbar (sie wird gerade auf die neue Oberflächen-Bibliothek umgestellt). Nutze bis dahin die Kommandos `fs-calendar --help` (Terminal) bzw. den Server (`fs-calendar serve`).";
const WINDOW_UNAVAILABLE_EN: &str = "fs-calendar: The calendar window is not available in this version yet (it is being moved to the new frontend library). Until then use the commands of `fs-calendar --help` (terminal) or the server (`fs-calendar serve`).";
