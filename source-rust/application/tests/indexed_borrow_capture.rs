//! Indexed borrows retain the actual byte address through a block edge.

#[path = "support/build_fixture.rs"]
mod build_fixture;
mod support;

fn fixture(tier: &str) -> std::path::PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/generics")
        .join(tier)
        .join("indexed_borrow_capture")
}

#[test]
fn indexed_borrows_keep_their_original_bytes_in_both_engines() {
    build_fixture::assert_silent_return(&fixture("run"));
}

#[test]
fn a_shared_indexed_borrow_cannot_become_mutable() {
    let (output, report) = build_fixture::check(&fixture("fail"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("the caller lends only immutable access to this argument and its parameter writes through it"),
        "{report}"
    );
}
