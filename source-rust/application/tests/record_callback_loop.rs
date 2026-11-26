//! Copied loop bodies retain the exact selected operation.

#[path = "support/program.rs"]
mod program;
mod support;

fn fixture_path(tier: &str) -> std::path::PathBuf {
    support::repository_root()
        .join("tests/fixtures/calls")
        .join(tier)
        .join("record_callback_loop")
}

#[test]
fn record_callback_loop_run_in_both_engines() {
    program::assert_fixture_silent_return(&fixture_path("run"));
}

#[test]
fn a_selected_operation_keeps_its_required_borrow_access() {
    let (output, report) = program::check_fixture(&fixture_path("fail"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("member signature differs from its requirement"),
        "{report}"
    );
}
