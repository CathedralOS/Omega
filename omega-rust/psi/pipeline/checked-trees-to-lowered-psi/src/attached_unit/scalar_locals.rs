//! Rechecked lowering for branch-free scalar locals in Unit bodies.

use super::*;
use checked_trees::CheckedUnitScalarResultBindingPlan;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_scalar_expression_local(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    result: &CheckedUnitScalarResultBindingPlan,
    value: &CheckedScalarExpression,
    scalar_parameter_count: usize,
    scalar_values: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<ValueDeclaration, LoweringError> {
    if usize::try_from(result.binding_ordinal)
        .ok()
        .and_then(|ordinal| ordinal.checked_add(scalar_parameter_count))
        != Some(scalar_values.len())
    {
        return unsupported("Unit scalar expression local binding drifted from source order");
    }
    let retained = checked
        .facts
        .values
        .scalar_expressions
        .expression_at(
            state,
            result.statement_index,
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: result.binding_ordinal,
            },
        )
        .ok_or(LoweringError::Unsupported(
            "Unit scalar expression local lost its checked value fact",
        ))?;
    if retained != value {
        return unsupported("Unit scalar expression local drifted from its checked value fact");
    }
    let expression = lower_checked_scalar_expression(value)?;
    if direct_expression_contains_short_circuit(&expression) {
        return unsupported("Unit scalar expression locals do not admit short-circuit control");
    }
    let scalar_type = terminal_scalar_type(result.primitive_type)?;
    if expression.scalar_type() != scalar_type {
        return unsupported("Unit scalar expression local type disagrees with its binding");
    }
    let source_types = scalar_values
        .iter()
        .map(|value| value.scalar_type)
        .collect::<Vec<_>>();
    validate_direct_parameter_types(&expression, &source_types)?;
    let id = emit_direct_expression(&expression, scalar_values, next_value_identity, operations);
    Ok(ValueDeclaration { id, scalar_type })
}
