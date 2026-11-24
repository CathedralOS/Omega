//! Generic helpers forward values and loans without gaining discard permission.

#[path = "support/build_fixture.rs"]
mod build_fixture;
mod support;

fn fixture(tier: &str) -> std::path::PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/ownership")
        .join(tier)
        .join("generic_parameter_ownership")
}

#[test]
fn generic_owned_and_borrowed_forwarding_agrees_in_both_engines() {
    build_fixture::assert_silent_return(&fixture("run"));
}

#[test]
fn an_unused_generic_declaration_cannot_discard_its_argument() {
    let (output, report) = build_fixture::check(&fixture("fail"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains(
            "normal edge cannot implicitly dispose storage without an authorized disposition"
        ),
        "{report}"
    );
}
