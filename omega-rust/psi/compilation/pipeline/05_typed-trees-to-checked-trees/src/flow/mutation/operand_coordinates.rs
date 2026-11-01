//! Operation meaning required before call operands supply captured coordinates.

use validation::CallSite;
use validation::call_site_argument_expressions;

use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use symbols::SymbolHandle;

pub(super) fn call_operands_have_builtin_coordinates(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    site: &CallSite<'_>,
) -> bool {
    let Some(machine) = program.machine_by_symbol(machine_symbol) else {
        return false;
    };
    let Some(state) = program.state_by_symbol(state_symbol) else {
        return false;
    };
    if !call_site_argument_expressions(program, site)
        .iter()
        .all(|argument| place_operands_are_builtin(program, machine, state, *argument, 0))
    {
        return false;
    }
    match site {
        CallSite::Expression { call, .. } if call.receiver.is_valid() => {
            place_operands_are_builtin(program, machine, state, call.receiver, 0)
        }
        // Statement receivers retain named field paths; captured reference
        // selectors are checked by the shared local-origin adapter.
        _ => true,
    }
}

fn place_operands_are_builtin(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    depth: usize,
) -> bool {
    if depth >= 128 {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => {
            place_operands_are_builtin(program, machine, state, borrow.target, depth + 1)
        }
        ExpressionNode::Member(member) => {
            place_operands_are_builtin(program, machine, state, member.receiver, depth + 1)
        }
        ExpressionNode::Indexed(indexed)
            if matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) =>
        {
            // Range normalization owns its endpoint evidence. This gate
            // checks newly retained scalar element geometry beneath it.
            place_operands_are_builtin(program, machine, state, indexed.collection, depth + 1)
        }
        ExpressionNode::Indexed(_) => {
            // An owned operand may project a completed call or constructor.
            // The storage-only spine judgment deliberately excludes those
            // producers; applying it here discards all surviving facts even
            // when the callee's ordinary frame establishes no caller writes.
            // Authenticate the produced value's selected index meaning, then
            // let the existing capture/frame and signature-ceiling analysis
            // determine its real effects. Coordinates confer no purity,
            // bounds, loan or ownership evidence.
            validation::value_has_builtin_coordinates(program, machine, state, expression)
        }
        // Nested non-place evaluation contributes its own call effects; its
        // syntax supplies no independently guessed storage geometry here.
        _ => true,
    }
}
