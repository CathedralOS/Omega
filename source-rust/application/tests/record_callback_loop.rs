//! Copied loop bodies retain the exact selected operation.

#[path = "support/build_fixture.rs"]
mod build_fixture;
mod support;

fn fixture_path(tier: &str) -> std::path::PathBuf {
    support::repository_root()
        .join("tests/fixtures/calls")
        .join(tier)
        .join("record_callback_loop")
}

#[test]
fn record_callback_loop_run_in_both_engines() {
    build_fixture::assert_silent_return(&fixture_path("run"));
}

#[test]
fn a_selected_operation_keeps_its_required_borrow_access() {
    let (output, report) = build_fixture::check(&fixture_path("fail"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("member signature differs from its requirement"),
        "{report}"
    );
}
