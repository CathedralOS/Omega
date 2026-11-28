//! Specialization preserves the loans carried by a type argument.
//!
//! A type argument names borrow-carrying data with its lifetime, as in
//! `forward::<Loan<'lender>>` inside a machine that declares `'lender`.
//! lifetimes.md, "Binders and source applications": named data applications
//! "must match the declaration's lifetime arity", so a bare `forward::<Loan>`
//! is refused for that rule.

#[path = "support/program.rs"]
mod program;
mod support;

fn fixture_path(tier: &str, case: &str) -> std::path::PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/calls")
        .join(tier)
        .join(case)
}

#[test]
fn a_borrowed_type_argument_runs_when_interpreted_and_built() {
    program::assert_fixture_silent_return(&fixture_path("run", "copied_borrowed_types"));
}

#[test]
fn a_forwarded_loan_still_excludes_overlapping_writes() {
    let (output, report) = program::check_fixture(&fixture_path("fail", "copied_borrowed_types"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("storage is changed while a conflicting reference remains live")
            && !report.contains("not supported yet"),
        "{report}"
    );
}

#[test]
fn a_type_argument_without_its_lifetime_arguments_is_refused() {
    let (output, report) =
        program::check_fixture(&fixture_path("fail", "bare_borrowed_type_argument"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains(
            "type argument `Loan` supplies no lifetime arguments, but `Loan` takes 1 lifetime \
             argument; write `Loan<'lender>` with a lifetime binder declared by the enclosing \
             machine"
        ) && !report.contains("not supported yet"),
        "{report}"
    );
}
