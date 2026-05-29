use super::*;
use omega_checked_trees::CheckedValueStatementRole;
use omega_typed_trees::statement::{StatementNode, TransitionGuardNode};

impl ValueFactBuilder<'_> {
    pub(super) fn collect_statement(
        &mut self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        statement: &StatementNode,
    ) {
        match statement {
            StatementNode::Assignment(assignment) => {
                self.collect_statement_expression(
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    assignment.target,
                    CheckedValueStatementRole::AssignmentTargetSubexpression,
                );
                self.collect_statement_expression(
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    assignment.value,
                    CheckedValueStatementRole::AssignmentValue,
                );
            }
            StatementNode::Call(call) => {
                for argument in self
                    .program
                    .statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                {
                    self.collect_statement_expression(
                        machine_symbol,
                        state_symbol,
                        statement_index,
                        argument,
                        CheckedValueStatementRole::CallArgument,
                    );
                }
            }
            StatementNode::Expression(expression) => {
                self.collect_statement_expression(
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    *expression,
                    CheckedValueStatementRole::Expression,
                );
            }
            StatementNode::LocalData(local_data) => {
                if local_data.initial_value.is_valid() {
                    self.collect_statement_expression(
                        machine_symbol,
                        state_symbol,
                        statement_index,
                        local_data.initial_value,
                        CheckedValueStatementRole::LocalInitializer,
                    );
                }
            }
            StatementNode::Transition(transition) => {
                if let TransitionGuardNode::When(expression) = transition.guard {
                    self.collect_statement_expression(
                        machine_symbol,
                        state_symbol,
                        statement_index,
                        expression,
                        CheckedValueStatementRole::TransitionGuard,
                    );
                }
                self.collect_transition_target(
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    transition.target,
                );
                self.collect_transition_target(
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    transition.continuation,
                );
            }
        }
    }
}
