//! Lowering must turn each condition arm of a `match` without a subject into
//! the branch condition of its own test, in authored order, and read no
//! subject. These tests keep the source fixed, change one fact of the typed
//! match as a faulty typer would, and require the lowered program to change
//! with it, or the match to be refused.
//!
//! The program is `GUARDED` in `support/dispatch_arms.rs`; typing's side of
//! the handoff is `03_symbol-resolved-trees-to-typed-trees/tests/subjectless_match.rs`,
//! and the checker's is `06_lowered-program-to-checked-program/tests/subjectless_match.rs`.
//! The rules: `wiki/spec/language/patterns.md`, "In a dispatch without a
//! subject, an arm condition such as `ready -> 1` reads the existing value of
//! `ready`", and "Other value/comparison ladders and predicate-domain guards
//! require a wildcard".
//!
//! `PreparedTypedTrees` cannot be built or edited outside stage 04. Each
//! altered product is made by taking the prepared trees back with
//! `into_inner`, changing one fact, and running stage 04 again.

#[path = "support/dispatch_arms.rs"]
mod program;

use program::{GUARDED, Prepared, machine, prepare_source, subjectless_matches};
use source_compiler::representations::source_state::expression::{
    BinaryOperator, ExpressionNode, MatchPattern,
};
use source_compiler::test_support::preparation::PreparedTypedTrees;
use terminal_psi::{OperationKind, OperationResult, TerminalMachine, Terminator};

fn fixture() -> Prepared {
    prepare_source(GUARDED, &["larger", "gap"])
}

/// The operands of the one comparison `machine` branches on, named by
/// parameter position, and whether any operation makes a Boolean constant.
fn branch(machine: &TerminalMachine) -> ([usize; 2], bool) {
    let parameters = machine
        .contract
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect::<Vec<_>>();
    let conditions = machine
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::Conditional { condition, .. } => Some(*condition),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [condition] = conditions.as_slice() else {
        panic!("one branch: {conditions:?}")
    };
    let operands = machine
        .operations()
        .find_map(|operation| match (&operation.kind, &operation.result) {
            (OperationKind::IntegerLessThan { left, right }, OperationResult::Scalar(result))
                if result.id == *condition =>
            {
                Some([*left, *right])
            }
            _ => None,
        })
        .expect("the branch condition is an integer comparison");
    let position = |value| {
        parameters
            .iter()
            .position(|parameter| *parameter == value)
            .expect("the comparison reads a parameter")
    };
    let constant = machine
        .operations()
        .any(|operation| matches!(operation.kind, OperationKind::BooleanConstant { .. }));
    ([position(operands[0]), position(operands[1])], constant)
}

/// The same trees with `larger`'s condition operator replaced.
fn with_operator(fixture: &Prepared, operator: BinaryOperator) -> PreparedTypedTrees {
    fixture.prepared_with(|trees| {
        let (_, larger) = subjectless_matches(trees)[0];
        let MatchPattern::Guard(condition) =
            trees.expression_table.match_arms(larger.arms)[0].pattern
        else {
            panic!("`larger`'s first arm is a condition")
        };
        let ExpressionNode::Binary(condition) = trees.expression_table.expression_mut(condition)
        else {
            panic!("the condition is a comparison")
        };
        condition.operator = operator;
    })
}

#[test]
fn a_condition_is_its_own_branch_and_no_subject_is_read() {
    let fixture = fixture();
    let lowered = fixture.lower();
    let lowered = lowered.closure.as_ref().unwrap();
    // `a > b` is `b < a`; the subject `true` the reader puts in place of the
    // missing subject is never made.
    assert_eq!(
        branch(machine(&fixture, lowered, "larger")),
        ([1, 0], false)
    );
}

#[test]
fn changing_the_typed_condition_changes_the_branch() {
    let fixture = fixture();
    let changed = with_operator(&fixture, BinaryOperator::Less);
    let lowered = fixture.lower_program(&changed);
    let lowered = lowered.closure.as_ref().unwrap();
    assert_eq!(
        branch(machine(&fixture, lowered, "larger")),
        ([0, 1], false)
    );
}

#[test]
fn a_match_without_a_subject_or_a_wildcard_is_refused() {
    let fixture = fixture();
    // Typing hands over `larger`'s match without its `_` arm.
    let changed = fixture.prepared_with(|trees| {
        let (handle, _) = subjectless_matches(trees)[0];
        let ExpressionNode::Match(larger) = trees.expression_table.expression_mut(handle) else {
            panic!("the handle is the match")
        };
        larger.arms = arena::HandleSpan::from_parts(larger.arms.start(), 1);
    });
    let lowered = fixture.lower_program(&changed);
    let refusal = lowered
        .closure
        .expect_err("no arm applies when the condition is false");
    assert!(
        refusal
            .detail
            .contains("this match has no `_` arm: when every condition is false no arm applies"),
        "{refusal:?}"
    );
}
