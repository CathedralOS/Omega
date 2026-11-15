//! `size_of<T>()` and `align_of<T>()` report the layout the selected policy
//! laid out for the target (layouts/plans.md, "Queries consume the same exact
//! schema, target and layout-policy application as placement and ABI").
//! The positive runs in native code and in the Psi interpreter; its twin
//! differs only in the queried record, which the C policy cannot lay out, and
//! must be refused rather than receive a fabricated measurement.
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn fixture(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures")
        .join(path)
}

fn run_both(project: &Path) -> (Output, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--both", "main.omg"])
        .current_dir(project)
        .stdin(Stdio::null())
        .output()
        .expect("omega runs on the hosted target");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    (output, report)
}

#[test]
fn layout_queries_observe_the_selected_c_layout_in_native_and_psi() {
    let project = fixture("run/layouts/c_layout_size_alignment_query");
    let expected = std::fs::read_to_string(project.join("expected_native_exit.txt")).unwrap();
    let (output, report) = run_both(&project);
    assert!(output.status.success(), "{report}");
    let code = expected.trim();
    for receipt in [
        format!("native exit {code}"),
        format!("interp exit: {code}"),
    ] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_layout_query_of_a_record_the_policy_cannot_lay_out_is_refused() {
    let project = fixture("fail/layouts/c_layout_unsupported_size_query");
    let (output, report) = run_both(&project);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("layout policy for `Nested`")
            && report.contains("does not retain its complete 64-entry carrier"),
        "{report}"
    );
}
