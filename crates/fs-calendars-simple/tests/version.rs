//! The frontend binary says its version and its help, and an unknown option ends with 2.
//!
//! Pattern: example-based tests on the real binary (no window, no server: the words are read before anything opens).
//! Tests: version, help, unknown option.

use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_fs-calendars-simple"))
        .args(args)
        .env_clear()
        .output()
        .unwrap()
}

#[test]
fn the_version_is_printed_with_exit_code_0() {
    let out = run(&["--version"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        format!("fs-calendars-simple {}", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(
        String::from_utf8_lossy(&run(&["-V"]).stdout),
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn the_help_exits_with_0_and_an_unknown_option_with_2() {
    let help = run(&["--help"]);
    assert_eq!(help.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&help.stdout).contains("--view month|week|day|agenda"));
    let bad = run(&["--wrong"]);
    assert_eq!(bad.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("unknown option --wrong"));
}
