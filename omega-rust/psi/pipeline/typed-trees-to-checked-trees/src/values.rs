use checked_trees::{
    CheckedIntegerRange, CheckedValueFact, CheckedValueFacts, CheckedValueOrigin,
    CheckedValueStatementRole,
};
use proof::obligations::ProofPlan;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;

mod evaluation;
mod expression;
mod snapshots;
pub(crate) use evaluation::BoundScalarValues;
pub(crate) use evaluation::evaluate as evaluate_checked_scalar;
pub(crate) use snapshots::{literal_at_place, scalar_value_at_place};
mod scalar;
mod statement;
mod transition;

/// Owned mutable parameters use current-state scalar storage, never reference
/// rebinding or the immutable incoming-parameter expression namespace.
pub(crate) fn mutable_scalar_parameter_type(
    program: &TypedTrees,
    parameter: &typed_trees::signature::StateParameter,
) -> Option<typed_trees::types::PrimitiveType> {
    use typed_trees::types::PrimitiveType;
    if !parameter.is_mutable
        || parameter.is_self
        || parameter.is_const
        || !parameter.symbol.is_valid()
    {
        return None;
    }
    let primitive = program.primitive_type_reference(parameter.type_reference)?;
    matches!(
        primitive,
        PrimitiveType::Bool
            | PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    )
    .then_some(primitive)
}

pub(crate) use scalar::{
    build_checked_scalar_computation_plans, build_checked_scalar_expression_plans,
    lower_integer_contract_predicate, lower_integer_parameter_range_requirements,
    lower_machine_entry_boolean_expression, lower_machine_entry_scalar_contract_expression,
    lower_machine_parameter_boolean_expression, lower_state_scalar_expression,
    lower_unit_scalar_argument, nested_structural_call_return_type,
    retain_nested_structural_call_arguments, scalar_expression_type,
};

pub(crate) fn build_value_facts(
    program: &TypedTrees,
    proof_plan: &ProofPlan<'_>,
) -> CheckedValueFacts {
    let assignment_ranges = proof::checker::AssignmentRangeContext::new(proof_plan);
    let mut builder = ValueFactBuilder {
        program,
        proof_plan,
        assignment_ranges,
        facts: CheckedValueFacts::default(),
    };

    for machine in program.machines() {
        if let Some(subjects) =
            typed_trees::ranking::resolve_machine_witness_subjects(program, machine)
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
    pub(super) assignment_ranges: proof::checker::AssignmentRangeContext<'program>,
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
        self.collect_expression_with_expected_primitive(expression, origin, None);
    }

    pub(super) fn collect_expression_with_expected_primitive(
        &mut self,
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
        expected_primitive: Option<typed_trees::types::PrimitiveType>,
    ) {
        if !expression.is_valid() {
            return;
        }

        let type_reference = crate::operators::expression_type_reference_for_origin(
            self.program,
            expression,
            origin,
        )
        .unwrap_or_else(typed_trees::types::TypeReferenceHandle::invalid);
        let primitive_type = self
            .program
            .primitive_type_reference(type_reference)
            .or(expected_primitive);
        let integer_range = match (
            origin,
            self.program.expression_table.expression(expression),
            expected_primitive,
        ) {
            (
                CheckedValueOrigin::StateStatement {
                    role: CheckedValueStatementRole::CallArgument,
                    ..
                },
                typed_trees::expression::ExpressionNode::Integer(literal),
                Some(_),
            ) => literal.value_bignum().map(|value| CheckedIntegerRange {
                minimum: value.clone(),
                maximum: value,
            }),
            (
                CheckedValueOrigin::StateStatement {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    role: CheckedValueStatementRole::AssignmentValue,
                },
                _,
                _,
            ) => proof::checker::proved_assignment_integer_range_with_context(
                self.proof_plan,
                machine_symbol,
                state_symbol,
                statement_index,
                &self.assignment_ranges,
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
            primitive_type,
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
