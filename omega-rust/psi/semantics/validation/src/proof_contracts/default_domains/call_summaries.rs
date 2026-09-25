//! Structural call-target queries for default-domain establishment summaries.
//!
//! Summary construction and flow mutation remain in the parent. This module
//! only resolves retained target identities and discovers nested calls whose
//! precomputed establishment summaries can be joined.

use symbols::SymbolHandle;
use symbols::SymbolKeyMap;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

/// Slice 11: state symbol to its owning machine symbol, built once per
/// analysis pass (call targets carry the STATE's symbol -- the effects
/// builder's proven resolution rule). First machine wins on the impossible
/// collision, matching the removed per-call scan's early return.
pub(super) type StateToMachine = SymbolKeyMap<SymbolHandle, SymbolHandle>;

pub(super) fn state_to_machine_index(program: &TypedTrees) -> StateToMachine {
    let mut index = StateToMachine::default();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            index.entry(state.symbol).or_insert(machine.symbol);
        }
    }
    index
}

/// Slice 11: find call targets inside an expression and join their
/// establishment summaries.
pub(super) fn collect_call_summaries(
    program: &TypedTrees,
    expression: ExpressionHandle,
    state_to_machine: &StateToMachine,
    summaries: &SymbolKeyMap<SymbolHandle, Vec<String>>,
    call_established: &mut Vec<String>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            let target_machine = state_to_machine
                .get(&call.target_symbol)
                .copied()
                .unwrap_or_else(SymbolHandle::invalid);
            if let Some(established) = summaries.get(&target_machine) {
                call_established.extend(established.iter().cloned());
            }
            // The receiver expression may itself carry a call (`f().m()`).
            collect_call_summaries(
                program,
                call.receiver,
                state_to_machine,
                summaries,
                call_established,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_call_summaries(
                    program,
                    *argument,
                    state_to_machine,
                    summaries,
                    call_established,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_call_summaries(
                program,
                binary.left,
                state_to_machine,
                summaries,
                call_established,
            );
            collect_call_summaries(
                program,
                binary.right,
                state_to_machine,
                summaries,
                call_established,
            );
        }
        ExpressionNode::Member(member) => {
            collect_call_summaries(
                program,
                member.receiver,
                state_to_machine,
                summaries,
                call_established,
            );
        }
        ExpressionNode::Borrow(inner) => {
            collect_call_summaries(
                program,
                inner.target,
                state_to_machine,
                summaries,
                call_established,
            );
        }
        ExpressionNode::Indexed(indexed) => {
            collect_call_summaries(
                program,
                indexed.collection,
                state_to_machine,
                summaries,
                call_established,
            );
            collect_call_summaries(
                program,
                indexed.index,
                state_to_machine,
                summaries,
                call_established,
            );
        }
        ExpressionNode::Cast(cast) => {
            collect_call_summaries(
                program,
                cast.value,
                state_to_machine,
                summaries,
                call_established,
            );
        }
        ExpressionNode::Unary(unary) => {
            collect_call_summaries(
                program,
                unary.operand,
                state_to_machine,
                summaries,
                call_established,
            );
        }
        ExpressionNode::Atomic(atomic) => {
            collect_call_summaries(
                program,
                atomic.value,
                state_to_machine,
                summaries,
                call_established,
            );
            collect_call_summaries(
                program,
                atomic.result,
                state_to_machine,
                summaries,
                call_established,
            );
        }
        ExpressionNode::Range(range) => {
            collect_call_summaries(
                program,
                range.start,
                state_to_machine,
                summaries,
                call_established,
            );
            collect_call_summaries(
                program,
                range.end,
                state_to_machine,
                summaries,
                call_established,
            );
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                collect_call_summaries(
                    program,
                    *element,
                    state_to_machine,
                    summaries,
                    call_established,
                );
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                collect_call_summaries(
                    program,
                    field.value,
                    state_to_machine,
                    summaries,
                    call_established,
                );
            }
        }
        ExpressionNode::Match(matched) => {
            collect_call_summaries(
                program,
                matched.subject,
                state_to_machine,
                summaries,
                call_established,
            );
            for arm in program.expression_table.match_arms(matched.arms) {
                collect_call_summaries(
                    program,
                    arm.value,
                    state_to_machine,
                    summaries,
                    call_established,
                );
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    collect_call_summaries(
                        program,
                        pattern,
                        state_to_machine,
                        summaries,
                        call_established,
                    );
                }
            }
        }
        _ => {}
    }
}
