//! Generic record construction and forwarding retain each field’s ownership.

#[path = "support/program.rs"]
mod program;
mod support;

fn fixture(tier: &str) -> std::path::PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/ownership")
        .join(tier)
        .join("generic_record_fields")
}

#[test]
fn generic_record_fields_survive_construction_and_calls_when_interpreted_and_built() {
    program::assert_fixture_silent_return(&fixture("run"));
}

#[test]
fn an_unused_generic_record_cannot_discard_its_unknown_field() {
    let (output, report) = program::check_fixture(&fixture("fail"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains(
            "normal edge cannot implicitly dispose storage without an authorized disposition"
        ),
        "{report}"
    );
}
