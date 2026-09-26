use typed_trees::statement::{StatementNode, TransitionTargetHandle, TransitionTargetNode};

use super::calls::collect_state_argument_facts_for_call;
use super::expressions::collect_state_argument_facts_from_expression;
use super::{StateArgumentContext, StateArgumentFacts};
use crate::checks::ranges::facts::RangeFacts;
use crate::checks::ranges::statement_transfer::{StatementTransferSink, transfer_statement_facts};
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::TableCall;

/// Replay one statement's fact transfer through the SHARED statement
/// transfer, collecting the argument facts each outgoing call/transition
/// edge carries. The collection pass deliberately runs the same transfer
/// the checking pass runs — a seed missing here (an ensured call result,
/// a name alias, a member store, a subslice window, a guard's declared
/// endpoint) silently weakens the merged parameter facts.
pub(super) fn collect_state_argument_facts_from_statement<'program>(
    context: &StateArgumentContext<'program, '_>,
    facts: &mut RangeFacts<'_>,
    statement: &'program StatementNode,
    collected: &mut Vec<StateArgumentFacts>,
) {
    struct CollectSink<'a, 'program, 'frames> {
        context: &'a StateArgumentContext<'program, 'frames>,
        collected: &'a mut Vec<StateArgumentFacts>,
    }
    impl<'program> StatementTransferSink<'program> for CollectSink<'_, 'program, '_> {
        fn visit_expression(&mut self, facts: &mut RangeFacts<'_>, expression: ExpressionHandle) {
            collect_state_argument_facts_from_expression(
                self.context,
                facts,
                expression,
                self.collected,
            );
        }
        fn visit_call(&mut self, facts: &mut RangeFacts<'_>, call: &'program TableCall) {
            // The callee's argument facts see the caller's seeded facts but
            // not the call's own write retirement — the transfer orders this
            // hook between the argument visits and `invalidate_call_writes`.
            collect_state_argument_facts_for_call(
                self.context.program,
                self.context.machine,
                self.context.state,
                facts,
                call.target_symbol,
                self.context
                    .program
                    .statement_table
                    .expression_handles(call.arguments),
                false,
                self.collected,
            );
        }
        fn visit_transition_target(
            &mut self,
            facts: &mut RangeFacts<'_>,
            target: TransitionTargetHandle,
        ) {
            collect_state_argument_facts_from_target(self.context, facts, target, self.collected);
        }
    }
    let mut sink = CollectSink { context, collected };
    transfer_statement_facts(
        context.program,
        context.machine,
        context.state,
        context.call_frames,
        facts,
        statement,
        &mut sink,
    );
}

fn collect_state_argument_facts_from_target(
    context: &StateArgumentContext<'_, '_>,
    facts: &mut RangeFacts<'_>,
    target: TransitionTargetHandle,
    collected: &mut Vec<StateArgumentFacts>,
) {
    let program = context.program;
    let machine = context.machine;
    if !target.is_valid() {
        return;
    }

    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            path, arguments, ..
        } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                collect_state_argument_facts_from_expression(context, facts, *argument, collected);
            }
            let Some(target_state) = program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == path.symbol)
            else {
                return;
            };

            collect_state_argument_facts_for_call(
                program,
                machine,
                context.state,
                facts,
                target_state.symbol,
                program.statement_table.expression_handles(*arguments),
                true,
                collected,
            );
        }
        TransitionTargetNode::Value(value) => {
            collect_state_argument_facts_from_expression(context, facts, *value, collected);
        }
        TransitionTargetNode::SelfTarget => {
            collect_state_argument_facts_for_call(
                program,
                machine,
                context.state,
                facts,
                context.state.symbol,
                &[],
                true,
                collected,
            );
        }
        TransitionTargetNode::Terminal => {}
    }
}
