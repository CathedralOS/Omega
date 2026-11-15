//! A reference to a field of one runtime-selected array element is formed,
//! returned and written through in both engines, and the array it came from
//! stays borrowed while that reference is live.
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
fn a_runtime_element_field_reference_returns_to_the_caller_in_both_engines() {
    let (success, report) = run_both("run/ownership/runtime_element_reference_return");
    assert!(success, "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_returned_runtime_element_reference_keeps_its_array_borrowed() {
    let fixture = "fail/ownership/runtime_element_reference_overlap";
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
    assert!(report.contains("main.omg:15:19"), "{report}");
}
