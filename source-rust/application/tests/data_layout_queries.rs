//! `size_of<T>()` and `align_of<T>()` report the layout the selected policy
//! laid out for the target (layouts/plans.md, "Queries consume the same exact
//! schema, target and layout-policy application as placement and ABI").
//! The positive runs in native code and in the Psi interpreter; its twin
//! differs only in the queried record, which the C policy cannot lay out, and
//! must be refused rather than receive a fabricated measurement.

#[path = "support/declared_exit.rs"]
mod declared_exit;
#[path = "support/project.rs"]
mod project;
mod support;

use std::path::{Path, PathBuf};
use std::process::Output;

fn fixture(path: &str) -> PathBuf {
    support::repository_root().join(path)
}

fn check(directory: &Path) -> (Output, String) {
    let output = project::omega(directory, &["--check", "main.omg"]);
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    (output, report)
}

#[test]
fn layout_queries_observe_the_selected_c_layout_in_native_and_psi() {
    let project =
        fixture("source-rust/application/tests/fixtures/layouts/run/c_layout_size_alignment_query");
    declared_exit::assert_declared_exit(&project, "main.omg");
}

#[test]
fn a_layout_query_of_a_record_the_policy_cannot_lay_out_is_refused() {
    let project = fixture(
        "source-rust/application/tests/fixtures/layouts/fail/c_layout_unsupported_size_query",
    );
    let (output, report) = check(&project);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("layout policy for `Nested`")
            && report.contains("does not retain its complete 64-entry carrier"),
        "{report}"
    );
}
