//! Exact source bindings and final custody of ordinary structural call results.

use super::*;
use checked_trees::CheckedUnitStructuralResultBindingPlan;

fn producer(
    caller: &CheckedUnitEffectMachinePlan,
    binding_ordinal: u32,
) -> Result<(&CheckedUnitStructuralResultBindingPlan, bool), LoweringError> {
    let mut matches = caller
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::StructuralCall {
                result,
                discard_result_on_return,
                ..
            } if result.binding_ordinal == binding_ordinal => {
                Some((result, *discard_result_on_return))
            }
            _ => None,
        });
    let result = matches.next().ok_or(LoweringError::Unsupported(
        "Unit structural result argument has no exact ordinary producer binding",
    ))?;
    if matches.next().is_some() {
        return unsupported("Unit structural result argument has ambiguous producer bindings");
    }
    Ok(result)
}

pub(crate) fn validate_usage(
    caller: &CheckedUnitEffectMachinePlan,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Result<(), LoweringError> {
    let (original, discard) = producer(caller, result.binding_ordinal)?;
    if original != result {
        return unsupported("Unit structural result binding disagrees with its producer");
    }
    let mut consumed = false;
    for operation in &caller.operations {
        let (CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            structural_arguments,
            ..
        }) = operation
        else {
            continue;
        };
        for argument in structural_arguments {
            if argument.source_structural_result_binding_ordinal() != Some(result.binding_ordinal) {
                continue;
            }
            if consumed || coordinate.statement_index <= result.statement_index {
                return unsupported(
                    "Unit structural result is consumed before production or twice",
                );
            }
            if !argument.path.is_empty()
                || argument.access != checked_trees::CheckedStructuralAccess::Owned
                || argument.type_identity != result.type_identity
                || result.multiplicity != Multiplicity::Affine
            {
                return unsupported("Unit structural result use is not a whole owned affine move");
            }
            consumed = true;
        }
    }
    if discard == consumed {
        return unsupported(
            "Unit structural result cleanup disagrees with its final consuming use",
        );
    }
    Ok(())
}

pub(crate) fn validate_consumer(
    checked: &CheckedTrees,
    caller: &CheckedUnitEffectMachinePlan,
    operation: &CheckedUnitEffectOperationPlan,
    target_parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    target_entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
) -> Result<(), LoweringError> {
    let (coordinate, structural_arguments, claim_transfers) = match operation {
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            structural_arguments,
            claim_transfers,
            ..
        } => (coordinate, structural_arguments, claim_transfers.as_slice()),
        CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            structural_arguments,
            ..
        } => (coordinate, structural_arguments, &[][..]),
        _ => {
            return unsupported(
                "ordinary structural result use requires a Unit or structural call",
            );
        }
    };
    if structural_arguments.len() != target_parameters.len() {
        return unsupported("Unit structural argument arity disagrees with its target");
    }
    let authored =
        crate::call_source_custody::authored::locate_source(checked, caller.state, *coordinate)?;
    let (_, state) = crate::scalar_source_custody::authored_state(checked, caller.state)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    for (index, (argument, parameter)) in structural_arguments
        .iter()
        .zip(target_parameters)
        .enumerate()
    {
        let expression = authored
            .structural_arguments
            .iter()
            .find_map(|(position, expression)| {
                (*position == parameter.position).then_some(*expression)
            });
        let binding_ordinal = argument.source_structural_result_binding_ordinal();
        // Check both directions: an authored result cannot be replaced with a
        // same-typed parameter or construction-local plan.
        for candidate in &caller.operations {
            let CheckedUnitEffectOperationPlan::StructuralCall { result, .. } = candidate else {
                continue;
            };
            let Some(StatementNode::LocalData(local)) =
                statements.get(result.statement_index as usize)
            else {
                return unsupported("Unit structural result producer has no authored local");
            };
            if local.is_mutable
                || !local.symbol.is_valid()
                || checked
                    .typed
                    .normalized_type_identity(local.type_reference)
                    .into_string()
                    != result.type_identity
            {
                return unsupported(
                    "Unit structural result producer disagrees with its immutable authored local",
                );
            }
            let names_result = expression.is_some_and(|expression| matches!(
                checked.expression_table.expression(expression),
                ExpressionNode::Name(name) if local.symbol.is_valid() && name.symbol == local.symbol
            ));
            if names_result != (binding_ordinal == Some(result.binding_ordinal)) {
                return unsupported(
                    "Unit structural result argument does not rejoin its exact authored local",
                );
            }
        }
        let Some(binding_ordinal) = binding_ordinal else {
            continue;
        };
        let (result, discard) = producer(caller, binding_ordinal)?;
        if discard
            || result.statement_index >= coordinate.statement_index
            || result.multiplicity != Multiplicity::Affine
            || argument.type_identity != result.type_identity
            || parameter.type_identity != result.type_identity
            || !argument.path.is_empty()
            || argument.access != checked_trees::CheckedStructuralAccess::Owned
            || parameter.access != checked_trees::CheckedStructuralAccess::Owned
            || parameter.multiplicity != Multiplicity::Affine
            || parameter.is_self
            || !parameter.qualifications.is_empty()
            || parameter.fused_service_erasure.is_some()
            || target_entry_claims
                .iter()
                .any(|claim| claim.parameter_index as usize == index)
            || claim_transfers
                .iter()
                .any(|transfer| transfer.argument_index as usize == index)
        {
            return unsupported(
                "Unit structural result argument has invalid owned claim-free custody",
            );
        }
    }
    Ok(())
}
