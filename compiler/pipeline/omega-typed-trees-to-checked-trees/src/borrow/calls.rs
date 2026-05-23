use super::*;
use crate::borrow::accesses::collect_call_argument_accesses;
use crate::lookup::{
    call_receiver_parts, receiver_can_dispatch_to_machine, resolve_state_call_target,
    statement_call_can_dispatch_to_machine, statement_call_receiver_path,
};

pub(crate) fn collect_statement_borrow_calls(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    statement: &StatementNode,
    call_ordinal: &mut usize,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    calls: &mut omega_core::arena::Arena<BorrowCallFact>,
    state_calls: &mut omega_core::arena::HandleSpan<BorrowCallFact>,
) {
    match statement {
        StatementNode::Assignment(assignment) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            assignment.value,
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
                        argument_accesses,
                        &program.expression_table,
                        program.statement_table.expression_handles(call.arguments),
                        machine.symbol,
                    ),
                );
                *call_ordinal += 1;
            }

            for argument in program.statement_table.expression_handles(call.arguments) {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    *argument,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        StatementNode::Expression(expression) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            *expression,
            argument_accesses,
            calls,
            state_calls,
        ),
        StatementNode::LocalData(local_data) => {
            if local_data.initial_value.is_valid() {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    local_data.initial_value,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(expression) = transition.guard {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    expression,
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
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    calls: &mut omega_core::arena::Arena<BorrowCallFact>,
    state_calls: &mut omega_core::arena::HandleSpan<BorrowCallFact>,
) {
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    *argument,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        TransitionTargetNode::Value(expression) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            *expression,
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

fn collect_expression_borrow_calls(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    call_ordinal: &mut usize,
    expression: ExpressionHandle,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    calls: &mut omega_core::arena::Arena<BorrowCallFact>,
    state_calls: &mut omega_core::arena::HandleSpan<BorrowCallFact>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    *value,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_borrow_calls(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
                binary.left,
                argument_accesses,
                calls,
                state_calls,
            );
            collect_expression_borrow_calls(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
                binary.right,
                argument_accesses,
                calls,
                state_calls,
            );
        }
        ExpressionNode::Call(call) => {
            let (receiver_symbol, receiver_path) = call_receiver_parts(program, call.receiver);
            let is_machine_call = resolve_state_call_target(
                program,
                machine,
                state,
                receiver_symbol,
                call.target_symbol,
                receiver_path.as_deref(),
                &call.target,
            )
            .is_valid()
                || receiver_can_dispatch_to_machine(
                    program,
                    machine,
                    state,
                    receiver_symbol,
                    receiver_path.as_deref(),
                );

            if is_machine_call {
                append_borrow_call(
                    calls,
                    state_calls,
                    statement_index,
                    *call_ordinal,
                    receiver_symbol,
                    call.target_symbol,
                    receiver_path.as_ref(),
                    collect_call_argument_accesses(
                        argument_accesses,
                        &program.expression_table,
                        program.expression_table.expression_handles(call.arguments),
                        machine.symbol,
                    ),
                );
                *call_ordinal += 1;
            }

            if call.receiver.is_valid() {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    call.receiver,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    *argument,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        ExpressionNode::Cast(cast) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            cast.value,
            argument_accesses,
            calls,
            state_calls,
        ),
        ExpressionNode::Indexed(indexed) => {
            collect_expression_borrow_calls(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
                indexed.collection,
                argument_accesses,
                calls,
                state_calls,
            );
            collect_expression_borrow_calls(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
                indexed.index,
                argument_accesses,
                calls,
                state_calls,
            );
        }
        ExpressionNode::Member(member) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            member.receiver,
            argument_accesses,
            calls,
            state_calls,
        ),
        ExpressionNode::Mutable(inner_expression) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            *inner_expression,
            argument_accesses,
            calls,
            state_calls,
        ),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program.expression_table.struct_fields(struct_literal.fields) {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    field.value,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}
