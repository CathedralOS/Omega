//! Value facts and scalar plans for the check pass.
//!
//! `build_value_facts` records a `CheckedValueFact` (type reference, primitive
//! type, and an integer range where one is known) for each expression it visits:
//! each machine's decrease-witness subjects, then its owned-data initializers,
//! then every statement of every state. `statement` and `transition` choose each
//! statement's expressions and roles; `expression` visits nested expressions.
//!
//! `scalar` builds the checked scalar expression plans, which the check pass
//! builds before flow, and the value computation plans, which it builds after
//! flow. `evaluation` evaluates selected scalar operations, `bounds` computes
//! integer bounds of selected scalar expressions, and `snapshots` reads the
//! current assigned value, literal, or integer bounds at a place.
use checked_trees::{
    CheckedIntegerRange, CheckedValueFact, CheckedValueFacts, CheckedValueOrigin,
    CheckedValueStatementRole,
};
use proof::obligations::ProofPlan;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;

pub(crate) mod bounds;
mod evaluation;
mod expression;
pub(crate) mod float_predicates;
mod snapshots;
pub(crate) use evaluation::evaluate as evaluate_checked_scalar;
pub(crate) use evaluation::{
    BoundScalarValues, ScalarValueSource, admitted_integer, integer_magnitude, integer_type,
    wrapping_cast_value,
};
pub(crate) use snapshots::{
    AssignmentObservation, PlaceScalarValues, integer_bounds_at_place,
    integer_bounds_from_payloads, is_assignment_literal, literal_assignment_payload,
    literal_at_place, prepare_assignment_observations, scalar_value_at_place,
    scalar_value_from_payloads,
};
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

#[cfg(test)]
pub(crate) use scalar::build_checked_scalar_computation_plans;
pub(crate) use scalar::{
    CallArrayConstruction, build_checked_scalar_expression_plans,
    build_checked_value_computation_plans, call_array_constructions,
    evaluate_closed_boolean_expression, exclusive_reference, is_scalar_return_call,
    lower_callable_parameter_range_requirements, lower_machine_entry_boolean_expression,
    lower_machine_entry_crash_contract_expression, lower_machine_parameter_boolean_expression,
    lower_operator_crash_contract_expression, lower_scalar_contract_predicate,
    lower_scalar_parameter_range_requirements, lower_scalar_parameter_refinement_predicates,
    lower_signature_crash_contract_expression, lower_state_interval_predicate,
    lower_state_scalar_contract_predicate, lower_state_scalar_expression,
    lower_unit_scalar_argument, nested_structural_call_return_type, occupies_scalar_position,
    operator_is_builtin, parameter_position, resolve_structural_parameter_path,
    retain_nested_structural_call_arguments, scalar_expression_type, shared_loan_place,
    structural_data, zeroed_scalar_local_initializer,
};

pub(crate) fn build_value_facts(
    program: &TypedTrees,
    proof_plan: &ProofPlan<'_>,
) -> CheckedValueFacts {
    collect_value_facts(
        program,
        Some(ValueTyping {
            proof_plan,
            assignment_ranges: proof::checker::AssignmentRangeContext::new(proof_plan),
        }),
    )
}

/// Every value row's expression and origin, in `build_value_facts` order,
/// without the type, primitive and proved range it attaches to each row.
/// Pre-flow operator selection reads nothing else, so it needs no proof plan.
pub(crate) fn build_value_origin_rows(program: &TypedTrees) -> CheckedValueFacts {
    collect_value_facts(program, None)
}

fn collect_value_facts<'program, 'plan>(
    program: &'program TypedTrees,
    typing: Option<ValueTyping<'program, 'plan>>,
) -> CheckedValueFacts {
    let mut builder = ValueFactBuilder {
        program,
        typing,
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
    /// Absent when only the rows' expressions and origins are wanted.
    typing: Option<ValueTyping<'program, 'plan>>,
    pub(super) facts: CheckedValueFacts,
}

/// What a typed value row needs beyond the traversal: the proof plan and
/// its assignment ranges for proved integer ranges.
struct ValueTyping<'program, 'plan> {
    proof_plan: &'plan ProofPlan<'program>,
    assignment_ranges: proof::checker::AssignmentRangeContext<'program>,
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
        let Some(typing) = &self.typing else {
            self.facts.values.insert(CheckedValueFact {
                expression,
                origin,
                type_reference: typed_trees::types::TypeReferenceHandle::invalid(),
                primitive_type: None,
                integer_range: None,
            });
            self.collect_expression_children(expression);
            return;
        };

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
                typing.proof_plan,
                machine_symbol,
                state_symbol,
                statement_index,
                &typing.assignment_ranges,
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
