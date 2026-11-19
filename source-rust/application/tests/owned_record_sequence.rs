//! Owned sequence values preserve their elements across native and Psi calls.
//! Runtime fields, two simultaneously live results, record containment and a
//! shorter live length exercise backing that moves with the aggregate.

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn fixture(tier: &str, name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures")
        .join(tier)
        .join("ownership")
        .join(name)
}

fn invoke(project: &Path, arguments: &[&str]) -> (Output, String) {
    let scratch = TemporaryDirectory::create("owned-sequence", false).unwrap();
    for name in ["main.omg", "build.omg"] {
        std::fs::copy(project.join(name), scratch.path().join(name)).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .arg("main.omg")
        .current_dir(scratch.path())
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

fn assert_both(name: &str) {
    let project = fixture("run", name);
    let expected = std::fs::read_to_string(project.join("expected_native_exit.txt")).unwrap();
    let (output, report) = invoke(&project, &["run", "--both"]);
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
fn an_owned_record_sequence_survives_return_and_by_value_transfer_natively() {
    assert_both("owned_record_sequence");
}

#[test]
fn a_runtime_valued_record_sequence_survives_return_and_by_value_transfer() {
    assert_both("runtime_record_sequence");
}

#[test]
fn runtime_sequence_fields_survive_separate_results_and_containing_records() {
    assert_both("runtime_sequence_values");
}

#[test]
fn an_insufficient_live_length_guard_still_refuses_the_second_element() {
    let project = fixture("fail", "runtime_sequence_values");
    let expected = std::fs::read_to_string(project.join("expected.txt")).unwrap();
    let (output, report) = invoke(&project, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains(expected.trim()), "{report}");
}
