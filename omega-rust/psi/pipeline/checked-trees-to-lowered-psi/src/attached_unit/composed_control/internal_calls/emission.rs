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
    validate_transfer_shape(
        structural_arguments,
        &[],
        parameters,
        &[],
        &[],
        &target.structural_parameters,
        type_ids,
        structural_types,
        &[],
        &[],
    )?;
    let structural_arguments = lower_structural_arguments(
        structural_arguments,
        parameters,
        &[],
        &[],
        byte_argument_places,
        &[],
    )?;
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
        source_site,
        id,
        *target_state,
    )?;
    let (result, kind) = if let Some(binding) = result_binding {
        let checked_trees::CheckedControlResultPlan::Structural(signature) = &target.result else {
            return unsupported("internal structural call result catalog missing");
        };
        if signature.type_identity != binding.type_identity
            || signature.multiplicity != binding.multiplicity
            || !signature.qualifications.is_empty()
        {
            return unsupported("internal structural call result catalog drifted");
        }
        let multiplicity = match signature.multiplicity {
            Multiplicity::Affine => StructuralMultiplicity::Affine,
            Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
            _ => return unsupported("internal structural call requires retained linear custody"),
        };
        let place = place_id(allocate_dense(next_place)?);
        let structural_type = lookup_type_id(type_ids, &signature.type_identity)?;
        result_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer: id,
                structural_type,
            },
        });
        (
            OperationResult::Structural(terminal_psi::StructuralOperationResult {
                place,
                structural_type,
                multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            OperationKind::CallStructuralWithScalarArguments {
                callee: target.id,
                arguments: arguments.into_iter().map(|value| value.id).collect(),
                structural_arguments,
                claim_transfers: Vec::new(),
                returned_claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations,
            },
        )
    } else {
        if target.result != checked_trees::CheckedControlResultPlan::Unit {
            return unsupported("internal Unit call result catalog drifted");
        }
        (
            OperationResult::Unit,
            OperationKind::CallUnit {
                callee: target.id,
                arguments: arguments.into_iter().map(|value| value.id).collect(),
                structural_arguments,
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations,
            },
        )
    };
    operations.push(Operation {
        static_reach_binding: None,
        id,
        result,
        kind,
    });
    Ok(())
}
