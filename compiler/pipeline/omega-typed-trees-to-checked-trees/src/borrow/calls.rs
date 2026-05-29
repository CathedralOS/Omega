use super::*;
use crate::borrow::accesses::collect_call_argument_accesses;
use crate::lookup::{statement_call_can_dispatch_to_machine, statement_call_receiver_path};
mod expression;
mod transitions;

use transitions::collect_transition_target_borrow_calls;

pub(super) struct BorrowCallCollection<'a> {
    program: &'a omega_typed_trees::TypedTrees,
    machine: &'a omega_typed_trees::machine::Machine,
    state: &'a omega_typed_trees::state::State,
    statement_index: usize,
    call_ordinal: &'a mut usize,
    access_segments: &'a mut omega_core::arena::Arena<omega_facts::PlaceSegment>,
    argument_accesses: &'a mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    calls: &'a mut omega_core::arena::Arena<BorrowCallFact>,
    state_calls: &'a mut omega_core::arena::HandleSpan<BorrowCallFact>,
}

impl BorrowCallCollection<'_> {
    fn collect_call_argument_accesses(
        &mut self,
        arguments: &[ExpressionHandle],
    ) -> omega_core::arena::HandleSpan<BorrowArgumentAccessFact> {
        collect_call_argument_accesses(
            self.program,
            self.access_segments,
            self.argument_accesses,
            arguments,
            self.state.symbol,
            self.statement_index,
            self.machine.symbol,
        )
    }

    fn append_borrow_call(
        &mut self,
        receiver_symbol: SymbolHandle,
        target_symbol: SymbolHandle,
        has_receiver: bool,
        accesses: omega_core::arena::HandleSpan<BorrowArgumentAccessFact>,
    ) {
        self.calls.append_to_span(
            self.state_calls,
            BorrowCallFact {
                statement_index: self.statement_index,
                call_ordinal: *self.call_ordinal,
                receiver_symbol,
                target_symbol,
                has_receiver,
                accesses,
            },
        );
        *self.call_ordinal += 1;
    }
}

pub(crate) fn collect_statement_borrow_calls(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    statement: &StatementNode,
    call_ordinal: &mut usize,
    access_segments: &mut omega_core::arena::Arena<omega_facts::PlaceSegment>,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    calls: &mut omega_core::arena::Arena<BorrowCallFact>,
    state_calls: &mut omega_core::arena::HandleSpan<BorrowCallFact>,
) {
    let mut collection = BorrowCallCollection {
        program,
        machine,
        state,
        statement_index,
        call_ordinal,
        access_segments,
        argument_accesses,
        calls,
        state_calls,
    };

    match statement {
        StatementNode::Assignment(assignment) => {
            expression::collect_expression_borrow_calls(&mut collection, assignment.value)
        }
        StatementNode::Call(call) => {
            if statement_call_can_dispatch_to_machine(program, machine, state, call) {
                let receiver_path = statement_call_receiver_path(program, call);
                let accesses = collection.collect_call_argument_accesses(
                    program.statement_table.expression_handles(call.arguments),
                );
                collection.append_borrow_call(
                    call.receiver_symbol,
                    call.target_symbol,
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
