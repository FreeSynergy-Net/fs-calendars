//! The command line of `fs-calendars-simple`: a process the starter runs; a person never types it.
//!
//! Pattern: Command: the words become one `Command`, or a message for exit code 2.

use std::path::PathBuf;

use fs_components_widgets::{Day, ViewRange};

use crate::state::range_of;

/// The help text.
pub const HELP: &str = "fs-calendars-simple [--picture FILE.png [--size WIDTHxHEIGHT]] [--screen all|private|work|family|holidays|start|coming] [--view month|week|day|agenda] [--date YYYY-MM-DD] [--empty] [--lang de|en]\n\
The standard frontend `simple` of FreeSynergy Calendars. Start the program with `fs-calendars`, not this one.\n\
  --picture FILE   draw the window to a picture, without opening it\n\
  --size WxH       the size of the picture (default 1000x760)\n\
  --screen ID      the page to show first (default all)\n\
  --view V         the view to open: month (default), week, day or agenda\n\
  --date D         take D as today (a picture that must not change)\n\
  --empty          hide the example (the picture of the empty state)\n\
  --lang de|en     the language (default: the session)\n\
  --version, -V    print the version and exit\n\
  --help, -h       print this help and exit\n\
Exit codes: 0 done, 1 failed, 2 wrong words";

/// What to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Print the version.
    Version,
    /// Print the help.
    Help,
    /// Open the window.
    Window(Options),
}

/// The choices for the window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    /// Draw to this picture instead of opening a window.
    pub picture: Option<(PathBuf, (u32, u32))>,
    /// The id of the screen to show first.
    pub screen: Option<String>,
    /// The view that opens.
    pub view: ViewRange,
    /// The day taken as today.
    pub date: Option<Day>,
    /// The example is hidden.
    pub empty: bool,
    /// The language, if named.
    pub lang: Option<String>,
}

/// The screens a person can ask for.
pub const SCREENS: [&str; 7] = [
    "all", "private", "work", "family", "holidays", "start", "coming",
];

/// Read the words after the program name.
///
/// # Errors
/// A message for the person: an unknown option, a missing or wrong value.
pub fn parse(words: &[String]) -> Result<Command, String> {
    let mut options = Options {
        picture: None,
        screen: None,
        view: ViewRange::Month,
        date: None,
        empty: false,
        lang: None,
    };
    let mut size = (1000, 760);
    let mut it = words.iter();
    while let Some(word) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match word.as_str() {
            "--version" | "-V" => return Ok(Command::Version),
            "--help" | "-h" => return Ok(Command::Help),
            "--empty" => options.empty = true,
            "--picture" => options.picture = Some((PathBuf::from(value("--picture")?), size)),
            "--size" => size = parse_size(&value("--size")?)?,
            "--screen" => {
                let id = value("--screen")?;
                if !SCREENS.contains(&id.as_str()) {
                    return Err(format!("--screen wants {}, not {id}", SCREENS.join(", ")));
                }
                options.screen = Some(if id == "start" { "home".to_owned() } else { id });
            }
            "--view" => {
                let v = value("--view")?;
                options.view = range_of(&v)
                    .ok_or_else(|| format!("--view wants month, week, day or agenda, not {v}"))?;
            }
            "--date" => {
                let d = value("--date")?;
                options.date = Some(
                    Day::parse(&d).ok_or_else(|| format!("--date wants YYYY-MM-DD, not {d}"))?,
                );
            }
            "--lang" => options.lang = Some(value("--lang")?),
            other => return Err(format!("unknown option {other}")),
        }
    }
    // `--size` may come after `--picture`: the picture takes the last size.
    if let Some((_, s)) = options.picture.as_mut() {
        *s = size;
    }
    Ok(Command::Window(options))
}

fn parse_size(text: &str) -> Result<(u32, u32), String> {
    let bad = || format!("--size wants WIDTHxHEIGHT, not {text}");
    let (w, h) = text.split_once('x').ok_or_else(bad)?;
    let (w, h): (u32, u32) = (w.parse().map_err(|_| bad())?, h.parse().map_err(|_| bad())?);
    if (1..=8192).contains(&w) && (1..=8192).contains(&h) {
        Ok((w, h))
    } else {
        Err(bad())
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, parse};
    use fs_components_widgets::{Day, ViewRange};

    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    fn window(text: &str) -> super::Options {
        match parse(&words(text)) {
            Ok(Command::Window(o)) => o,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_known_words_are_read() {
        assert_eq!(parse(&words("--version")), Ok(Command::Version));
        assert_eq!(parse(&words("-h")), Ok(Command::Help));
        let o = window(
            "--picture a.png --size 800x600 --view week --date 2026-11-18 --empty --lang de --screen work",
        );
        assert_eq!(o.picture.map(|p| p.1), Some((800, 600)));
        assert_eq!(
            (o.view, o.date, o.empty),
            (ViewRange::Week, Day::new(2026, 11, 18), true)
        );
        assert_eq!(
            (o.lang.as_deref(), o.screen.as_deref()),
            (Some("de"), Some("work"))
        );
        assert_eq!(window("--screen start").screen.as_deref(), Some("home"));
        assert_eq!(window("").view, ViewRange::Month);
    }

    #[test]
    fn the_size_may_come_after_the_picture() {
        assert_eq!(
            window("--picture a.png --size 640x480")
                .picture
                .map(|p| p.1),
            Some((640, 480))
        );
        assert_eq!(
            window("--size 640x480 --picture a.png")
                .picture
                .map(|p| p.1),
            Some((640, 480))
        );
    }

    #[test]
    fn unknown_words_and_missing_values_are_errors() {
        assert!(
            parse(&words("--nope"))
                .unwrap_err()
                .contains("unknown option --nope")
        );
        assert!(parse(&words("--picture")).is_err());
        assert!(parse(&words("--size 0x5")).is_err());
        assert!(parse(&words("--view year")).is_err());
        assert!(parse(&words("--date 2026-13-40")).is_err());
        assert!(parse(&words("--screen nope")).is_err());
    }
}
