//! Calls into complete Unit bodies in the shared catalog.

use super::*;

pub(in crate::attached_unit::composed_control) fn emit_call_operation(
    checked: &CheckedTrees,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    operation: &CheckedUnitEffectOperationPlan,
    targets: &[super::super::catalogs::LoweredComposedInternalTarget],
    parameters: &[StructuralParameterDeclaration],
    type_ids: &[(String, StructuralTypeId)],
    domain_ids: &[(SemanticDomainId, StructuralDomainId)],
    claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
    structural_types: &[StructuralTypeDeclaration],
    scalar_values: Option<&[ValueDeclaration]>,
    byte_argument_places: &[PlaceId],
    next_place: &mut u64,
    result_places: &mut Vec<StructuralPlaceDeclaration>,
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    let (
        coordinate,
        target_machine,
        target_state,
        structural_arguments,
        result_binding,
        source_site,
    ) = match operation {
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_machine,
            target_state,
            structural_arguments,
            claim_transfers,
            ..
        } if claim_transfers.is_empty() => (
            coordinate,
            target_machine,
            target_state,
            structural_arguments,
            None,
            None,
        ),
        CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            source_site,
            target_machine,
            target_state,
            structural_arguments,
            result,
            ..
        } => (
            coordinate,
            target_machine,
            target_state,
            structural_arguments,
            Some(result),
            *source_site,
        ),
        _ => return unsupported("composed internal call custody drifted before emission"),
    };
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
    let custody = match operation {
        CheckedUnitEffectOperationPlan::StructuralCall { custody, .. } => Some(custody),
        _ => None,
    };
    let target_entry = crate::attached_unit::bodies::UnitBody::find(
        &checked.facts.flow.terminal_unit_effects,
        *target_machine,
    )?
    .entry()?;
    let earlier_results = result_places
        .iter()
        .copied()
        .map(|place| (place, false))
        .collect::<Vec<_>>();
    validate_transfer_shape(
        structural_arguments,
        custody.map_or(&[], |custody| custody.claim_transfers.as_slice()),
        parameters,
        &[],
        &earlier_results,
        &target.structural_parameters,
        type_ids,
        structural_types,
        &target_entry
            .entry_claims
            .iter()
            .map(|claim| claim.parameter_index)
            .collect::<Vec<_>>(),
        &[],
    )?;
    let structural_arguments = lower_structural_arguments(
        structural_arguments,
        parameters,
        &[],
        &earlier_results,
        byte_argument_places,
        &[],
    )?;
    // Instantiate against completed arguments, never callee-local value IDs or
    // a second evaluation of the authored argument expressions.
    let crash_continuations =
        lower_checked_crash_route_buckets(&target.parameter_relative_crash_routes, &arguments)?;
    if result_binding.is_some() {
        let declaration = crate::attached_unit::ordinary_calls::emit_structural(
            checked,
            state.state,
            operation,
            crate::attached_unit::ordinary_calls::PreparedCall {
                arguments: arguments.into_iter().map(|value| value.id).collect(),
                structural_arguments,
                requirement_obligations: Vec::new(),
                crash_continuations,
            },
            target.id,
            type_ids,
            domain_ids,
            claim_bindings,
            false,
            next_place,
            operations,
        )?;
        result_places.push(declaration);
        return Ok(());
    }
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
        source_site,
        id,
        *target_state,
    )?;
    if target.result != checked_trees::CheckedControlResultPlan::Unit {
        return unsupported("internal Unit call result catalog drifted");
    }
    operations.push(Operation {
        static_reach_binding: None,
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
