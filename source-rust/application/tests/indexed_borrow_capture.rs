//! Indexed borrows retain the actual byte address through a block edge.

#[path = "support/program.rs"]
mod program;
mod support;

fn fixture(tier: &str) -> std::path::PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/generics")
        .join(tier)
        .join("indexed_borrow_capture")
}

#[test]
fn indexed_borrows_keep_their_original_bytes_when_interpreted_and_built() {
    program::assert_fixture_silent_return(&fixture("run"));
}

#[test]
fn a_shared_indexed_borrow_cannot_become_mutable() {
    let (output, report) = program::check_fixture(&fixture("fail"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("the caller lends only immutable access to this argument and its parameter writes through it"),
        "{report}"
    );
}
