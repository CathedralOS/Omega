//! Lowering the Boolean expressions that machine parameters admit.
//!
//! A predicate that reads a structural parameter lowers through `structural`
//! in the entry's authored parameter namespace. Every other predicate lowers
//! through the shared scalar `lower_boolean_expression` over the entry's
//! dense scalar parameters.

mod structural;
mod structural_equality;
mod structural_paths;

use crate::values::scalar::boolean_lowering::lower_boolean_expression;
use checked_trees::{CheckedBooleanExpression, CheckedOperatorFacts};
use typed_trees::expression::ExpressionHandle;

/// Lower a contract predicate in the selected machine's entry-parameter
/// namespace. Crash contracts use this to retain the same checked scalar
/// meaning as executable guards without carrying typed-tree handles into the
/// terminal producer.
pub(crate) fn lower_machine_parameter_boolean_expression(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
    expression: ExpressionHandle,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    let entry = program.machine_states(machine).first()?;
    let parameters = program.state_parameters(entry);

    if let Some(structural) = structural::lower(program, machine, operators, parameters, expression)
    {
        return Some(structural);
    }
    // A machine parameter without a primitive carrier keeps the scalar path
    // unreachable: those predicates lower through
    // `structural::lower` instead, and an address-typed
    // equality must never gain a fixed-integer crash meaning.
    parameters
        .iter()
        .map(|parameter| program.primitive_type_reference(parameter.type_reference))
        .collect::<Option<Vec<_>>>()?;
    // `lower_boolean_expression` resolves `CheckedScalarExpression::Parameter`
    // positions against `parameters`, which lowering consumers read as the
    // dense retained-scalar roster; `[erased]` and structural bindings must
    // stay out of it so their references fall through to the erased/structural
    // branches that still see the authored roster.
    let (scalar_parameters, parameter_types) = scalar_parameters_and_types(program, parameters)?;
    lower_boolean_expression(
        program,
        operators,
        expression,
        &scalar_parameters,
        parameters,
        &parameter_types,
        &[],
        exact_integer_casts,
    )
}

/// Lower one predicate in its authored parameter namespace. Call binding may
/// instantiate the parameter types, but never replaces authored symbols or
/// manufactures an executable state for a bodyless requirement.
pub(crate) fn lower_parameter_boolean_expression(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    owner: symbols::SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    expression: ExpressionHandle,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    if !validation::has_builtin_parameter_bound_expression_meaning(
        program, owner, parameters, expression,
    ) {
        return None;
    }
    let (scalar_parameters, parameter_types) = scalar_parameters_and_types(program, parameters)?;
    lower_boolean_expression(
        program,
        operators,
        expression,
        &scalar_parameters,
        parameters,
        &parameter_types,
        &[],
        exact_integer_casts,
    )
}

/// The retained-scalar parameter roster and each element's primitive type.
fn scalar_parameters_and_types(
    program: &dyn ::validation::CheckingInput,
    parameters: &[typed_trees::signature::StateParameter],
) -> Option<(
    Vec<typed_trees::signature::StateParameter>,
    Vec<typed_trees::types::PrimitiveType>,
)> {
    let scalar_parameters = parameters
        .iter()
        .filter(|parameter| crate::values::scalar::occupies_scalar_position(program, parameter))
        .cloned()
        .collect::<Vec<_>>();
    let parameter_types = scalar_parameters
        .iter()
        .map(|parameter| program.primitive_type_reference(parameter.type_reference))
        .collect::<Option<Vec<_>>>()?;
    Some((scalar_parameters, parameter_types))
}

/// Lower a scalar in an authored parameter telescope without manufacturing an
/// executable state. Proof aliases and abstract callable contracts share this
/// checked arithmetic and operator-selection path.
pub(crate) fn lower_parameter_scalar_expression(
    program: &dyn validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    owner: symbols::SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    expression: ExpressionHandle,
    primitive: typed_trees::types::PrimitiveType,
) -> Option<checked_trees::CheckedScalarExpression> {
    if !validation::has_builtin_parameter_bound_expression_meaning(
        program, owner, parameters, expression,
    ) {
        return None;
    }
    let (scalar_parameters, parameter_types) = scalar_parameters_and_types(program, parameters)?;
    super::scalar_lowering::lower_return_expression(
        program,
        operators,
        expression,
        &scalar_parameters,
        parameters,
        &parameter_types,
        &[],
        primitive,
        &[],
    )
}
