//! A goal the prover cannot reach fails as a resource limit, promptly,
//! however many path facts surround it. Its search once recursed through
//! sign and bound queries for minutes before giving up.
#[path = "support/program.rs"]
mod program;

const SOURCE: &str = include_str!("fixtures/numbers/fail/out_of_reach_bound_search/main.omg");

const BUDGET_EXHAUSTED: &str =
    "proof search step budget exhausted: the proof search exceeded its step budget";

const GOAL_STEPS: &str = "[Incomplete code 37 (proof_search_goal_steps): obligation ";

#[test]
fn an_out_of_reach_range_goal_reports_its_exhausted_budget() {
    let (output, report) = program::invoke(SOURCE, &["--check"]);
    // A resource limit, not a verdict: the request is `Incomplete`.
    assert_eq!(output.status.code(), Some(2), "{report}");
    assert!(
        report.contains("local `padded` may overflow `u32`"),
        "{report}"
    );
    assert!(report.contains("may overflow `i64`"), "{report}");
    assert_eq!(report.matches(BUDGET_EXHAUSTED).count(), 2, "{report}");
    assert_eq!(report.matches(GOAL_STEPS).count(), 2, "{report}");
}

/// The same body checks once its two out-of-reach operations are replaced by
/// ones whose range follows directly: the budget refuses only those goals.
#[test]
fn the_body_checks_without_its_out_of_reach_operations() {
    let source = SOURCE
        .replace(
            "(((shift - match count <= shift { true -> count, false -> shift }) % 16u32) as +Wrapping)",
            "((shift % 16u32) as +Wrapping)",
        )
        .replace(
            "decoder.max_bits = decoder.max_bits - (shift as i64);",
            "decoder.max_bits = available;",
        );
    assert_ne!(source, SOURCE);
    let (output, report) = program::invoke(&source, &["--check"]);
    assert!(output.status.success(), "{report}");
    assert!(!report.contains(BUDGET_EXHAUSTED), "{report}");
    assert!(!report.contains(GOAL_STEPS), "{report}");
}
