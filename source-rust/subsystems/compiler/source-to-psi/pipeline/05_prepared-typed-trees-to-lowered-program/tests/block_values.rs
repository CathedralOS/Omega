//! Lowering must lower a block-valued arm from the block typing handed it:
//! the block's own statements, then its final value, then the end of its
//! locals' storage. These tests keep the source fixed, change one fact of the
//! typed block as a faulty typer would, and require the lowered program to
//! change with it.
//!
//! The program and typing's side of the handoff are in
//! `support/dispatch_arms.rs` and
//! `03_symbol-resolved-trees-to-typed-trees/tests/block_values.rs`; the
//! checker's side is `06_lowered-program-to-checked-program/tests/block_values.rs`.
//! The rule: `wiki/spec/language/expressions.md`, "A block introduces local
//! scope and cleanup edges, not a new machine declaration".
//!
//! `PreparedTypedTrees` cannot be built or edited outside stage 04. Each
//! altered product is made by taking the prepared trees back with
//! `into_inner`, changing one fact, and running stage 04 again.

#[path = "support/dispatch_arms.rs"]
mod program;

use program::{Prepared, block_binding, machine, prepare};
use semantic_vocabulary::IntegerValue;
use source_compiler::representations::source_state::expression::ExpressionNode;
use source_compiler::representations::source_state::statement::StatementNode;
use terminal_psi::{
    OperationKind, OperationResult, TerminalAffineCleanupAction, TerminalMachine, Terminator,
};

/// The constants `machine` returns, in block order.
fn returned_constants(machine: &TerminalMachine) -> Vec<IntegerValue> {
    let constants = machine
        .operations()
        .filter_map(|operation| match (&operation.kind, &operation.result) {
            (OperationKind::IntegerConstant { value }, OperationResult::Scalar(result)) => {
                Some((result.id, *value))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    machine
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::Return { value, .. } => Some(
                constants
                    .iter()
                    .find(|(id, _)| id == value)
                    .unwrap_or_else(|| panic!("a return of a value no constant produced"))
                    .1,
            ),
            _ => None,
        })
        .collect()
}

fn fixture() -> Prepared {
    prepare(&["pick", "scoped"])
}

#[test]
fn a_block_arm_returns_the_value_its_own_statement_binds() {
    let fixture = fixture();
    let lowered = fixture.lower();
    let lowered = lowered.closure.as_ref().unwrap();
    let pick = machine(&fixture, lowered, "pick");
    assert_eq!(
        returned_constants(pick),
        [IntegerValue::Unsigned(41), IntegerValue::Unsigned(7)]
    );
}

#[test]
fn changing_the_blocks_statement_changes_the_lowered_value() {
    let fixture = fixture();
    let changed = fixture.prepared_with(|trees| {
        let (_, block) = block_binding(trees, "port");
        let StatementNode::LocalData(port) =
            trees.statement_table.statements(block.statements)[0].clone()
        else {
            panic!("the block binds its local first")
        };
        let ExpressionNode::Integer(value) =
            trees.expression_table.expression_mut(port.initial_value)
        else {
            panic!("the local is bound to a literal")
        };
        *value = numeric_literals::IntegerLiteral::from_value(43);
    });
    let lowered = fixture.lower_program(&changed);
    let lowered = lowered.closure.as_ref().unwrap();
    assert_eq!(
        returned_constants(machine(&fixture, lowered, "pick")),
        [IntegerValue::Unsigned(43), IntegerValue::Unsigned(7)]
    );
}

#[test]
fn a_block_without_the_statement_that_binds_its_value_is_refused() {
    let fixture = fixture();
    // The block keeps its final value but no longer binds the local the
    // value names; no other statement of the body binds it either.
    let changed = fixture.prepared_with(|trees| {
        let (handle, _) = block_binding(trees, "port");
        let ExpressionNode::Block(block) = trees.expression_table.expression_mut(handle) else {
            panic!("the handle is the block")
        };
        block.statements = arena::HandleSpan::empty();
    });
    let lowered = fixture.lower_program(&changed);
    let refusal = lowered
        .closure
        .expect_err("lowering cannot find the block's local elsewhere");
    assert!(
        refusal.detail.contains("local of another scope"),
        "{refusal:?}"
    );
}

#[test]
fn a_blocks_record_local_ends_on_its_arms_edge_out_of_the_match() {
    let fixture = fixture();
    let lowered = fixture.lower();
    let lowered = lowered.closure.as_ref().unwrap();
    let scoped = machine(&fixture, lowered, "scoped");
    // One edge ends a root: the block arm's arrival at the match's join,
    // which carries the arm's value. The root is the `Counter` the block
    // built. The wildcard arm builds nothing and ends nothing.
    let discarding = scoped
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::Jump {
                arguments,
                cleanup_actions,
                ..
            } if !cleanup_actions.is_empty() => Some((arguments.len(), cleanup_actions.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(1, actions)] = discarding.as_slice() else {
        panic!("one arrival with the arm's value ends storage: {discarding:?}")
    };
    let [TerminalAffineCleanupAction::DiscardRoot(root)] = actions.as_slice() else {
        panic!("the arrival ends one root: {actions:?}")
    };
    let built = scoped
        .operations()
        .find_map(|operation| match &operation.result {
            OperationResult::Structural(result) if result.place == *root => {
                Some(result.structural_type)
            }
            _ => None,
        })
        .expect("an operation of `scoped` produced the root");
    let declared = lowered
        .module
        .types
        .structural_types
        .iter()
        .find(|declared| declared.id == built)
        .unwrap();
    let identity = lowered.module.identities.text(declared.identity);
    assert!(
        identity.is_some_and(|identity| identity.contains("Counter")),
        "the ended root is the block's `Counter`: {identity:?}"
    );
}

#[test]
fn a_block_arm_crash_emits_a_crash_terminator_with_the_arms_guard() {
    let fixture = program::prepare_source(program::CONTROL_FLOW, &["guarded_crash", "early_exit"]);
    let lowered = fixture.lower();
    let lowered = lowered.closure.as_ref().unwrap();
    let guarded = machine(&fixture, lowered, "guarded_crash");
    let crashes = guarded
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::Crash {
                cause, site_guard, ..
            } => Some((*cause, site_guard.len())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        crashes,
        [(semantic_vocabulary::CrashCause::Trap, 1)],
        "the block arm crash carries the arm's branch guard: {crashes:?}"
    );
}

#[test]
fn a_block_arm_transition_returns_from_the_enclosing_machine() {
    let fixture = program::prepare_source(program::CONTROL_FLOW, &["guarded_crash", "early_exit"]);
    let lowered = fixture.lower();
    let lowered = lowered.closure.as_ref().unwrap();
    let early = machine(&fixture, lowered, "early_exit");
    let returns = early
        .blocks
        .iter()
        .filter(|block| matches!(block.terminator, Terminator::Return { .. }))
        .count();
    assert_eq!(
        returns, 3,
        "two returns from the block arm's transition plus the outer return"
    );
}

#[test]
fn a_tail_after_crash_in_a_block_arm_is_refused() {
    let fixture = program::prepare_source(program::TRAILING_AFTER_CRASH, &["trailing"]);
    let lowered = fixture.lower_program(&fixture.program);
    let refusal = lowered
        .closure
        .expect_err("a tail expression after crash must be refused");
    assert!(
        refusal
            .detail
            .contains("statement after an unconditional arm"),
        "{refusal:?}"
    );
}
