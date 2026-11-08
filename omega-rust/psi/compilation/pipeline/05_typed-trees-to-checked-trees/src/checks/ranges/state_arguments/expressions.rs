use typed_trees::expression::{ExpressionHandle, TableCallExpression};

use super::StateArgumentFacts;
use super::calls::collect_state_argument_facts_for_call;
use crate::checks::ranges::expression_transfer::{ExpressionTransfer, ExpressionTransferSink};
use crate::checks::ranges::facts::RangeFacts;

pub(super) fn collect_state_argument_facts_from_expression(
    context: &ExpressionTransfer<'_, '_>,
    facts: &mut RangeFacts<'_>,
    expression: ExpressionHandle,
    collected: &mut Vec<StateArgumentFacts>,
) {
    let transfer = ExpressionTransfer {
        program: context.program,
        machine: context.machine,
        state: context.state,
        call_frames: context.call_frames,
    };
    let mut sink = CallArguments { context, collected };
    transfer.transfer(facts, expression, &mut sink);
}

struct CallArguments<'context, 'program, 'frames> {
    context: &'context ExpressionTransfer<'program, 'frames>,
    collected: &'context mut Vec<StateArgumentFacts>,
}

impl ExpressionTransferSink for CallArguments<'_, '_, '_> {
    fn checks_indexed(&self) -> bool {
        false
    }

    fn call_entry(&mut self, facts: &RangeFacts<'_>, call: &TableCallExpression) {
        collect_state_argument_facts_for_call(
            self.context.program,
            self.context.machine,
            self.context.state,
            facts,
            call.target_symbol,
            self.context
                .program
                .expression_table
                .expression_handles(call.arguments),
            false,
            self.collected,
        );
    }
}
