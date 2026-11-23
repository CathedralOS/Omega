//! The checker knows no source arms. What it holds about a condition arm of
//! a `match` without a subject is the branch stage 05 lowers it to: inside
//! the arm the condition is a fact, and outside it is not. These tests take
//! stage 05's output for one fixed program, change that branch as a faulty
//! stage 05 would, and hand the result to the checker.
//!
//! The program is `GUARDED` in
//! `05_prepared-typed-trees-to-lowered-program/tests/support/dispatch_arms.rs`,
//! and stage 05's side of the handoff is that stage's
//! `tests/subjectless_match.rs`. In `gap`, `a > b -> a - b` subtracts only
//! where `a > b` holds, so the result cannot go below zero. A lowered `gap`
//! that runs the subtraction on the other edge of the branch is refused. The
//! rules: `wiki/spec/language/patterns.md`, "In a dispatch without a subject,
//! an arm condition such as `ready -> 1` reads the existing value of
//! `ready`", and `wiki/spec/language/numeric_values.md`, "Every Exact
//! operation owes its own representability proof in its actual carrier".

#[path = "../../05_prepared-typed-trees-to-lowered-program/tests/support/dispatch_arms.rs"]
mod program;

use source_compiler::test_support::checking::{
    CompilationRefusal, RouteVerdict, judge_compilation,
};
use terminal_check_tables::compilation::{PreparedCompilation, TerminalCompilation};
use terminal_psi::Terminator;

#[derive(Debug)]
enum Verdict {
    Accepted,
    Refused(String),
}

fn judged(lowered: PreparedCompilation) -> Verdict {
    let mut verdicts = judge_compilation(TerminalCompilation::Prepared(lowered)).into_iter();
    let verdict = verdicts.next().expect("one module, one verdict");
    assert!(verdicts.next().is_none(), "the program is one module");
    match verdict {
        Ok(RouteVerdict::Accepted(_)) => Verdict::Accepted,
        Ok(rejected @ RouteVerdict::Rejected { .. }) => Verdict::Refused(format!("{rejected:?}")),
        Err(CompilationRefusal::Elaboration(refusal)) => Verdict::Refused(refusal.detail),
        other => panic!("{other:?}"),
    }
}

#[test]
fn condition_arms_are_accepted() {
    let fixture = program::prepare_source(program::GUARDED, &["larger", "gap"]);
    assert!(matches!(judged(fixture.lower()), Verdict::Accepted));
}

#[test]
fn a_subtraction_on_the_edge_where_the_condition_is_false_is_refused() {
    let fixture = program::prepare_source(program::GUARDED, &["larger", "gap"]);
    let mut lowered = fixture.lower();
    let closure = lowered.closure.as_mut().unwrap();
    let id = program::machine(&fixture, closure, "gap").id;
    let gap = closure
        .module
        .machines
        .iter_mut()
        .find(|machine| machine.id == id)
        .unwrap();
    let mut swapped = 0;
    for block in &mut gap.blocks {
        if let Terminator::Conditional {
            when_true,
            when_false,
            ..
        } = &mut block.terminator
        {
            std::mem::swap(when_true, when_false);
            swapped += 1;
        }
    }
    assert_eq!(swapped, 1, "`gap` branches once");
    match judged(lowered) {
        Verdict::Refused(detail) => assert!(detail.contains("may overflow `u32`"), "{detail}"),
        other => panic!("`a - b` runs where `a > b` is false: {other:?}"),
    }
}
