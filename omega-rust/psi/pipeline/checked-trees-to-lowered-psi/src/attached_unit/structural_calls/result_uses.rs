//! Exact source bindings and final custody of structural call results.

use super::super::parameters::expression_producer;
use super::*;
use checked_trees::CheckedUnitStructuralResultBindingPlan;

struct Producer<'plan> {
    operation_index: usize,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    result: &'plan CheckedUnitStructuralResultBindingPlan,
    discard: bool,
}

fn producer(
    caller: &CheckedUnitEffectMachinePlan,
    binding_ordinal: u32,
) -> Result<Producer<'_>, LoweringError> {
    let mut matches =
        caller
            .operations
            .iter()
            .enumerate()
            .filter_map(|(operation_index, operation)| match operation {
                CheckedUnitEffectOperationPlan::StructuralCall {
                    coordinate,
                    result,
                    discard_result_on_return,
                    ..
                }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    coordinate,
                    result,
                    discard_result_on_return,
                    ..
                } if result.binding_ordinal == binding_ordinal => Some(Producer {
                    operation_index,
                    coordinate: *coordinate,
                    result,
                    discard: *discard_result_on_return,
                }),
                _ => None,
            });
    let result = matches.next().ok_or(LoweringError::Unsupported(
        "Unit structural result argument has no exact producer binding",
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
    let producer = producer(caller, result.binding_ordinal)?;
    if producer.result != result {
        return unsupported("Unit structural result binding disagrees with its producer");
    }
    let mut consumed = false;
    let mut disposed = false;
    let mut projected_paths = Vec::<&[checked_trees::CheckedUnitStructuralPathSegment]>::new();
    for (operation_index, operation) in caller.operations.iter().enumerate() {
        if let CheckedUnitEffectOperationPlan::CallContinuationCleanup {
            coordinate,
            affine_discards,
        } = operation
        {
            for discard in affine_discards {
                if discard.source
                    != (checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: result.binding_ordinal,
                    })
                {
                    continue;
                }
                if consumed
                    || disposed
                    || producer.discard
                    || !discard.path.is_empty()
                    || discard.type_identity != result.type_identity
                    || producer.coordinate.call_ordinal == 0
                    || producer.coordinate.statement_index != coordinate.statement_index
                    || operation_index <= producer.operation_index
                {
                    return unsupported("call continuation does not own this intact result");
                }
                disposed = true;
            }
            continue;
        }
        let (CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::ScalarCall {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            coordinate,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
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
            let source_order = if producer.coordinate.call_ordinal == 0 {
                result.statement_index < coordinate.statement_index
            } else {
                result.statement_index == coordinate.statement_index
                    && coordinate.call_ordinal < producer.coordinate.call_ordinal
            };
            if consumed || disposed || operation_index <= producer.operation_index || !source_order
            {
                return unsupported(
                    "Unit structural result is consumed before production or twice",
                );
            }
            if !argument.path.is_empty() {
                if !matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. })
                    || argument.access != checked_trees::CheckedStructuralAccess::Owned
                    || result.multiplicity != Multiplicity::Affine
                    || projected_paths.iter().any(|earlier| {
                        earlier.starts_with(&argument.path) || argument.path.starts_with(earlier)
                    })
                {
                    return unsupported(
                        "Unit result projection overlaps or lacks owned call custody",
                    );
                }
                projected_paths.push(&argument.path);
                continue;
            }
            if !projected_paths.is_empty()
                || !matches!(
                    argument.access,
                    checked_trees::CheckedStructuralAccess::Owned
                        | checked_trees::CheckedStructuralAccess::SharedBorrow
                )
                || argument.type_identity != result.type_identity
                || result.multiplicity != Multiplicity::Affine
            {
                return unsupported(
                    "Unit structural result use is not a whole affine move or shared borrow",
                );
            }
            if argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow
                && producer.coordinate.call_ordinal != 0
                && (producer.coordinate.call_ordinal != 1
                    || producer.discard
                    || coordinate.call_ordinal != 0
                    || !matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { scalar_arguments, structural_arguments, .. }
                        if scalar_arguments.is_empty() && structural_arguments.len() == 1)
                    || !matches!(caller.operations.get(operation_index + 1),
                        Some(CheckedUnitEffectOperationPlan::CallContinuationCleanup { coordinate: cleanup, affine_discards })
                            if cleanup == coordinate && affine_discards.len() == 1
                                && affine_discards[0].source == argument.source))
            {
                return unsupported("anonymous shared result has no dying Unit call continuation");
            }
            consumed = argument.access == checked_trees::CheckedStructuralAccess::Owned;
        }
    }
    if (projected_paths.is_empty() && producer.discard == (consumed || disposed))
        || (!projected_paths.is_empty() && producer.discard)
        || (producer.coordinate.call_ordinal != 0
            && !consumed
            && projected_paths.is_empty()
            && !disposed)
    {
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
        }
        | CheckedUnitEffectOperationPlan::ScalarCall {
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
        CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            structural_arguments,
            completion_receipts,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            coordinate,
            structural_arguments,
            completion_receipts,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate,
            structural_arguments,
            completion_receipts,
            ..
        } => (
            coordinate,
            structural_arguments,
            completion_receipts.as_slice(),
        ),
        _ => {
            return unsupported("structural result use requires an ordinary or boundary call");
        }
    };
    if structural_arguments.len() != target_parameters.len() {
        return unsupported("Unit structural argument arity disagrees with its target");
    }
    let operation_index = caller
        .operations
        .iter()
        .position(|candidate| candidate == operation)
        .ok_or(LoweringError::Unsupported(
            "structural result consumer has no operation position",
        ))?;
    let authored =
        crate::call_source_custody::authored::locate_source(checked, caller.state, *coordinate)?;
    let authored_nested = authored
        .structural_arguments
        .iter()
        .any(|(_, expression)| expression_producer(checked, *expression).is_some());
    validate_nested_execution_order(checked, caller, coordinate.statement_index, authored_nested)?;
    let (source_machine, state) =
        crate::scalar_source_custody::authored_state(checked, caller.state)?;
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
        if binding_ordinal.is_none()
            && expression
                .is_some_and(|expression| expression_producer(checked, expression).is_some())
        {
            return unsupported(
                "nested structural argument has no expression-owned producer binding",
            );
        }
        // Check both directions: an authored result cannot be replaced with a
        // same-typed parameter or construction-local plan.
        for candidate in &caller.operations {
            let (CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate: producer_coordinate,
                source_site,
                result,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate: producer_coordinate,
                source_site,
                result,
                ..
            }) = candidate
            else {
                continue;
            };
            let names_result = if producer_coordinate.call_ordinal == 0 {
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
                match expression.map(|expression| {
                    super::super::parameters::source_path(
                        checked,
                        source_machine,
                        local.type_reference,
                        expression,
                    )
                }) {
                    Some(Ok((root, path, access))) if root == local.symbol => {
                        if path != argument.path
                            || access.unwrap_or(checked_trees::CheckedStructuralAccess::Owned)
                                != argument.access
                        {
                            return unsupported(
                                "Unit result projection disagrees with its authored path",
                            );
                        }
                        true
                    }
                    _ => false,
                }
            } else {
                let source = crate::call_source_custody::authored::locate_source(
                    checked,
                    caller.state,
                    *producer_coordinate,
                )?;
                if source.source_site != *source_site {
                    return unsupported(
                        "nested structural producer has a different authored source",
                    );
                }
                let Some(checked_trees::NominalMachineUseSite::Expression(source_expression)) =
                    source.source_site
                else {
                    return unsupported("anonymous producer has no expression-owned source");
                };
                let matches = producer_coordinate.statement_index == coordinate.statement_index
                    && expression.and_then(|expression| expression_producer(checked, expression))
                        == Some(source_expression);
                if matches {
                    let signature = crate::call_source_custody::authored::target_signature(
                        checked,
                        source_machine.symbol,
                        source.source_target,
                    )?;
                    let (root, path, access) = super::super::parameters::source_place_path(
                        checked,
                        source_machine,
                        signature.return_type,
                        expression.unwrap(),
                    )?;
                    if root != facts::PlaceRoot::Expression(source_expression)
                        || path != argument.path
                        || access.unwrap_or(checked_trees::CheckedStructuralAccess::Owned)
                            != argument.access
                    {
                        return unsupported(
                            "anonymous result projection disagrees with its authored source",
                        );
                    }
                    if argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow {
                        super::shared_temporary::validate(
                            checked,
                            caller,
                            *producer_coordinate,
                            *coordinate,
                            source_expression,
                        )?;
                    }
                }
                matches
            };
            if names_result != (binding_ordinal == Some(result.binding_ordinal)) {
                return unsupported(if producer_coordinate.call_ordinal == 0 {
                    "Unit structural result argument does not rejoin its exact authored local"
                } else {
                    "Unit structural result argument does not rejoin its exact authored source"
                });
            }
            if names_result
                && producer_coordinate.call_ordinal == 0
                && expression.is_none_or(|expression| {
                    named_result_operand(checked, expression).1 != argument.access
                })
            {
                return unsupported(
                    "Unit structural result access disagrees with its authored operand",
                );
            }
        }
        let Some(binding_ordinal) = binding_ordinal else {
            continue;
        };
        let producer = producer(caller, binding_ordinal)?;
        let result = producer.result;
        let source_order = if producer.coordinate.call_ordinal == 0 {
            result.statement_index < coordinate.statement_index
        } else {
            result.statement_index == coordinate.statement_index
                && coordinate.call_ordinal < producer.coordinate.call_ordinal
        };
        if (producer.discard && argument.access == checked_trees::CheckedStructuralAccess::Owned)
            || producer.operation_index >= operation_index
            || !source_order
            || matches!(operation, CheckedUnitEffectOperationPlan::StructuralCall { result: consumer, .. }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result: consumer, .. }
                if result.binding_ordinal >= consumer.binding_ordinal)
            || result.multiplicity != Multiplicity::Affine
            || (argument.path.is_empty() && argument.type_identity != result.type_identity)
            || parameter.type_identity != argument.type_identity
            || (!argument.path.is_empty()
                && (!matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. })
                    || argument.access != checked_trees::CheckedStructuralAccess::Owned))
            || !matches!(
                argument.access,
                checked_trees::CheckedStructuralAccess::Owned
                    | checked_trees::CheckedStructuralAccess::SharedBorrow
            )
            || argument.access != parameter.access
            || parameter.multiplicity
                != if argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow {
                    Multiplicity::Unrestricted
                } else {
                    Multiplicity::Affine
                }
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
            return unsupported("Unit structural result argument has invalid claim-free custody");
        }
    }
    Ok(())
}

fn named_result_operand(
    checked: &CheckedTrees,
    expression: checked_trees::expression::ExpressionHandle,
) -> (
    checked_trees::expression::ExpressionHandle,
    checked_trees::CheckedStructuralAccess,
) {
    if let ExpressionNode::Borrow(borrow) = checked.expression_table.expression(expression)
        && borrow.access == language_core::ReferenceAccess::Shared
    {
        (
            borrow.target,
            checked_trees::CheckedStructuralAccess::SharedBorrow,
        )
    } else {
        (expression, checked_trees::CheckedStructuralAccess::Owned)
    }
}

fn validate_nested_execution_order(
    checked: &CheckedTrees,
    caller: &CheckedUnitEffectMachinePlan,
    statement_index: u32,
    authored_nested: bool,
) -> Result<(), LoweringError> {
    if !authored_nested
        && !caller.operations.iter().any(|operation| {
            matches!(operation,
        CheckedUnitEffectOperationPlan::StructuralCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { coordinate, .. }
            if coordinate.statement_index == statement_index && coordinate.call_ordinal != 0)
        })
    {
        return Ok(());
    }
    let expected = crate::call_source_custody::authored::nested::authored_postorder(
        checked,
        caller.state,
        statement_index,
    )?;
    let mut actual = Vec::new();
    for operation in &caller.operations {
        let coordinate = match operation {
            CheckedUnitEffectOperationPlan::StructuralCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. }
            | CheckedUnitEffectOperationPlan::ScalarCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::BoundaryCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall { coordinate, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { coordinate, .. }
                if coordinate.statement_index == statement_index =>
            {
                coordinate
            }
            _ => continue,
        };
        actual.push(coordinate.call_ordinal);
    }
    if !actual
        .iter()
        .copied()
        .eq(expected.iter().filter_map(|(ordinal, expression)| {
            if *ordinal != 0
                && let ExpressionNode::Call(call) = checked.expression_table.expression(*expression)
                && crate::scalar_source_custody::authored_state(checked, call.target_symbol)
                    .is_ok_and(|(_, target)| {
                        checked
                            .primitive_type_reference(target.return_type)
                            .is_some()
                    })
            {
                None
            } else {
                Some(*ordinal)
            }
        }))
    {
        return unsupported(
            "nested structural operations disagree with authored argument execution order",
        );
    }
    Ok(())
}
