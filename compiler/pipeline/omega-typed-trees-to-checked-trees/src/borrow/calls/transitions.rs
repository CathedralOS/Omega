use super::*;

pub(super) fn collect_transition_target_borrow_calls(
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
                        state.symbol,
                        statement_index,
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
