//! Static applications retain layout queries and their entry contracts.

#[path = "support/build_fixture.rs"]
mod build_fixture;
mod support;

fn fixture_path(tier: &str) -> std::path::PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/layout")
        .join(tier)
        .join("declaration_copy_queries")
}

#[test]
fn copied_size_and_alignment_queries_agree_in_both_engines() {
    build_fixture::assert_silent_return(&fixture_path("run"));
}

#[test]
fn copying_a_layout_query_does_not_remove_the_entry_contract() {
    let (output, report) = build_fixture::check(&fixture_path("fail"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("requires") && report.contains("measure"),
        "{report}"
    );
}
