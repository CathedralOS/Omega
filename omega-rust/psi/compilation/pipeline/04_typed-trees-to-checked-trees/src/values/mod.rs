//! Value facts and scalar plans for the check pass.
//!
//! `build_value_facts` records a `CheckedValueFact` (type reference, primitive
//! type, and an integer range where one is known) for each expression it visits:
//! each executable occurrence visited by validation. That shared semantic owner
//! chooses expression roles and traversal order without producing proof facts;
//! this owner attaches the checked type and range evidence.
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

pub(crate) mod bounds;
mod evaluation;
pub(crate) mod float_predicates;
pub(crate) mod reference_sources;
mod snapshots;
pub(crate) use evaluation::evaluate as evaluate_checked_scalar;
pub(crate) use evaluation::{
    BoundScalarValues, ScalarValueSource, admitted_integer, integer_magnitude, integer_type,
};
pub(crate) use snapshots::{
    AssignmentObservation, PlaceScalarValues, integer_bounds_at_place,
    integer_bounds_from_payloads, is_assignment_literal, literal_assignment_payload,
    literal_at_place, prepare_assignment_observations, scalar_value_at_place,
    scalar_value_from_payloads,
};
mod scalar;
pub(crate) use scalar::subslice_endpoints;

/// Owned mutable parameters use current-state scalar storage, never reference
/// rebinding or the immutable incoming-parameter expression namespace.
pub(crate) fn mutable_scalar_parameter_type(
    program: &dyn ::validation::CheckingInput,
    parameter: &typed_trees::signature::StateParameter,
) -> Option<typed_trees::types::PrimitiveType> {
    use typed_trees::types::PrimitiveType;
    if (!parameter.is_mutable) || parameter.is_const || !parameter.symbol.is_valid() {
        return None;
    }
    let primitive = program.primitive_type_reference(parameter.type_reference)?;
    (matches!(primitive, PrimitiveType::Bool) || primitive.is_fixed_integer()).then_some(primitive)
}

pub(crate) use scalar::{
    assignment_target_primitive_type, build_checked_scalar_expression_plans,
    build_checked_value_computation_plans, evaluate_closed_boolean_expression, exclusive_reference,
    is_scalar_return_call, landed_for_primitive, lower_callable_parameter_range_requirements,
    lower_callable_scalar_contract_predicate, lower_entry_predicate_with_subjects,
    lower_entry_scalar_with_subjects, lower_machine_entry_boolean_expression,
    lower_machine_entry_crash_contract_expression, lower_machine_parameter_boolean_expression,
    lower_operator_crash_contract_expression, lower_parameter_boolean_expression,
    lower_scalar_contract_predicate, lower_scalar_parameter_range_requirements,
    lower_scalar_parameter_refinement_predicates, lower_signature_crash_contract_expression,
    lower_state_interval_predicate, lower_state_scalar_contract_predicate,
    lower_state_scalar_expression, lower_unit_scalar_argument,
    lower_unit_scalar_argument_with_places, nested_structural_call_return_type,
    occupies_scalar_position, operator_is_builtin, parameter_position,
    resolve_structural_parameter_path, retain_nested_structural_call_arguments,
    scalar_expression_type, structural_data, structural_parameter_place,
    unit_scalar_binding_symbols, zeroed_scalar_local_initializer,
};

pub(crate) fn build_value_facts(
    program: &dyn ::validation::CheckingInput,
    proof_plan: &ProofPlan<'_>,
) -> CheckedValueFacts {
    let assignment_ranges = proof::checker::AssignmentRangeContext::new(proof_plan);
    let mut facts = CheckedValueFacts::default();
    validation::visit_executable_value_occurrences(
        program,
        &typed_trees::references::DeclarationSelection::all(),
        &mut |expression, origin, expected_primitive| {
            let type_reference =
                validation::expression_type_reference_for_origin(program, expression, origin)
                    .unwrap_or_else(typed_trees::types::TypeReferenceHandle::invalid);
            let primitive_type = program
                .primitive_type_reference(type_reference)
                .or(expected_primitive);
            let integer_range = match (
                origin,
                program.expression_table.expression(expression),
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
                    proof_plan,
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    &assignment_ranges,
                )
                .map(|range| CheckedIntegerRange {
                    minimum: range.minimum,
                    maximum: range.maximum,
                }),
                _ => None,
            };
            facts.values.insert(CheckedValueFact {
                expression,
                origin,
                type_reference,
                primitive_type,
                integer_range,
            });
        },
    );
    facts
}
