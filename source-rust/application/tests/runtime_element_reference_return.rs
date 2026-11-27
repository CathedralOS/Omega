//! A machine returns a reference to a field of a runtime-selected array
//! element, and its caller writes through it. The array it came from
//! stays borrowed while that reference is live; the record's other fields
//! do not.

#[path = "support/program.rs"]
mod program;
mod support;

#[test]
fn a_runtime_element_field_reference_returns_to_the_caller_when_interpreted_and_built() {
    program::assert_fixture_silent_return(&support::repository_root().join(
        "source-rust/application/tests/fixtures/ownership/run/runtime_element_reference_return",
    ));
}

#[test]
fn a_returned_element_reference_leaves_the_other_fields_of_its_argument_free() {
    program::assert_fixture_silent_return(&support::repository_root().join(
        "source-rust/application/tests/fixtures/ownership/run/returned_reference_disjoint_field",
    ));
}

#[test]
fn a_returned_runtime_element_reference_keeps_its_array_borrowed() {
    let fixture =
        "source-rust/application/tests/fixtures/ownership/fail/runtime_element_reference_overlap";
    let expected = std::fs::read_to_string(
        support::repository_root()
            .join(fixture)
            .join("expected.txt"),
    )
    .unwrap();
    let (output, report) = program::check_fixture(&support::repository_root().join(fixture));
    assert!(!output.status.success(), "{report}");
    assert!(report.contains(expected.trim()), "{report}");
    assert!(report.contains("main.omg:15:19"), "{report}");
}
