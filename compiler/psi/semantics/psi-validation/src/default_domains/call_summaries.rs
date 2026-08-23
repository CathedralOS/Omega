//! Structural call-target queries for default-domain establishment summaries.
//!
//! Summary construction and flow mutation remain in the parent. This module
//! only resolves retained target identities and discovers nested calls whose
//! precomputed establishment summaries can be joined.

use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};

/// Slice 11: the machine owning a state symbol (call targets carry the
/// STATE's symbol -- the effects builder's proven resolution rule).
pub(super) fn machine_symbol_for_state(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
) -> SymbolHandle {
    if !state_symbol.is_valid() {
        return SymbolHandle::invalid();
    }
    for machine in program.machines() {
        if program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == state_symbol)
        {
            return machine.symbol;
        }
    }
    SymbolHandle::invalid()
}

/// Slice 11: find call targets inside an expression and join their
/// establishment summaries.
pub(super) fn collect_call_summaries(
    program: &TypedTrees,
    expression: ExpressionHandle,
    summaries: &[(SymbolHandle, Vec<String>)],
    call_established: &mut Vec<String>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            let target_machine = machine_symbol_for_state(program, call.target_symbol);
            if let Some((_, established)) = summaries
                .iter()
                .find(|(symbol, _)| *symbol == target_machine)
            {
                call_established.extend(established.iter().cloned());
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_call_summaries(program, *argument, summaries, call_established);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_call_summaries(program, binary.left, summaries, call_established);
            collect_call_summaries(program, binary.right, summaries, call_established);
        }
        ExpressionNode::Member(member) => {
            collect_call_summaries(program, member.receiver, summaries, call_established);
        }
        ExpressionNode::Mutable(inner) => {
            collect_call_summaries(program, *inner, summaries, call_established);
        }
        _ => {}
    }
}
