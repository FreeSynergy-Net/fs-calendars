//! The starter of the window: finds the frontend `simple` next to this program and runs it with the same words.
//!
//! Pattern: Strategy (`decide` is a pure function over what was found) + Facade over the process (ADR 0094 draft, item 6: the program
//! starts its frontend as a process of its own; the frontend knows no engine). There is no server for appointments yet, so the starter
//! does not start one: it only opens the window, which shows its example data as a preview.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The file name of the standard frontend.
pub const FRONTEND: &str = "fs-calendars-simple";

/// What the starter does.
#[derive(Debug, PartialEq, Eq)]
pub enum Next {
    /// Run this frontend.
    Run(PathBuf),
    /// Say `message` to the person and end with exit code 1; nothing is run.
    Missing {
        /// What the person is told (German and English).
        message: String,
    },
}

/// `found` is the frontend file next to this program, if there is one.
#[must_use]
pub fn decide(found: Option<PathBuf>) -> Next {
    match found {
        Some(path) => Next::Run(path),
        None => Next::Missing {
            message: format!(
                "fs-calendars: Das Fenster `{FRONTEND}` wurde neben dem Programm nicht gefunden; installiere das Paket `{FRONTEND}`.\n\
                 fs-calendars: The window `{FRONTEND}` was not found next to the program; install the package `{FRONTEND}`."
            ),
        },
    }
}

/// The frontend file in the folder of `exe`, if it is a file there.
#[must_use]
pub fn locate(exe: &Path) -> Option<PathBuf> {
    let path = exe.parent()?.join(FRONTEND);
    path.is_file().then_some(path)
}

/// Open the window with `args` and return the exit code of the frontend (1 when it cannot be run).
#[must_use]
pub fn open(args: &[String]) -> i32 {
    let found = std::env::current_exe().ok().and_then(|exe| locate(&exe));
    match decide(found) {
        Next::Missing { message } => {
            eprintln!("{message}");
            1
        }
        Next::Run(path) => match Command::new(&path).args(args).status() {
            Ok(status) => status.code().unwrap_or(1),
            Err(e) => {
                eprintln!("fs-calendars: {}: {e}", path.display());
                1
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{FRONTEND, Next, decide, locate};
    use std::path::PathBuf;

    #[test]
    fn a_found_frontend_is_run_and_a_missing_one_is_said_in_both_languages() {
        assert_eq!(
            decide(Some(PathBuf::from("/x/y"))),
            Next::Run(PathBuf::from("/x/y"))
        );
        match decide(None) {
            Next::Missing { message } => {
                assert!(
                    message.contains("nicht gefunden") && message.contains("was not found"),
                    "{message}"
                );
                assert!(message.contains(FRONTEND));
            }
            Next::Run(p) => panic!("{p:?}"),
        }
    }

    #[test]
    fn the_frontend_is_looked_for_next_to_the_program_only() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("fs-calendars");
        assert_eq!(locate(&exe), None);
        std::fs::write(dir.path().join(FRONTEND), b"").unwrap();
        assert_eq!(locate(&exe), Some(dir.path().join(FRONTEND)));
    }
}
