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
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;

/// Lower a contract predicate in the selected machine's entry-parameter
/// namespace. Crash contracts use this to retain the same checked scalar
/// meaning as executable guards without carrying typed-tree handles into the
/// terminal producer.
pub(crate) fn lower_machine_parameter_boolean_expression(
    program: &TypedTrees,
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
    let scalar_parameters = parameters
        .iter()
        .filter(|parameter| crate::values::scalar::occupies_scalar_position(program, parameter))
        .cloned()
        .collect::<Vec<_>>();
    let parameter_types = scalar_parameters
        .iter()
        .map(|parameter| program.primitive_type_reference(parameter.type_reference))
        .collect::<Option<Vec<_>>>()?;
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
