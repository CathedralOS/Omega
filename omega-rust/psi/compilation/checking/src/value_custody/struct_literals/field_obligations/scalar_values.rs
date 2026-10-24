//! One scalar field check owns arithmetic, semantic landing and value bounds.
//! Ordinary and layout-bearing records consume this same decision. The facts
//! first establish every receiving obligation independently of flow; otherwise
//! they come from the field's evaluation occurrence, not initializer replay or
//! the environment after another field has already mutated its inputs.

use super::super::observations::ConstructionObservations;
use crate::proof_contracts::arithmetic_domains::{
    ArithmeticDecisions, Interval, ValueEnvironment, check_validated_value_narrowing,
    immutable_integer_expression_interval, validate_value_range_retaining_decisions,
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

/// Validate the complete receiving operation before requesting occurrence flow.
/// A successful independent attempt has no provisional diagnostics or pending
/// formations, and its receiver has already checked every range/physical target.
/// Failed attempts publish nothing; the same body then uses exact observations.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_scalar_receiving<Output>(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    observations: &ConstructionObservations<'_>,
    expression: ExpressionHandle,
    field_type: TypeReferenceHandle,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    receiving: impl Fn(Option<&ConstructionScalarValue>, &mut Vec<Diagnostic>) -> Output,
) -> Output {
    let mut independent_diagnostics = Vec::new();
    let mut independent_formations = Vec::new();
    let independent = analyze_scalar_value(
        program,
        machine,
        state,
        &|_| ValueEnvironment::new(),
        expression,
        field_type,
        owner,
        &mut independent_diagnostics,
        &mut independent_formations,
    );
    let output = receiving(independent.as_ref(), &mut independent_diagnostics);
    if independent.is_some()
        && independent_diagnostics.is_empty()
        && independent_formations.is_empty()
    {
        return output;
    }
    // The receiver returns construction scratch (currently physical-write rows),
    // never appends it to its caller. Discard it together with the speculative
    // diagnostics and obligations before the original occurrence-sensitive run.
    drop(output);
    let value = analyze_scalar_value(
        program,
        machine,
        state,
        &|expression| observations.environment(expression),
        expression,
        field_type,
        owner,
        diagnostics,
        formation_obligations,
    );
    receiving(value.as_ref(), diagnostics)
}

#[allow(clippy::too_many_arguments)]
fn analyze_scalar_value(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    environment_for: &impl Fn(ExpressionHandle) -> ValueEnvironment,
    expression: ExpressionHandle,
    field_type: TypeReferenceHandle,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) -> Option<ConstructionScalarValue> {
    let target = program.primitive_type_reference(field_type)?;
    if let Some(dispatch) = program.expression_table.match_node(expression) {
        let before = diagnostics.len();
        let mut combined: Option<ConstructionScalarValue> = None;
        let mut complete = true;
        for arm in program.expression_table.match_arms(dispatch.arms) {
            let Some(value) = analyze_scalar_value(
                program,
                machine,
                state,
                environment_for,
                arm.value,
                field_type,
                owner,
                diagnostics,
                formation_obligations,
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

    let environment = if matches!(
        program.expression_table.expression(expression),
        typed_trees::expression::ExpressionNode::Integer(_)
            | typed_trees::expression::ExpressionNode::Float(_)
            | typed_trees::expression::ExpressionNode::Boolean(_)
            | typed_trees::expression::ExpressionNode::String(_)
    ) && !crate::proof_contracts::arithmetic_domains::declares_bracketed_range(
        program, field_type,
    ) {
        ValueEnvironment::default()
    } else {
        environment_for(expression)
    };
    let before = diagnostics.len();
    let mut decisions = ArithmeticDecisions::default();
    let (interval, primitive, source_domain) = validate_value_range_retaining_decisions(
        program,
        machine,
        Some(state),
        expression,
        &environment,
        Some(target),
        numerics::arithmetic::ArithmeticDomain::Exact,
        owner,
        diagnostics,
        &mut decisions,
    );
    formation_obligations.extend(decisions.integer_range_obligations);
    if diagnostics.len() != before {
        return None;
    }
    check_validated_value_narrowing(
        program,
        machine,
        Some(state),
        expression,
        target,
        program.arithmetic_domain_for_type_reference(field_type),
        interval,
        primitive,
        source_domain,
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
