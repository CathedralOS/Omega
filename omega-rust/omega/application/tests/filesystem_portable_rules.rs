//! The portable filesystem calls must mean the same thing on every target.
//! Before this test, the POSIX adapter emptied a file for `open_with` with
//! truncate and no write access, clamped oversized read counts, stripped
//! execute bits on `Permissions::Default`, skipped long parent prefixes in
//! `create_dir_all` and stopped `remove_dir_all` after 4096 steps. Windows
//! refused or completed each of these. The fixture exits with the number of
//! the first rule that breaks, so the Psi interpreter and the native program
//! must both reach exit 42.

use std::path::Path;
use std::process::{Command, Stdio};

#[test]
fn posix_filesystem_follows_the_portable_rules() {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/run/filesystem/portable_rules");
    let expected = std::fs::read_to_string(project.join("expected_native_exit.txt")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--both", "main.omg"])
        .current_dir(&project)
        .stdin(Stdio::null())
        .output()
        .expect("omega runs on the hosted target");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(output.status.success(), "{report}");
    let code = expected.trim();
    for receipt in [
        format!("native exit {code}"),
        format!("interp exit: {code}"),
    ] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}
