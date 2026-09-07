//! State-local scalar storage uses the shared source-bound value namespace.

use super::*;
use crate::scalar_bindings::ScalarBindings;
use checked_trees::{CheckedScalarBindingDestination, CheckedScalarBindingValue};

fn role(
    binding: &checked_trees::CheckedScalarBinding,
    immutable_ordinal: u32,
) -> CheckedScalarExpressionRole {
    match binding.destination {
        CheckedScalarBindingDestination::Immutable => {
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: immutable_ordinal,
            }
        }
        CheckedScalarBindingDestination::StorageInitialize { .. } => {
            CheckedScalarExpressionRole::StorageInitializer
        }
        CheckedScalarBindingDestination::StorageAssign { .. } => {
            CheckedScalarExpressionRole::AssignmentValue
        }
    }
}

pub(super) fn validate(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
) -> Result<(), LoweringError> {
    if state.bindings.len() != state.binding_initializers.len() {
        return unsupported("Unit graph scalar prefix lost an initializer");
    }
    let mut immutable_ordinal = 0;
    for (ordinal, (binding, value)) in state
        .bindings
        .iter()
        .zip(&state.binding_initializers)
        .enumerate()
    {
        if binding.statement_ordinal as usize != ordinal
            || binding.value != CheckedScalarBindingValue::Expression
        {
            return unsupported("Unit graph scalar prefix reordered or replaced a binding");
        }
        let (source, retained) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(
                state.state,
                binding.statement_ordinal,
                role(binding, immutable_ordinal),
            )
            .ok_or(LoweringError::Unsupported(
                "Unit graph scalar prefix has no exact checked expression",
            ))?;
        if retained != value {
            return unsupported(
                "Unit graph scalar initializer disagrees with its checked expression",
            );
        }
        if let CheckedScalarBindingDestination::StorageInitialize { symbol }
        | CheckedScalarBindingDestination::StorageAssign { symbol } = binding.destination
            && source.destination != symbol
        {
            return unsupported("Unit graph scalar storage destination disagrees with source");
        }
        crate::scalar_source_custody::validate_pure(
            checked,
            source,
            terminal_scalar_type(binding.primitive_type)?,
        )?;
        if binding.destination == CheckedScalarBindingDestination::Immutable {
            immutable_ordinal += 1;
        }
    }
    Ok(())
}

pub(super) fn emit_prefix(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    parameters: &[(u32, StructuralParameterDeclaration)],
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<ScalarBindings, LoweringError> {
    let mut bindings = ScalarBindings::new(values.len()).with_structural_parameters(parameters);
    let (_, authored) = crate::scalar_source_custody::authored_state(checked, state.state)?;
    for (position, parameter) in state.scalar_parameters.iter().enumerate() {
        let source = &checked.state_parameters(authored)[parameter.source_position as usize];
        if source.is_mutable {
            bindings.initialize_parameter(
                source.symbol,
                terminal_scalar_type(parameter.primitive_type)?,
                position,
            )?;
        }
    }
    let mut immutable_ordinal = 0;
    for binding in &state.bindings {
        let expression = bindings.expression_at(
            checked,
            state.state,
            binding.statement_ordinal,
            role(binding, immutable_ordinal),
        )?;
        let scalar_type = terminal_scalar_type(binding.primitive_type)?;
        if expression.scalar_type() != scalar_type
            || direct_expression_contains_short_circuit(&expression)
        {
            return unsupported("Unit graph scalar prefix needs a matching branch-free value");
        }
        validate_direct_parameter_types(
            &expression,
            &values
                .iter()
                .map(|value| value.scalar_type)
                .collect::<Vec<_>>(),
        )?;
        let id = emit_direct_expression(&expression, values, next_value, operations);
        bindings.append(binding.destination, scalar_type, values.len())?;
        values.push(ValueDeclaration { id, scalar_type });
        if binding.destination == CheckedScalarBindingDestination::Immutable {
            immutable_ordinal += 1;
        }
    }
    Ok(bindings)
}
