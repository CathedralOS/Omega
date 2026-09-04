use super::*;
use crate::lookup::{statement_call_can_dispatch_to_machine, statement_call_receiver_path};
mod collection;
mod expression;
mod transitions;

use collection::BorrowCallCollection;
use transitions::collect_transition_target_borrow_calls;

pub(crate) fn collect_statement_borrow_calls(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    statement_index: usize,
    statement: &StatementNode,
    call_ordinal: &mut usize,
    access_segments: &mut psi_arena::Arena<psi_facts::PlaceSegment>,
    argument_accesses: &mut psi_arena::Arena<BorrowArgumentAccessFact>,
    calls: &mut psi_arena::Arena<BorrowCallFact>,
    state_calls: &mut psi_arena::HandleSpan<BorrowCallFact>,
) {
    let mut collection = BorrowCallCollection::new(
        program,
        machine,
        state,
        statement_index,
        call_ordinal,
        access_segments,
        argument_accesses,
        calls,
        state_calls,
    );

    match statement {
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            expression::collect_expression_borrow_calls(&mut collection, assignment.value)
        }
        StatementNode::Call(call) => {
            let resolved_target = crate::lookup::resolve_state_call_target(
                program,
                machine,
                state,
                call.receiver_symbol,
                call.target_symbol,
                crate::lookup::statement_call_receiver_members(program, call),
                &call.target,
            );
            if !call.target.as_str().starts_with("accept_boundary#")
                && (resolved_target.is_valid()
                    || statement_call_can_dispatch_to_machine(program, machine, state, call))
            {
                let receiver_path = statement_call_receiver_path(program, call);
                let accesses = collection.collect_call_argument_accesses(
                    program.statement_table.expression_handles(call.arguments),
                );
                collection.append_borrow_call(
                    call.receiver_symbol,
                    if resolved_target.is_valid() {
                        resolved_target
                    } else {
                        call.target_symbol
                    },
                    receiver_path.is_some(),
                    accesses,
                );
            }

            for argument in program.statement_table.expression_handles(call.arguments) {
                expression::collect_expression_borrow_calls(&mut collection, *argument);
            }
        }
        StatementNode::Expression(expression) => {
            expression::collect_expression_borrow_calls(&mut collection, *expression)
        }
        StatementNode::LocalData(local_data) => {
            if local_data.initial_value.is_valid() {
                expression::collect_expression_borrow_calls(
                    &mut collection,
                    local_data.initial_value,
                );
            }
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(expression) = transition.guard {
                expression::collect_expression_borrow_calls(&mut collection, expression);
            }

            collect_transition_target_borrow_calls(&mut collection, transition.target);

            if transition.continuation.is_valid() {
                collect_transition_target_borrow_calls(&mut collection, transition.continuation);
            }
        }
    }
}
