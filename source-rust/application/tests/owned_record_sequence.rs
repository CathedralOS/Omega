//! Owned sequence values preserve their elements across native and Psi calls.
//! Runtime fields, two simultaneously live results, record containment and a
//! shorter live length exercise backing that moves with the aggregate.

#[path = "support/declared_exit.rs"]
mod declared_exit;
#[path = "support/project.rs"]
mod project;
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::{Path, PathBuf};
use std::process::Output;

fn fixture(tier: &str, name: &str) -> PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/ownership")
        .join(tier)
        .join(name)
}

fn invoke(project: &Path, arguments: &[&str]) -> (Output, String) {
    let scratch = TemporaryDirectory::create("owned-sequence", false).unwrap();
    for name in ["main.omg", "build.omg"] {
        std::fs::copy(project.join(name), scratch.path().join(name)).unwrap();
    }
    let output = project::omega(scratch.path(), &[arguments, &["main.omg"]].concat());
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    (output, report)
}

fn assert_both(name: &str) {
    let project = fixture("run", name);
    declared_exit::assert_declared_exit(&project, "main.omg");
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
fn a_forwarded_borrowed_slice_and_an_owned_sequence_keep_their_own_layouts() {
    assert_both("forwarded_typed_slice");
}

#[test]
fn a_borrowed_slice_yields_an_owned_copy_of_the_selected_record() {
    assert_both("copied_borrowed_record");
}

#[test]
fn a_record_retains_the_length_and_elements_of_a_reborrowed_slice() {
    assert_both("reborrowed_record_slice");
}

/// A `@'a mutable [u32]` formal moved into a local keeps the formal's
/// pointer-and-count view natively: a write through the local reaches only the
/// selected element of the caller's array, and a local a `match` joins from
/// two such formals keeps the length of the view that arrived.
#[test]
fn a_mutable_slice_formal_reborrowed_into_a_local_writes_the_callers_element() {
    assert_both("reborrowed_mutable_slice_local");
}

#[test]
fn a_nested_state_writes_only_the_selected_borrowed_slice_element() {
    assert_both("indexed_borrowed_element_store");
}

#[test]
fn recursive_transitions_forward_the_slice_and_return_the_matched_element() {
    assert_both("recursive_typed_slice");
}

#[test]
fn an_insufficient_live_length_guard_still_refuses_the_second_element() {
    let project = fixture("fail", "runtime_sequence_values");
    let expected = std::fs::read_to_string(project.join("expected.txt")).unwrap();
    let (output, report) = invoke(&project, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains(expected.trim()), "{report}");
}
