//! The checker knows no source scopes. It holds a block's locals to the
//! storage stage 05 ends on the block arm's edge out of its match, and
//! checks that ending against its own disposal schedule. These tests take
//! stage 05's output for one fixed program, change that one fact as a faulty
//! stage 05 would, and hand the result to the checker.
//!
//! The program is
//! `05_prepared-typed-trees-to-lowered-program/tests/support/dispatch_arms.rs`,
//! and stage 05's side of the handoff is that stage's `tests/block_values.rs`.
//! `scoped` builds a record in its block and reads it there; a lowered
//! program that kept the record alive past the arm is refused. `escape`
//! returns a loan of such a record out of its block: the storage ends while
//! the loan is still used, so the program is refused. The rules:
//! `wiki/spec/language/expressions.md`, "A block introduces local scope and
//! cleanup edges" and "Temporary storage must outlive every derived borrow".

#[path = "../../05_prepared-typed-trees-to-lowered-program/tests/support/dispatch_arms.rs"]
mod program;

use source_compiler::test_support::checking::{
    CompilationRefusal, RouteVerdict, judge_compilation,
};
use terminal_check_tables::compilation::{PreparedCompilation, TerminalCompilation};
use terminal_psi::{TerminalAffineCleanupAction, TerminalMachine, Terminator};

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

/// Leave out every root `machine` ends on a jump, as a stage 05 that kept
/// a block's locals alive past its arm would.
fn omit_discards(machine: &mut TerminalMachine) -> Vec<TerminalAffineCleanupAction> {
    let mut omitted = Vec::new();
    for block in &mut machine.blocks {
        if let Terminator::Jump {
            cleanup_actions, ..
        } = &mut block.terminator
        {
            let (discards, kept) = std::mem::take(cleanup_actions)
                .into_iter()
                .partition(|action| matches!(action, TerminalAffineCleanupAction::DiscardRoot(_)));
            *cleanup_actions = kept;
            omitted.extend::<Vec<_>>(discards);
        }
    }
    omitted
}

#[test]
fn a_block_whose_locals_end_inside_it_is_accepted() {
    let fixture = program::prepare(&["pick", "scoped"]);
    assert!(matches!(judged(fixture.lower()), Verdict::Accepted));
}

#[test]
fn a_block_local_that_outlives_its_arm_is_refused() {
    let fixture = program::prepare(&["pick", "scoped"]);
    let lowered = fixture.lower();
    let position = position(&fixture, &lowered, "scoped");
    let mut kept = lowered;
    let machine = &mut kept.closure.as_mut().unwrap().module.machines[position];
    assert_eq!(
        omit_discards(machine).len(),
        1,
        "the arm ends its block's record"
    );
    match judged(kept) {
        Verdict::Refused(detail) => assert!(
            detail.contains("normal edge affine disposal differs from its canonical schedule"),
            "{detail}"
        ),
        other => panic!("the record outlives its block: {other:?}"),
    }
}

#[test]
fn a_loan_of_a_block_local_used_after_the_block_is_refused() {
    let fixture = program::prepare_source(program::ESCAPE, &["escape"]);
    match judged(fixture.lower()) {
        Verdict::Refused(detail) => assert!(
            detail.contains("storage is changed while a conflicting reference remains live"),
            "{detail}"
        ),
        other => panic!("the loan outlives its block: {other:?}"),
    }
}

#[test]
fn a_block_arm_crash_and_transition_with_valid_contracts_are_accepted() {
    let fixture = program::prepare_source(program::CONTROL_FLOW, &["guarded_crash", "early_exit"]);
    assert!(
        matches!(judged(fixture.lower()), Verdict::Accepted),
        "guarded crash and early transition in block arms must pass checking"
    );
}

#[test]
fn an_uncovered_crash_in_a_block_arm_is_refused() {
    let fixture = program::prepare_source(program::UNCOVERED_CRASH, &["uncovered_crash"]);
    match judged(fixture.lower()) {
        Verdict::Refused(detail) => assert!(
            detail.contains("CrashCoverage"),
            "uncovered crash in block arm must fail CrashCoverage: {detail}"
        ),
        other => panic!("uncovered crash in block arm must be refused: {other:?}"),
    }
}

#[test]
fn a_block_arm_crash_with_a_mismatched_guard_is_refused() {
    let fixture = program::prepare_source(program::WRONG_GUARD_CRASH, &["wrong_guard_crash"]);
    match judged(fixture.lower()) {
        Verdict::Refused(detail) => assert!(
            detail.contains("CrashCoverage"),
            "wrong crash guard in block arm must fail CrashCoverage: {detail}"
        ),
        other => panic!("wrong crash guard in block arm must be refused: {other:?}"),
    }
}

/// The position of the machine the source names `name` in stage 05's module.
fn position(fixture: &program::Prepared, lowered: &PreparedCompilation, name: &str) -> usize {
    let closure = lowered.closure.as_ref().unwrap();
    let id = program::machine(fixture, closure, name).id;
    closure
        .module
        .machines
        .iter()
        .position(|machine| machine.id == id)
        .unwrap()
}
