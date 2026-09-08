//! Source custody shared by Unit and boundary call operands.

use super::*;

pub(crate) mod authored;
pub(crate) mod initializers;
pub(crate) mod occurrences;
pub(crate) mod projected_receivers;

/// Preserve calls around authored stores and direct call initializers, even
/// when a result is unused. Each retained owner checks its exact operands.
pub(super) fn validate_store_and_initializer_calls(
    checked: &CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    use checked_trees::statement::StatementNode;
    let (_, state) = crate::scalar_source_custody::authored_state(checked, plan.state)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let has_stores = statements
        .iter()
        .any(|statement| matches!(statement, StatementNode::Assignment(_)));
    for (statement_index, statement) in statements.iter().enumerate() {
        let initializer = match statement {
            StatementNode::LocalData(local)
                if checked
                    .expression_table
                    .expression_is_valid(local.initial_value)
                    && matches!(
                        checked.expression_table.expression(local.initial_value),
                        checked_trees::expression::ExpressionNode::Call(_)
                    ) =>
            {
                Some(local.initial_value)
            }
            StatementNode::Assignment(_) | StatementNode::LocalData(_) => continue,
            _ if !has_stores => continue,
            _ => None,
        };
        let statement_index = u32::try_from(statement_index)
            .map_err(|_| LoweringError::Unsupported("call statement ordinal exceeds u32"))?;
        let coordinate = checked_trees::CheckedUnitCallCoordinate {
            statement_index,
            call_ordinal: 0,
        };
        let mut owners = plan.operations.iter().filter(|operation| match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                coordinate: actual, ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryCall {
                coordinate: actual, ..
            }
            | CheckedUnitEffectOperationPlan::ScalarCall {
                coordinate: actual, ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                coordinate: actual, ..
            }
            | CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate: actual, ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate: actual, ..
            }
            | CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                coordinate: actual,
                ..
            }
            | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                coordinate: actual,
                ..
            }
            | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
                coordinate: actual,
                ..
            }
            | CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd {
                coordinate: actual,
                ..
            } => *actual == coordinate,
            _ => false,
        });
        let Some(owner) = owners.next() else {
            return unsupported("Unit body omits or duplicates an authored call");
        };
        if owners.next().is_some() {
            return unsupported("Unit body omits or duplicates an authored call");
        }
        // Selected dispatch may rewrite the source call after checked planning.
        // Its existing exact application owner validates that realization;
        // ordinary calls instead rejoin their captured flow occurrence.
        if let Some(expression) = initializer {
            if !matches!(
                owner,
                CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { .. }
                    | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall { .. }
                    | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall { .. }
                    | CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd { .. }
            ) {
                occurrences::validate(checked, plan.machine, plan.state, coordinate, expression)?;
            }
        } else {
            authored::locate_source(checked, plan.state, coordinate)?;
        }
    }
    Ok(())
}

pub(super) fn validate_operation(
    checked: &CheckedTrees,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    if let CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
        coordinate, result, ..
    } = operation
    {
        initializers::validate_structural(
            checked,
            caller_machine,
            caller_state,
            *coordinate,
            result,
        )?;
    }
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
    ) && coordinate.call_ordinal == 0
        && arguments.iter().any(|argument| {
            matches!(
                argument,
                checked_trees::CheckedCallScalarArgument::Computation(_)
            )
        })
    {
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
                    || argument.primitive_type() != Some(*primitive_type)
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
