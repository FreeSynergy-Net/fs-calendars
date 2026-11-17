//! The frontend knows no engine: no way leads from its normal dependencies to the core, the local provider or the server of the
//! program (ADR 0094 draft, item 6).
//!
//! Pattern: example-based, reads `cargo tree` of the real manifest.
//! Tests: forbidden crates are not in the tree, fs-components is.

use std::process::Command;

#[test]
fn no_normal_dependency_leads_to_the_core_the_local_provider_or_the_server() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let out = Command::new(cargo)
        .args([
            "tree",
            "-p",
            "fs-calendars-simple",
            "-e",
            "normal",
            "--prefix",
            "none",
            "--locked",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let tree = String::from_utf8_lossy(&out.stdout);
    assert!(
        tree.contains("fs-components-widgets"),
        "fs-components must be there: {tree}"
    );
    for forbidden in [
        "fs-calendars-core",
        "fs-calendars-local",
        "fs-calendars-server",
        "fs-objects",
    ] {
        assert!(
            !tree.lines().any(|l| l.starts_with(forbidden)),
            "{forbidden} is a dependency"
        );
    }
}
