//! `size_of<T>()` and `align_of<T>()` report the layout the selected policy
//! laid out for the target (layouts/plans.md, "Queries consume the same exact
//! schema, target and layout-policy application as placement and ABI").
//! The value form observes the borrowed location and can appear in the
//! guarantee on the machine that returns its alignment.

#[path = "support/declared_exit.rs"]
mod declared_exit;
#[path = "support/program.rs"]
mod program;
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

    let scratch = platform_custody::temporary_directory::TemporaryDirectory::create(
        "saved-c-layout-query", false,
    ).unwrap();
    let saved = scratch.path().join("query.psi");
    let output = project::omega(
        &project,
        &["--emit-psi", saved.to_str().unwrap(), "--target", "linux_x86_64", "build.omg"],
    );
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let artifact = std::fs::read(saved).unwrap();
    let _reconstructed = terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact)
        .expect("the saved schema reconstructs the target's address field geometry");
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

#[test]
fn a_call_returns_the_borrowed_locations_alignment_in_native_and_psi() {
    let project =
        fixture("source-rust/application/tests/fixtures/layouts/run/borrowed_alignment_contract");
    program::assert_fixture_silent_return(&project);
}

#[test]
fn a_literal_cannot_stand_in_for_a_borrowed_locations_alignment() {
    let project =
        fixture("source-rust/application/tests/fixtures/layouts/fail/borrowed_alignment_contract");
    let (output, report) = program::check_fixture(&project);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("cannot prove ensures contract for exit from alignment"),
        "{report}"
    );
}
