//! Numeric policy signatures preserve operation meaning in code and contracts.
//! Runtime results cover every value/count policy combination, while the twin
//! changes only the Exact shift's qualified count from 31 to 32.

#[path = "support/declared_exit.rs"]
mod declared_exit;
#[path = "support/project.rs"]
mod project;
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::PathBuf;
use std::process::Output;

fn fixture(tier: &str) -> PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/numeric")
        .join(tier)
        .join("policy_operators")
}

fn invoke(tier: &str, arguments: &[&str]) -> (Output, String) {
    let scratch = TemporaryDirectory::create("policy-operators", false).unwrap();
    for name in ["main.omg", "build.omg"] {
        std::fs::copy(fixture(tier).join(name), scratch.path().join(name)).unwrap();
    }
    let output = project::omega(scratch.path(), &[arguments, &["main.omg"]].concat());
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output, report)
}

#[test]
fn every_shift_policy_pair_and_total_bitwise_operation_runs_when_interpreted_and_built() {
    declared_exit::assert_declared_exit(&fixture("run"), "main.omg");
}

#[test]
fn a_wrapping_count_cannot_relax_an_exact_shift_requirement() {
    let expected = std::fs::read_to_string(fixture("fail").join("expected.txt")).unwrap();
    let (output, report) = invoke("fail", &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains(expected.trim()), "{report}");
}
