//! Structural call-target queries for default-domain establishment summaries.
//!
//! Summary construction and flow mutation remain in the parent. This module
//! only resolves retained target identities and discovers nested calls whose
//! precomputed establishment summaries can be joined.

use symbols::SymbolHandle;
use symbols::SymbolKeyMap;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

/// Slice 11: state symbol to its owning machine symbol, built once per
/// analysis pass (call targets carry the STATE's symbol -- the effects
/// builder's proven resolution rule). First machine wins on the impossible
/// collision, matching the removed per-call scan's early return.
pub(crate) type StateToMachine = SymbolKeyMap<SymbolHandle, SymbolHandle>;

pub(crate) fn state_to_machine_index(program: &dyn crate::CheckingInput) -> StateToMachine {
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
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    state_to_machine: &StateToMachine,
    summaries: &SymbolKeyMap<SymbolHandle, Vec<String>>,
    call_established: &mut Vec<String>,
) {
    if !expression.is_valid() {
        return;
    }
    macro_rules! visit_child {
        ($child:expr) => {
            collect_call_summaries(
                program,
                $child,
                state_to_machine,
                summaries,
                call_established,
            );
        };
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
            visit_child!(call.receiver);
            for argument in program.expression_table.expression_handles(call.arguments) {
                visit_child!(*argument);
            }
        }
        ExpressionNode::Binary(binary) => {
            visit_child!(binary.left);
            visit_child!(binary.right);
        }
        ExpressionNode::Member(member) => {
            visit_child!(member.receiver);
        }
        ExpressionNode::Borrow(inner) => {
            visit_child!(inner.target);
        }
        ExpressionNode::Indexed(indexed) => {
            visit_child!(indexed.collection);
            visit_child!(indexed.index);
        }
        ExpressionNode::Cast(cast) => {
            visit_child!(cast.value);
        }
        ExpressionNode::Unary(unary) => {
            visit_child!(unary.operand);
        }
        ExpressionNode::Atomic(atomic) => {
            visit_child!(atomic.value);
            visit_child!(atomic.result);
        }
        ExpressionNode::Range(range) => {
            visit_child!(range.start);
            visit_child!(range.end);
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                visit_child!(*element);
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                visit_child!(field.value);
            }
        }
        ExpressionNode::Match(matched) => {
            visit_child!(matched.subject);
            for arm in program.expression_table.match_arms(matched.arms) {
                visit_child!(arm.value);
                for pattern in program.expression_table.match_pattern_operands(arm.pattern) {
                    visit_child!(pattern);
                }
            }
        }
        _ => {}
    }
}
