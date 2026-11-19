//! A value `match` whose arms each borrow the same incoming record forms one
//! loan per arm. The joined reference still borrows the caller's record, so
//! a call can borrow it in both engines. Arms that borrow two different
//! inputs have no single lender and stay refused.
use std::path::Path;
use std::process::{Command, Stdio};

fn run_both(fixture: &str) -> (bool, String) {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures")
        .join(fixture);
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
    let _ = std::fs::remove_dir_all(project.join("build"));
    let _ = std::fs::remove_file(project.join("omega.lock"));
    (output.status.success(), report)
}

#[test]
fn a_match_of_one_incoming_reference_passes_to_a_borrowing_call() {
    let (success, report) = run_both("run/ownership/incoming_reference_match");
    assert!(success, "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_match_of_two_incoming_references_has_no_single_lender() {
    let fixture = "fail/ownership/incoming_reference_match_two_lenders";
    let expected = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures")
            .join(fixture)
            .join("expected.txt"),
    )
    .unwrap();
    let (success, report) = run_both(fixture);
    assert!(!success, "{report}");
    assert!(report.contains(expected.trim()), "{report}");
}
