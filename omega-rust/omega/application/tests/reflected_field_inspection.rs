//! `reflect::inspect_record_fields<Record, callback>(&mut context)` runs the
//! callback once per field of `Record`, in declared order, in both engines.
//! Typed preparation writes those calls after checking the callback against
//! the binder the library publishes; a callback that adds a precondition on
//! its inputs is refused even when every generated call would meet it.
use std::path::Path;
use std::process::{Command, Stdio};

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures")
        .join(name)
}

fn run_both(name: &str) -> (bool, String) {
    let project = fixture(name);
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
fn inspecting_a_record_calls_the_callback_once_per_field_in_both_engines() {
    let (success, report) = run_both("run/reflection/inspect_record_fields");
    assert!(success, "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_callback_with_a_precondition_the_binder_does_not_publish_is_refused() {
    let name = "fail/reflection/inspect_record_fields_input_precondition";
    let expected = std::fs::read_to_string(fixture(name).join("expected.txt")).unwrap();
    let (success, report) = run_both(name);
    assert!(!success, "{report}");
    assert!(report.contains(expected.trim()), "{report}");
    assert!(report.contains("main.omg:21:5"), "{report}");
}
