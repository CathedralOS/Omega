//! A value `match` whose arms each borrow an incoming record forms one loan
//! per arm, and the joined reference keeps every one of those loans live.
//! A call can borrow the joined reference whether the arms name one incoming
//! record or two. Because each arm's loan stays live, a
//! write through a lender while the joined reference is live is refused,
//! even the lender of the arm the program does not take.

#[path = "support/program.rs"]
mod program;
mod support;

fn assert_runs(fixture: &str) {
    program::assert_fixture_silent_return(&support::repository_root().join(fixture));
}

#[test]
fn a_match_of_one_incoming_reference_passes_to_a_borrowing_call() {
    assert_runs("source-rust/application/tests/fixtures/ownership/run/incoming_reference_match");
}

#[test]
fn a_match_of_two_incoming_references_passes_to_a_borrowing_call() {
    assert_runs(
        "source-rust/application/tests/fixtures/ownership/run/incoming_reference_match_two_lenders",
    );
}

#[test]
fn a_write_to_the_untaken_lender_while_the_match_is_live_is_refused() {
    let fixture = "source-rust/application/tests/fixtures/ownership/fail/incoming_reference_match_two_lenders_write";
    let expected = std::fs::read_to_string(
        support::repository_root()
            .join(fixture)
            .join("expected.txt"),
    )
    .unwrap();
    let (output, report) = program::check_fixture(&support::repository_root().join(fixture));
    assert!(!output.status.success(), "{report}");
    assert!(report.contains(expected.trim()), "{report}");
}
