use super::*;
use psi_checked_trees::CheckedValueStatementRole;
use psi_typed_trees::statement::{StatementNode, TransitionGuardNode};

impl ValueFactBuilder<'_, '_> {
    pub(super) fn collect_statement(
        &mut self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        statement: &StatementNode,
    ) {
        match statement {
            StatementNode::AssemblyFact(_) => {}
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
                let expected_primitives = if self
                    .program
                    .symbols
                    .builtin_function_symbol(psi_symbols::BuiltinFunction::AsmPortOut)
                    == Some(call.target_symbol)
                {
                    [
                        Some(psi_typed_trees::types::PrimitiveType::U16),
                        Some(psi_typed_trees::types::PrimitiveType::U8),
                    ]
                } else {
                    [None, None]
                };
                for (argument_index, argument) in self
                    .program
                    .statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                    .enumerate()
                {
                    self.collect_expression_with_expected_primitive(
                        argument,
                        psi_checked_trees::CheckedValueOrigin::StateStatement {
                            machine_symbol,
                            state_symbol,
                            statement_index,
                            role: CheckedValueStatementRole::CallArgument,
                        },
                        expected_primitives.get(argument_index).copied().flatten(),
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
