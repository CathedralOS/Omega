//! An unlocked project still compiles and runs natively (packages/locks.md):
//! its program entry comes from the standard library's default entry schema,
//! which entry selection picks as the single supplier without a lock binding.
//! Native production must admit that same schema rather than demand a binding
//! only a lock can supply.

use std::path::Path;
use std::process::{Command, Stdio};

#[test]
fn an_unlocked_std_entry_runs_on_both_engines() {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/run/calls/published_process_exit");
    assert!(
        !project.join("omega.lock").exists(),
        "the fixture must stay unlocked"
    );
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
