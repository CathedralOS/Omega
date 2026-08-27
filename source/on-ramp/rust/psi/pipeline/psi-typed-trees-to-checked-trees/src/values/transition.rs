use super::*;
use psi_checked_trees::CheckedValueStatementRole;
use psi_typed_trees::statement::{TransitionTargetHandle, TransitionTargetNode};

impl ValueFactBuilder<'_, '_> {
    pub(super) fn collect_transition_target(
        &mut self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        target: TransitionTargetHandle,
    ) {
        if !target.is_valid() {
            return;
        }

        match self.program.statement_table.transition_target(target) {
            TransitionTargetNode::Named { arguments, .. } => {
                for argument in self
                    .program
                    .statement_table
                    .expression_handles(*arguments)
                    .iter()
                    .copied()
                {
                    self.collect_statement_expression(
                        machine_symbol,
                        state_symbol,
                        statement_index,
                        argument,
                        CheckedValueStatementRole::TransitionTargetArgument,
                    );
                }
            }
            TransitionTargetNode::Value(expression) => {
                self.collect_statement_expression(
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    *expression,
                    CheckedValueStatementRole::TransitionTargetValue,
                );
            }
            TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
        }
    }
}
