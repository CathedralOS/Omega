use super::*;
use crate::borrow::accesses::collect_call_argument_accesses;
use crate::lookup::{
    statement_call_can_dispatch_to_machine, statement_call_receiver_path,
};
mod expression;

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
    match statement {
        StatementNode::Assignment(assignment) => expression::collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            assignment.value,
            access_segments,
            argument_accesses,
            calls,
            state_calls,
        ),
        StatementNode::Call(call) => {
            if statement_call_can_dispatch_to_machine(program, machine, state, call) {
                let receiver_path = statement_call_receiver_path(program, call);
                append_borrow_call(
                    calls,
                    state_calls,
                    statement_index,
                    *call_ordinal,
                    call.receiver_symbol,
                    call.target_symbol,
                    receiver_path.as_ref(),
                    collect_call_argument_accesses(
                        program,
                        access_segments,
                        argument_accesses,
                        program.statement_table.expression_handles(call.arguments),
                        machine.symbol,
                    ),
                );
                *call_ordinal += 1;
            }

            for argument in program.statement_table.expression_handles(call.arguments) {
                expression::collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    *argument,
                    access_segments,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        StatementNode::Expression(expression) => expression::collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            *expression,
            access_segments,
            argument_accesses,
            calls,
            state_calls,
        ),
        StatementNode::LocalData(local_data) => {
            if local_data.initial_value.is_valid() {
                expression::collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    local_data.initial_value,
                    access_segments,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(expression) = transition.guard {
                expression::collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    expression,
                    access_segments,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }

            collect_transition_target_borrow_calls(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
                transition.target,
                access_segments,
                argument_accesses,
                calls,
                state_calls,
            );

            if transition.continuation.is_valid() {
                collect_transition_target_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    transition.continuation,
                    access_segments,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
    }
}

fn collect_transition_target_borrow_calls(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    call_ordinal: &mut usize,
    target: omega_typed_trees::statement::TransitionTargetHandle,
    access_segments: &mut omega_core::arena::Arena<omega_facts::PlaceSegment>,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    calls: &mut omega_core::arena::Arena<BorrowCallFact>,
    state_calls: &mut omega_core::arena::HandleSpan<BorrowCallFact>,
) {
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, path } => {
            calls.append_to_span(
                state_calls,
                BorrowCallFact {
                    statement_index,
                    call_ordinal: *call_ordinal,
                    receiver_symbol: path.head_symbol,
                    target_symbol: path.symbol,
                    has_receiver: path.members.count() > 1,
                    accesses: collect_call_argument_accesses(
                        program,
                        access_segments,
                        argument_accesses,
                        program.statement_table.expression_handles(*arguments),
                        machine.symbol,
                    ),
                },
            );
            *call_ordinal += 1;

            for argument in program.statement_table.expression_handles(*arguments) {
                expression::collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    *argument,
                    access_segments,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        TransitionTargetNode::Value(expression) => expression::collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            *expression,
            access_segments,
            argument_accesses,
            calls,
            state_calls,
        ),
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}

fn append_borrow_call(
    calls: &mut omega_core::arena::Arena<BorrowCallFact>,
    state_calls: &mut omega_core::arena::HandleSpan<BorrowCallFact>,
    statement_index: usize,
    call_ordinal: usize,
    receiver_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
    receiver_path: Option<&NamePath>,
    accesses: omega_core::arena::HandleSpan<BorrowArgumentAccessFact>,
) {
    calls.append_to_span(
        state_calls,
        BorrowCallFact {
            statement_index,
            call_ordinal,
            receiver_symbol,
            target_symbol,
            has_receiver: receiver_path.is_some(),
            accesses,
        },
    );
}
