//! Calls into complete Unit bodies in the shared catalog.

use super::*;

pub(in crate::attached_unit::composed_control) fn emit_call_operation(
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    operation: &CheckedUnitEffectOperationPlan,
    targets: &[super::super::catalogs::LoweredComposedInternalTarget],
    parameters: &[StructuralParameterDeclaration],
    type_ids: &[(String, StructuralTypeId)],
    structural_types: &[StructuralTypeDeclaration],
    scalar_values: Option<&[ValueDeclaration]>,
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    let CheckedUnitEffectOperationPlan::CallUnit {
        coordinate,
        target_machine,
        target_state,
        structural_arguments,
        claim_transfers,
        ..
    } = operation
    else {
        return unsupported("composed internal operation is not a Unit call");
    };
    if !claim_transfers.is_empty() {
        return unsupported("composed internal Unit call custody drifted before emission");
    }
    let target = targets
        .iter()
        .find(|target| target.source == *target_machine)
        .ok_or(LoweringError::Unsupported(
            "composed internal Unit target is absent",
        ))?;
    let arguments = crate::attached_unit::argument_evaluation::validated_values(
        scalar_values,
        &target.scalar_parameters,
    )?;
    validate_transfer_shape(
        structural_arguments,
        &[],
        parameters,
        &[],
        &[],
        &[],
        &target.structural_parameters,
        type_ids,
        structural_types,
        &[],
    )?;
    let structural_arguments =
        lower_structural_arguments(structural_arguments, parameters, &[], &[], &[], &[])?;
    // Instantiate against completed arguments, never callee-local value IDs or
    // a second evaluation of the authored argument expressions.
    let crash_continuations =
        lower_checked_crash_route_buckets(&target.parameter_relative_crash_routes, &arguments)?;
    let id = operations.allocate();
    operations.record_source_call(
        SourceCallCoordinate {
            state: state.state,
            statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                LoweringError::Unsupported("Unit statement coordinate exceeds usize")
            })?,
            call_ordinal: usize::try_from(coordinate.call_ordinal)
                .map_err(|_| LoweringError::Unsupported("Unit call coordinate exceeds usize"))?,
        },
        None,
        id,
        *target_state,
    )?;
    operations.push(Operation {
        id,
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: target.id,
            arguments: arguments.into_iter().map(|value| value.id).collect(),
            structural_arguments,
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations,
        },
    });
    Ok(())
}
