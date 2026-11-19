//! Numeric policy signatures preserve operation meaning in code and contracts.
//! Runtime results cover every value/count policy combination, while the twin
//! changes only the Exact shift's qualified count from 31 to 32.

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn fixture(tier: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures")
        .join(tier)
        .join("numeric/policy_operators")
}

fn invoke(tier: &str, arguments: &[&str]) -> (Output, String) {
    let scratch = TemporaryDirectory::create("policy-operators", false).unwrap();
    for name in ["main.omg", "build.omg"] {
        std::fs::copy(fixture(tier).join(name), scratch.path().join(name)).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .arg("main.omg")
        .current_dir(scratch.path())
        .stdin(Stdio::null())
        .output()
        .expect("run numeric policy program");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output, report)
}

#[test]
fn every_shift_policy_pair_and_total_bitwise_operation_runs_in_both_engines() {
    let expected =
        std::fs::read_to_string(fixture("run").join("expected_native_exit.txt")).unwrap();
    let (output, report) = invoke("run", &["run", "--both"]);
    assert!(output.status.success(), "{report}");
    for receipt in [
        format!("native exit {}", expected.trim()),
        format!("interp exit: {}", expected.trim()),
    ] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_wrapping_count_cannot_relax_an_exact_shift_requirement() {
    let expected = std::fs::read_to_string(fixture("fail").join("expected.txt")).unwrap();
    let (output, report) = invoke("fail", &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains(expected.trim()), "{report}");
}
