//! Copying a generic helper preserves its authored invocation ceiling.

#[path = "support/build_fixture.rs"]
mod build_fixture;
mod support;

fn fixture_path(tier: &str) -> std::path::PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/calls")
        .join(tier)
        .join("copied_invocation_contracts")
}

#[test]
fn copied_invocation_contracts_run_in_both_engines() {
    build_fixture::assert_silent_return(&fixture_path("run"));
}

#[test]
fn a_copied_helper_cannot_enter_outside_its_written_ceiling() {
    let (output, report) = build_fixture::check(&fixture_path("fail"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("machine `relay` invokes")
            && report.contains("(synchronous invocation)")
            && !report.contains("not supported yet"),
        "{report}"
    );
}
