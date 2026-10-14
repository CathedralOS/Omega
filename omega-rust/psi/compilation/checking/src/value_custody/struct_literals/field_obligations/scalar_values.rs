//! One scalar field check owns arithmetic, semantic landing and value bounds.
//! Ordinary and layout-bearing records consume this same decision. The facts
//! come from the field's evaluation occurrence, not a replay of initializers or
//! the environment after another field has already mutated its inputs.

use super::super::observations::ConstructionObservations;
use crate::proof_contracts::arithmetic_domains::{
    Interval, ValueEnvironment, check_value_narrowing, immutable_integer_expression_interval,
    validate_value_range,
};
use diagnostics::Diagnostic;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

pub(super) struct ConstructionScalarValue {
    pub(super) interval: Interval,
    pub(super) primitive: Option<PrimitiveType>,
    pub(super) environment: ValueEnvironment,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    observations: &ConstructionObservations<'_>,
    expression: ExpressionHandle,
    field_type: TypeReferenceHandle,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ConstructionScalarValue> {
    let target = program.primitive_type_reference(field_type)?;
    if let Some(dispatch) = program.expression_table.match_node(expression) {
        let before = diagnostics.len();
        let mut combined: Option<ConstructionScalarValue> = None;
        let mut complete = true;
        for arm in program.expression_table.match_arms(dispatch.arms) {
            let Some(value) = validate(
                program,
                machine,
                state,
                observations,
                arm.value,
                field_type,
                owner,
                diagnostics,
            ) else {
                complete = false;
                continue;
            };
            combined = Some(match combined {
                Some(prior) => ConstructionScalarValue {
                    interval: prior.interval.union(value.interval),
                    primitive: Some(target),
                    environment: prior.environment.join(&value.environment),
                },
                None => value,
            });
        }
        return (complete && diagnostics.len() == before)
            .then_some(combined)
            .flatten();
    }

    let environment = observations.environment(expression);
    let before = diagnostics.len();
    let (interval, primitive, _) = validate_value_range(
        program,
        machine,
        Some(state),
        expression,
        &environment,
        Some(target),
        numerics::arithmetic::ArithmeticDomain::Exact,
        owner,
        diagnostics,
    );
    if diagnostics.len() != before {
        return None;
    }
    check_value_narrowing(
        program,
        machine,
        Some(state),
        expression,
        target,
        program.arithmetic_domain_for_type_reference(field_type),
        &environment,
        owner,
        diagnostics,
    );
    if diagnostics.len() != before {
        return None;
    }
    let interval = immutable_integer_expression_interval(program, machine, state, expression)
        .map_or(interval, |standing| interval.intersect(standing));
    Some(ConstructionScalarValue {
        interval,
        primitive,
        environment,
    })
}
