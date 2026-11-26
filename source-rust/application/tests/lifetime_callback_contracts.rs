//! Selecting a callback keeps the wrapper's lifetime and entry contract.

#[path = "support/build_fixture.rs"]
mod build_fixture;
mod support;

fn fixture_path(tier: &str) -> std::path::PathBuf {
    support::repository_root()
        .join("tests/fixtures/calls")
        .join(tier)
        .join("lifetime_callback_contracts")
}

#[test]
fn lifetime_callback_contracts_run_in_both_engines() {
    build_fixture::assert_silent_return(&fixture_path("run"));
}

#[test]
fn a_callback_does_not_supply_a_missing_scalar_requirement() {
    let (output, report) = build_fixture::check(&fixture_path("fail"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("cannot prove requires contract for call checked_remaining from forward"),
        "{report}"
    );
}
