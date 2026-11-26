//! An arm of a `transition` without a subject is one guard, and the guard may
//! start with a parenthesis: `(a + b) > a + 3 -> done()`.
//!
//! The parser used to read the leading `(a + b)` as a tuple pattern and then
//! refuse the rest ("expected transition pattern"). The program takes the
//! guarded arm only when `7 > 6`, and crashes in the other arm, so it exits 0
//! only when the whole comparison was the guard. Its twin writes the
//! parenthesized list `(a + b, a)`, a tuple pattern, which a transition
//! without a subject cannot hold.
//!
//! `wiki/spec/language/patterns.md#bare-names-in-dispatch-arms`: "In a
//! dispatch without a subject, an arm condition such as `ready -> 1` reads the
//! existing value of `ready`." The parser's own test is
//! `pipeline/01_tokens-to-syntax-trees/tests/unit/subjectless_guards.rs`.

#[path = "support/program.rs"]
mod program;
mod support;

fn fixture(tier: &str) -> String {
    let path = support::repository_root()
        .join("source-rust/application/tests/fixtures/transitions")
        .join(tier)
        .join("parenthesized_guard/main.omg");
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn a_guard_that_starts_with_a_parenthesis_runs_natively_and_in_psi() {
    program::assert_silent_return(&fixture("run"));
}

#[test]
fn a_parenthesized_list_in_a_transition_without_a_subject_is_refused() {
    // The twins differ only in the first arm's condition.
    assert_eq!(
        fixture("run").replace("(a + b) > a + 3 -> done(),", "(a + b, a) -> done(),"),
        fixture("fail")
    );
    let (output, report) = program::invoke(&fixture("fail"), &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("main.omg:5:9: anonymous transition blocks do not support tuple patterns"),
        "{report}"
    );
}
