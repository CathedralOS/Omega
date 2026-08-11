use psi_checked_trees::{
    CheckedIntegerRange, CheckedValueFact, CheckedValueFacts, CheckedValueOrigin,
    CheckedValueStatementRole,
};
use psi_proof::obligations::ProofPlan;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::ExpressionHandle;

mod expression;
mod scalar;
mod statement;
mod transition;

pub(crate) use scalar::{
    build_checked_scalar_expression_plans, lower_machine_parameter_boolean_expression,
};

pub(crate) fn build_value_facts(
    program: &TypedTrees,
    proof_plan: &ProofPlan<'_>,
) -> CheckedValueFacts {
    let mut builder = ValueFactBuilder {
        program,
        proof_plan,
        facts: CheckedValueFacts::default(),
    };

    for machine in program.machines() {
        if let Some(subjects) =
            psi_typed_trees::ranking::resolve_machine_witness_subjects(program, machine)
        {
            for (ordinal, expression) in subjects.into_iter().enumerate() {
                builder.collect_expression(
                    expression,
                    CheckedValueOrigin::MachineDecrease {
                        machine_symbol: machine.symbol,
                        ordinal,
                    },
                );
            }
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

pub(super) struct ValueFactBuilder<'program, 'plan> {
    pub(super) program: &'program TypedTrees,
    pub(super) proof_plan: &'plan ProofPlan<'program>,
    pub(super) facts: CheckedValueFacts,
}

impl ValueFactBuilder<'_, '_> {
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

        let type_reference = crate::operators::expression_type_reference_for_origin(
            self.program,
            expression,
            origin,
        )
        .unwrap_or_else(psi_typed_trees::types::TypeReferenceHandle::invalid);
        let integer_range = match origin {
            CheckedValueOrigin::StateStatement {
                machine_symbol,
                state_symbol,
                statement_index,
                role: CheckedValueStatementRole::AssignmentValue,
            } => psi_proof::checker::proved_assignment_integer_range(
                self.proof_plan,
                machine_symbol,
                state_symbol,
                statement_index,
            )
            .map(|range| CheckedIntegerRange {
                minimum: range.minimum,
                maximum: range.maximum,
            }),
            _ => None,
        };
        self.facts.values.insert(CheckedValueFact {
            expression,
            origin,
            type_reference,
            integer_range,
        });

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
