//! Nominal extents retain each constant’s declared integer type.

#[path = "support/build_fixture.rs"]
mod build_fixture;
mod support;

fn fixture(tier: &str) -> std::path::PathBuf {
    support::repository_root()
        .join("tests/fixtures/generics")
        .join(tier)
        .join("nominal_const_extent")
}

#[test]
fn nominal_extents_survive_generic_headers_in_both_engines() {
    build_fixture::assert_silent_return(&fixture("run"));
}

#[test]
fn a_const_argument_cannot_change_its_declared_width() {
    let (output, report) = build_fixture::check(&fixture("fail"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("record const argument does not inhabit its declared carrier"),
        "{report}"
    );
}
