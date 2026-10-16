use super::SymbolHandle;
use super::ValueOccurrenceVisitor;
use checked_trees::CheckedValueStatementRole;
use typed_trees::statement::{StatementNode, TransitionGuardNode};

impl ValueOccurrenceVisitor<'_, '_> {
    pub(super) fn collect_statement(
        &mut self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        statement: &StatementNode,
    ) {
        match statement {
            StatementNode::RootBinding(binding) => {
                self.collect_statement_expression(
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    binding.receiver,
                    CheckedValueStatementRole::Expression,
                );
                if binding.implementation_operand.is_valid() {
                    self.collect_statement_expression(
                        machine_symbol,
                        state_symbol,
                        statement_index,
                        binding.implementation_operand,
                        CheckedValueStatementRole::Expression,
                    );
                }
            }
            StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
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
                let register_operation = self
                    .program
                    .symbols
                    .builtin_function_for_symbol(call.target_symbol)
                    .and_then(|builtin| {
                        language_core::inline_assembly::AsmRegisterOperation::from_intrinsic_name(
                            builtin.name(),
                        )
                    });
                for (argument_index, argument) in self
                    .program
                    .statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                    .enumerate()
                {
                    self.collect_expression(
                        argument,
                        checked_trees::CheckedValueOrigin::StateStatement {
                            machine_symbol,
                            state_symbol,
                            statement_index,
                            role: CheckedValueStatementRole::CallArgument,
                        },
                        register_operation
                            .and_then(|operation| operation.input_carriers().get(argument_index))
                            .and_then(|carrier| {
                                typed_trees::types::PrimitiveType::from_name(carrier.name())
                            }),
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
                for target in transition.targets() {
                    self.collect_transition_target(
                        machine_symbol,
                        state_symbol,
                        statement_index,
                        target,
                    );
                }
            }
        }
    }
}
