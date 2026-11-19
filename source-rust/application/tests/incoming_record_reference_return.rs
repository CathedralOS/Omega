use std::path::Path;
use std::process::{Command, Stdio};

#[test]
fn a_shared_record_reference_returns_to_the_callers_original_record() {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/run/ownership/incoming_record_reference_return");
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
