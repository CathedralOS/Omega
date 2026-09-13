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
    // Binding ordinals belong to this source state, whereas the place catalog
    // spans the whole emitted machine. Rejoin the current operation registry.
    let earlier_results = if structural_arguments.iter().any(|argument| {
        argument
            .source_structural_result_binding_ordinal()
            .is_some()
    }) {
        operations.structural_values.iter().enumerate().map(|(binding_position, (ordinal, result))| {
        if *ordinal as usize != binding_position {
            return unsupported("internal call result binding namespace is stale or duplicated");
        }
        let mut declarations = result_places.iter().filter(|place| place.id == result.place);
        let declaration = declarations.next().ok_or(LoweringError::Unsupported("internal call completed result has no place declaration"))?;
        if declarations.next().is_some() || !matches!(declaration.kind,
            StructuralPlaceKind::OperationResult { structural_type, producer }
                if structural_type == result.structural_type && operations.operations.iter().any(|candidate|
                    candidate.id == producer && candidate.result.structural() == Some(result)))
        {
            return unsupported("internal call result declaration differs from its operation");
        }
        Ok((*declaration, false))
        }).collect::<Result<Vec<_>, LoweringError>>()?
    } else {
        Vec::new()
    };
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
        Some(crate::attached_unit::parameters::StructuralResultCustody {
            results: &operations.structural_values,
            domains: domain_ids,
            claims: claim_bindings,
        }),
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
