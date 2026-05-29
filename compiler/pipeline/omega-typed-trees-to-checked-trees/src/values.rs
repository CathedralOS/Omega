use omega_checked_trees::{
    CheckedValueFact, CheckedValueFacts, CheckedValueOrigin, CheckedValueStatementRole,
};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::statement::{
    StatementNode, TransitionGuardNode, TransitionTargetHandle, TransitionTargetNode,
};

pub(crate) fn build_value_facts(program: &TypedTrees) -> CheckedValueFacts {
    let mut builder = ValueFactBuilder {
        program,
        facts: CheckedValueFacts::default(),
    };

    for machine in program.machines() {
        for (ordinal, expression) in program
            .expression_table
            .expression_handles(machine.decreases)
            .iter()
            .copied()
            .enumerate()
        {
            builder.collect_expression(
                expression,
                CheckedValueOrigin::MachineDecrease {
                    machine_symbol: machine.symbol,
                    ordinal,
                },
            );
        }

        for owned_data in program.machine_owned_data(machine) {
            if owned_data.initial_value.is_valid() {
                builder.collect_expression(
                    owned_data.initial_value,
                    CheckedValueOrigin::MachineOwnedDataInitializer {
                        machine_symbol: machine.symbol,
                        data_symbol: owned_data.symbol,
                    },
                );
            }
        }

        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                builder.collect_statement(machine.symbol, state.symbol, statement_index, statement);
            }
        }
    }

    builder.facts
}

struct ValueFactBuilder<'program> {
    program: &'program TypedTrees,
    facts: CheckedValueFacts,
}

impl ValueFactBuilder<'_> {
    fn collect_statement(
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

    fn collect_transition_target(
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

    fn collect_statement_expression(
        &mut self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
        role: CheckedValueStatementRole,
    ) {
        self.collect_expression(
            expression,
            CheckedValueOrigin::StateStatement {
                machine_symbol,
                state_symbol,
                statement_index,
                role,
            },
        );
    }

    fn collect_expression(&mut self, expression: ExpressionHandle, origin: CheckedValueOrigin) {
        if !expression.is_valid() {
            return;
        }

        self.facts
            .values
            .insert(CheckedValueFact { expression, origin });

        match self.program.expression_table.expression(expression) {
            ExpressionNode::ArrayLiteral(values) => {
                for value in self
                    .program
                    .expression_table
                    .expression_handles(*values)
                    .iter()
                    .copied()
                {
                    self.collect_nested_expression(expression, value);
                }
            }
            ExpressionNode::Binary(binary) => {
                self.collect_nested_expression(expression, binary.left);
                self.collect_nested_expression(expression, binary.right);
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_) => {}
            ExpressionNode::Cast(cast) => {
                self.collect_nested_expression(expression, cast.value);
            }
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    self.collect_nested_expression(expression, call.receiver);
                }
                for argument in self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                {
                    self.collect_nested_expression(expression, argument);
                }
            }
            ExpressionNode::Indexed(indexed) => {
                self.collect_nested_expression(expression, indexed.collection);
                self.collect_nested_expression(expression, indexed.index);
            }
            ExpressionNode::Member(member) => {
                self.collect_nested_expression(expression, member.receiver);
            }
            ExpressionNode::Mutable(value) => {
                self.collect_nested_expression(expression, *value);
            }
            ExpressionNode::Range(range) => {
                self.collect_nested_expression(expression, range.start);
                self.collect_nested_expression(expression, range.end);
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in self.program.expression_table.struct_fields(literal.fields) {
                    self.collect_nested_expression(expression, field.value);
                }
            }
        }
    }

    fn collect_nested_expression(
        &mut self,
        parent: ExpressionHandle,
        expression: ExpressionHandle,
    ) {
        self.collect_expression(expression, CheckedValueOrigin::NestedExpression { parent });
    }
}
