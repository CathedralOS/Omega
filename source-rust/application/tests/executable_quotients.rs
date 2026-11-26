//! A quotient of plain copyable data now runs: `Representative { value: 11 }
//! as EquivalenceClass` stores the class as its representative, and the
//! program reads it only through `Quotient::lift`, which calls the selected
//! representative operation. Before, the ordinary reader refused quotient data
//! outright, so the nineteen programs under
//! `samples/cli/facts/quotient_closed_literal/` could not compile.
//!
//! `wiki/spec/proofs/quotients.md`: a class is formed by `as`, eliminated
//! only by `Quotient::lift` or `Quotient::define`, and each elimination is
//! judged by its selected congruence theorem.
//!
//! The run program picks one of two classes at a join and lifts it with the
//! constant `3i32`. Each twin changes one thing and is refused: a constant
//! that misses `representative`'s precondition, a congruence theorem that
//! relates the left representative to itself, and a carrier that is not
//! copyable.

#[path = "support/program.rs"]
mod program;
mod support;

use program::invoke;

fn fixture(tier: &str, case: &str) -> String {
    let path = support::repository_root()
        .join("source-rust/application/tests/fixtures/quotients")
        .join(tier)
        .join(case)
        .join("main.omg");
    std::fs::read_to_string(path).unwrap()
}

/// The run program with `from` replaced by `to` is refused by `--check`
/// with each `expected` line.
fn refused(from: &str, to: &str, expected: &[&str]) {
    let run = fixture("run", "lifted_join");
    assert_eq!(run.matches(from).count(), 1, "the change has one site");
    let (output, report) = invoke(&run.replace(from, to), &["--check"]);
    assert!(!output.status.success(), "{report}");
    for expected in expected {
        assert!(report.contains(expected), "{expected}: {report}");
    }
}

#[test]
fn a_lifted_quotient_runs_natively_and_in_psi() {
    program::assert_silent_return(&fixture("run", "lifted_join"));
}

#[test]
fn a_lift_argument_must_meet_the_representative_precondition() {
    refused(
        "(value, 3i32)",
        "(value, 4i32)",
        &["cannot prove requires contract for call representative from lifted: amount == 3"],
    );
}

#[test]
fn a_congruence_theorem_must_relate_both_representatives() {
    refused(
        "ensures representative(left, amount) == representative(right, amount)",
        "ensures representative(left, amount) == representative(left, amount)",
        &["quotient congruence representative call passes the wrong formal"],
    );
}

#[test]
fn an_executable_quotient_carrier_must_be_copyable() {
    refused(
        "data Representative [copy] { value: i32; }",
        "data Representative { value: i32; }",
        &["executable quotient carrier must be copyable without cleanup"],
    );
}
