//! Structural call-target queries for default-domain establishment summaries.
//!
//! Summary construction and flow mutation remain in the parent. This module
//! only resolves retained target identities and discovers nested calls whose
//! precomputed establishment summaries can be joined.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

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
            // The receiver expression may itself carry a call (`f().m()`).
            collect_call_summaries(program, call.receiver, summaries, call_established);
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
        ExpressionNode::Borrow(inner) => {
            collect_call_summaries(program, inner.target, summaries, call_established);
        }
        ExpressionNode::Indexed(indexed) => {
            collect_call_summaries(program, indexed.collection, summaries, call_established);
            collect_call_summaries(program, indexed.index, summaries, call_established);
        }
        ExpressionNode::Cast(cast) => {
            collect_call_summaries(program, cast.value, summaries, call_established);
        }
        ExpressionNode::Unary(unary) => {
            collect_call_summaries(program, unary.operand, summaries, call_established);
        }
        ExpressionNode::Atomic(atomic) => {
            collect_call_summaries(program, atomic.value, summaries, call_established);
            collect_call_summaries(program, atomic.result, summaries, call_established);
        }
        ExpressionNode::Range(range) => {
            collect_call_summaries(program, range.start, summaries, call_established);
            collect_call_summaries(program, range.end, summaries, call_established);
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                collect_call_summaries(program, *element, summaries, call_established);
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                collect_call_summaries(program, field.value, summaries, call_established);
            }
        }
        ExpressionNode::Match(matched) => {
            collect_call_summaries(program, matched.subject, summaries, call_established);
            for arm in program.expression_table.match_arms(matched.arms) {
                collect_call_summaries(program, arm.value, summaries, call_established);
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    collect_call_summaries(program, pattern, summaries, call_established);
                }
            }
        }
        _ => {}
    }
}
