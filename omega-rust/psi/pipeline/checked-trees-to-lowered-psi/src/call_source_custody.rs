//! Source custody shared by Unit and boundary call operands.

use super::*;

pub(crate) mod authored;
pub(crate) mod initializers;
pub(crate) mod occurrences;

pub(super) fn validate_operation(
    checked: &CheckedTrees,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    if let CheckedUnitEffectOperationPlan::ScalarCall { result, .. }
    | CheckedUnitEffectOperationPlan::BoundaryScalarCall { result, .. } = operation
    {
        let source = crate::scalar_source_custody::locate(
            checked,
            caller_state,
            result.statement_index,
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: result.binding_ordinal,
            },
        )?;
        if source.machine != caller_machine || source.primitive_type != result.primitive_type {
            return unsupported("call result binding disagrees with its authored scalar local");
        }
    }
    let (coordinate, target_machine, target_state, arguments, boundary, source_site) =
        match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                target_machine,
                target_state,
                scalar_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::ScalarCall {
                coordinate,
                target_machine,
                target_state,
                scalar_arguments,
                ..
            } => (
                coordinate,
                target_machine,
                target_state,
                scalar_arguments,
                false,
                None,
            ),
            CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate,
                source_site,
                target_machine,
                target_state,
                scalar_arguments,
                ..
            } => (
                coordinate,
                target_machine,
                target_state,
                scalar_arguments,
                false,
                Some(source_site),
            ),
            CheckedUnitEffectOperationPlan::BoundaryCall {
                coordinate,
                source_site,
                target_machine,
                target_state,
                scalar_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                coordinate,
                source_site,
                target_machine,
                target_state,
                scalar_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate,
                source_site,
                target_machine,
                target_state,
                scalar_arguments,
                ..
            } => (
                coordinate,
                target_machine,
                target_state,
                scalar_arguments,
                true,
                Some(source_site),
            ),
            // Selected operators retain their own exact provider application;
            // they are not authored calls with positional argument facts.
            _ => return Ok(()),
        };
    if matches!(
        operation,
        CheckedUnitEffectOperationPlan::ScalarCall { .. }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall { .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. }
    ) && arguments.iter().any(|argument| {
        matches!(
            argument,
            checked_trees::CheckedCallScalarArgument::Computation(_)
        )
    }) {
        initializers::validate(checked, caller_machine, caller_state, *coordinate)?;
    }
    let call = authored::locate(
        checked,
        caller_machine,
        caller_state,
        *coordinate,
        *target_machine,
        *target_state,
    )?;
    if call.boundary != boundary
        || source_site.is_some_and(|site| *site != call.source_site)
        || call.scalar_arguments.len() != arguments.len()
    {
        return unsupported("call operands disagree with their authored call site or signature");
    }
    for (ordinal, (argument, (expression, primitive_type))) in
        arguments.iter().zip(&call.scalar_arguments).enumerate()
    {
        let argument_ordinal = u32::try_from(ordinal)
            .map_err(|_| LoweringError::Unsupported("call scalar operand ordinal exceeds u32"))?;
        let role = if boundary {
            CheckedScalarExpressionRole::BoundaryCallArgument {
                call_ordinal: coordinate.call_ordinal,
                argument_ordinal,
            }
        } else {
            CheckedScalarExpressionRole::UnitCallArgument {
                call_ordinal: coordinate.call_ordinal,
                argument_ordinal,
            }
        };
        match argument {
            checked_trees::CheckedCallScalarArgument::Pure(argument) => {
                let (binding, selected) = checked
                    .facts
                    .values
                    .scalar_expressions
                    .bound_expression_at(caller_state, coordinate.statement_index, role)
                    .ok_or(LoweringError::Unsupported(
                        "call scalar operand has no unique source-bound checked plan",
                    ))?;
                if binding.expression != *expression
                    || binding.destination.is_valid()
                    || selected != argument
                    || lower_checked_scalar_expression(argument)?.scalar_type()
                        != terminal_scalar_type(*primitive_type)?
                {
                    return unsupported("call scalar operand disagrees with its authored argument");
                }
                crate::scalar_source_custody::validate_namespace(checked, binding)?;
            }
            checked_trees::CheckedCallScalarArgument::Computation(computation) => {
                let plans = &checked.facts.values.scalar_computations;
                let root = plans
                    .root_at(caller_state, coordinate.statement_index, role)
                    .ok_or(LoweringError::Unsupported(
                        "call scalar operand has no unique checked computation root",
                    ))?;
                if root.machine != caller_machine
                    || root.root != *computation
                    || !plans.nodes.is_valid(*computation)
                {
                    return unsupported("call scalar operand disagrees with its computation root");
                }
                let node = plans.nodes.get(*computation);
                if node.authored_root != *expression || node.primitive_type != *primitive_type {
                    return unsupported("call computation disagrees with its authored argument");
                }
                crate::scalar_source_custody::validate_computation_calls(
                    checked,
                    caller_machine,
                    caller_state,
                    coordinate.statement_index,
                    *computation,
                    *expression,
                )?;
            }
        }
    }
    Ok(())
}
