//! The first word picks the subcommand (ADR 0063).

use std::process::Command;

fn run(args: &[&str]) -> (Option<i32>, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_fs-calendars"))
        .args(args)
        .output()
        .unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn help_names_all_three_ways_in() {
    let (code, out, _) = run(&["--help"]);
    assert_eq!(code, Some(0));
    for word in ["fs-calendars serve", "fs-calendars app", "list", "import"] {
        assert!(out.contains(word), "{word} missing in {out}");
    }
}

#[test]
fn an_unknown_command_is_refused_with_2() {
    let (code, _, err) = run(&["frob"]);
    assert_eq!(code, Some(2));
    assert!(err.contains("unknown command frob"), "{err}");
}

#[test]
fn serve_help_is_the_servers_own() {
    let (code, out, _) = run(&["serve", "--help"]);
    assert_eq!(code, Some(0));
    assert!(
        out.contains("--socket") && out.contains("--instance"),
        "{out}"
    );
}

#[test]
fn a_command_reaches_a_missing_server_with_exit_1() {
    let dir = std::env::temp_dir().join("fs-calendars-no-such.sock");
    let (code, _, err) = run(&["--socket", dir.to_str().unwrap(), "list"]);
    assert_eq!(code, Some(1));
    assert!(err.contains("fs-calendars:"), "{err}");
}

// A copy of the program in a folder of its own, with or without a frontend next to it. Writing and starting executables
// in several threads gives "text file busy" (ETXTBSY), so the tests of the starter take this lock and close every file first.
static ONE_COPY_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn copy_with(frontend: Option<&str>) -> (tempfile::TempDir, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("fs-calendars");
    std::fs::copy(env!("CARGO_BIN_EXE_fs-calendars"), &exe).unwrap();
    if let Some(script) = frontend {
        let path = dir.path().join("fs-calendars-simple");
        std::fs::write(&path, script).unwrap();
        std::fs::File::open(&path).unwrap().sync_all().unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    (dir, exe)
}

#[test]
fn a_missing_window_is_said_in_german_and_english_and_ends_with_1() {
    let _one = ONE_COPY_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (_dir, exe) = copy_with(None);
    for args in [&["app"][..], &[][..], &["app", "--x"][..]] {
        let out = Command::new(&exe).args(args).output().unwrap();
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        assert!(out.stdout.is_empty());
        let err = String::from_utf8_lossy(&out.stderr);
        for word in ["nicht gefunden", "was not found", "fs-calendars-simple"] {
            assert!(err.contains(word), "{word} missing in {err}");
        }
    }
}

#[test]
fn the_window_is_the_frontend_next_to_the_program_with_the_words_and_its_exit_code() {
    let _one = ONE_COPY_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (_dir, exe) = copy_with(Some("#!/bin/sh\necho \"words: $*\"\nexit 7\n"));
    let out = Command::new(&exe)
        .args(["app", "--view", "week"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(7),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // after `app` the words go on; without a word the frontend gets none
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "words: --view week"
    );
    let bare = Command::new(&exe).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&bare.stdout).trim(), "words:");
}

#[test]
fn version_names_the_program_and_the_workspace_version_and_needs_nothing() {
    for flag in ["--version", "-V"] {
        let (code, out, err) = run(&[flag]);
        assert_eq!(code, Some(0), "{err}");
        assert_eq!(
            out.trim(),
            format!("fs-calendars {}", env!("CARGO_PKG_VERSION"))
        );
    }
    // the version is that of the workspace, in the form 0.0.N (ADR 0064)
    let parts: Vec<&str> = env!("CARGO_PKG_VERSION").split('.').collect();
    assert_eq!((parts.len(), parts[0], parts[1]), (3, "0", "0"));
}
