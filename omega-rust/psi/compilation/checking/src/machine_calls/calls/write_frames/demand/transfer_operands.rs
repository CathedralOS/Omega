//! Effects before a completing transition arrives at its callee. Ranking
//! compares entry coordinates with the prepared actuals: the callee's body
//! executes after that comparison. Ordinary statement frames still include it.
//! Strip only the resolved outer completing invocation; guards and nested
//! operand calls keep their complete caller-visible write frames.

use super::CallFrameResolver;
use facts::NormalizedWriteFrame;
use typed_trees::machine::Machine;
use typed_trees::statement::{StatementNode, StatementRootRole, TransitionExit};

impl<'program> CallFrameResolver<'program> {
    pub(crate) fn transition_operand_write_frame(
        &self,
        current_machine: &'program Machine,
        statement: &StatementNode,
    ) -> NormalizedWriteFrame {
        let StatementNode::Transition(transition) = statement else {
            return NormalizedWriteFrame::opaque();
        };
        if transition.exit != TransitionExit::Ordinary || transition.continuation.is_valid() {
            return NormalizedWriteFrame::opaque();
        }
        let Some(machine_symbols) = self.machine_symbols(current_machine) else {
            return NormalizedWriteFrame::opaque();
        };
        let mut expressions = Vec::new();
        for (role, expression) in self.program.tables.expression_roots(statement) {
            if role == StatementRootRole::TransitionValue
                && let Some(transfer) = crate::completing_call_transfer(self.program, expression)
                && let Some(call) = self.program.expression_table.call_node(transfer.expression)
            {
                // Resolution proves there is no runtime receiver and that the
                // actual roster is complete. Keep namespace receivers in the
                // traversal rather than guessing meaning from source spelling.
                if call.receiver.is_valid() {
                    expressions.push(call.receiver);
                }
                expressions.extend_from_slice(transfer.arguments);
            } else {
                expressions.push(expression);
            }
        }
        self.statement_expression_roots_write_frame(
            current_machine,
            &machine_symbols,
            statement,
            &expressions,
        )
    }
}
