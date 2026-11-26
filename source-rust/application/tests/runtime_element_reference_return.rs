//! A reference to a field of one runtime-selected array element is formed,
//! returned and written through in both engines. The array it came from
//! stays borrowed while that reference is live; the record's other fields
//! do not.

#[path = "support/build_fixture.rs"]
mod build_fixture;
mod support;




#[test]
fn a_runtime_element_field_reference_returns_to_the_caller_in_both_engines() {
    build_fixture::assert_silent_return(&support::repository_root().join(
        "source-rust/application/tests/fixtures/ownership/run/runtime_element_reference_return",
    ));
}

#[test]
fn a_returned_element_reference_leaves_the_other_fields_of_its_argument_free() {
    build_fixture::assert_silent_return(&support::repository_root().join(
        "source-rust/application/tests/fixtures/ownership/run/returned_reference_disjoint_field",
    ));
}

#[test]
fn a_returned_runtime_element_reference_keeps_its_array_borrowed() {
    let fixture = "source-rust/application/tests/fixtures/ownership/fail/runtime_element_reference_overlap";
    let expected = std::fs::read_to_string(
        support::repository_root()
            .join(fixture)
            .join("expected.txt"),
    )
    .unwrap();
    let (output, report) = build_fixture::check(&support::repository_root().join(fixture));
    assert!(!output.status.success(), "{report}");
    assert!(report.contains(expected.trim()), "{report}");
    assert!(report.contains("main.omg:15:19"), "{report}");
}
