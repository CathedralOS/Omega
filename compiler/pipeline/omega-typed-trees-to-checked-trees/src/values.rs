use omega_checked_trees::{
    CheckedValueFact, CheckedValueFacts, CheckedValueOrigin, CheckedValueStatementRole,
};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::ExpressionHandle;

mod expression;
mod statement;
mod transition;

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

pub(super) struct ValueFactBuilder<'program> {
    pub(super) program: &'program TypedTrees,
    pub(super) facts: CheckedValueFacts,
}

impl ValueFactBuilder<'_> {
    pub(super) fn collect_statement_expression(
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

    pub(super) fn collect_expression(
        &mut self,
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
    ) {
        if !expression.is_valid() {
            return;
        }

        self.facts
            .values
            .insert(CheckedValueFact { expression, origin });

        self.collect_expression_children(expression);
    }

    pub(super) fn collect_nested_expression(
        &mut self,
        parent: ExpressionHandle,
        expression: ExpressionHandle,
    ) {
        self.collect_expression(expression, CheckedValueOrigin::NestedExpression { parent });
    }
}
