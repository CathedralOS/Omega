//! Rejoin borrowed call arguments to their original referents and exact geometry.
use super::*;

/// Rejoin the original parameter or earlier literal before pointer transport.
/// Callee declarations and source-call correspondence remain legalization inputs.
pub(super) fn validate_borrowed_argument(
    source: &LegalizedScalarFunction,
    call: &LegalizedScalarCall,
    operation: semantic_vocabulary::OperationId,
    argument_index: usize,
) -> Option<()> {
    let signature = source.structural.as_ref()?;
    let LegalizedScalarArgument::Structural { semantic, target } =
        call.arguments.get(argument_index)?
    else {
        return None;
    };
    // Only incoming structural parameters participate in the caller's ABI.
    // Local literal descriptors are established in the activation instead.
    if source
        .parameters
        .len()
        .checked_add(signature.parameters.len())?
        != source.call_plan.parameters.len()
        || source
            .parameters
            .iter()
            .zip(&source.call_plan.parameters)
            .any(|(parameter, placement)| {
                scalar_shape(parameter.scalar_type) != Some(placement.shape)
                    || parameter.placement != *placement
            })
    {
        return None;
    }
    let parameters = signature
        .parameters
        .iter()
        .map(|parameter| crate::structural_unit_input::Parameter {
            semantic: &parameter.semantic,
            target: &parameter.target,
        })
        .collect::<Vec<_>>();
    let exclusive = matches!(
        semantic.access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    );
    let local = crate::selection::primitive_local_input::local(source, semantic.place);
    let record_home = crate::selection::record_input::home(source, semantic.place);
    let record_root = record_home
        .map(|(_, result)| result.structural_type)
        .or_else(|| {
            signature
                .parameters
                .iter()
                .find(|parameter| parameter.semantic.place == semantic.place)
                .filter(|parameter| {
                    matches!(
                        parameter.semantic.access,
                        StructuralAccess::Owned | StructuralAccess::MutableBorrow
                    ) || parameter.semantic.access == semantic.access
                })
                .map(|parameter| parameter.semantic.structural_type)
        });
    let record = record_root
        .filter(|root| {
            signature.structural_types.iter().any(|declaration| {
                declaration.id == *root
                    && matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::Record { .. }
                    )
            })
        })
        .and_then(|root| {
            let (selected, offset) = crate::structural_reference_input::project(
                root,
                &semantic.path,
                &signature.structural_types,
            )?;
            let shape =
                crate::structural_reference_input::shape(selected, &signature.structural_types)?;
            (target.root_structural_type == root
                && target.structural_type == selected
                && target.source_byte_offset == offset)
                .then_some(ValueShape::borrowed_reference(
                    shape.byte_size,
                    shape.alignment,
                ))
        });
    let byte_view = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == semantic.place)
        .and_then(|parameter| {
            crate::structural_reference_input::fixed_byte_array_view(
                &parameter.semantic,
                semantic,
                target.structural_type,
                &signature.structural_types,
            )
        });
    let shape = if let Some(shape) = record {
        shape
    } else if let Some((offset, _)) = byte_view {
        if offset != target.source_byte_offset
            || call.result_placement.is_some()
            || call.call_plan.result.is_some()
        {
            return None;
        }
        ValueShape::borrowed_reference(16, 8)
    } else if let Some((_, result, _, referent)) = local {
        if target.structural_type != result.structural_type
            || target.root_structural_type != result.structural_type
            || !semantic.path.is_empty()
            || target.source_byte_offset != 0
        {
            return None;
        }
        ValueShape::borrowed_reference(referent.byte_size, referent.alignment)
    } else if exclusive
        || signature.structural_types.iter().any(|declaration| {
            declaration.id == target.structural_type
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
                )
        })
    {
        exclusive_projection_shape(source, semantic, target)?
    } else {
        ValueShape::borrowed_reference(16, 8)
    };
    let expected = evaluate_call_plan(
        source.call_plan.policy,
        &CallSignature {
            parameters: call
                .arguments
                .iter()
                .enumerate()
                .map(|(position, argument)| match argument {
                    LegalizedScalarArgument::Scalar {
                        source: value,
                        placement,
                    } => {
                        let shape = scalar_shape(scalar_value_type(source, *value)?)?;
                        (placement.shape == shape).then_some(shape)
                    }
                    LegalizedScalarArgument::Structural { target, .. } => {
                        Some(if position == argument_index {
                            shape
                        } else {
                            target.shape
                        })
                    }
                })
                .collect::<Option<Vec<_>>>()?,
            result: if call.structural_result.is_some() {
                Some(
                    crate::selection::aggregate_result_input::call_result(source, call)?
                        .1
                        .shape,
                )
            } else {
                call.result_placement
                    .as_ref()
                    .map(|placement| placement.shape)
            },
        },
    )
    .ok()?;
    // Unit entry attachments and service ceilings are retained declaration
    // metadata; they do not add ABI arguments. The scalar-result attachment
    // family remains outside this transport contract.
    if (source.attachment.is_some()
        && !exclusive
        && record.is_none()
        && source.call_plan.result.is_some())
        || !signature.entry_claims.is_empty()
        || (source.call_plan.result.is_some() && !signature.published_service_ceiling.is_empty())
        || (!parameters.is_empty()
            && !crate::unobserved_owned_input::accepts(source)
            && !crate::structural_unit_input::accepts_graph(
                &source.call_plan,
                &parameters,
                &signature.structural_types,
            ))
        || call.source != legalized_operations::LegalizedCallUnitSource::AuthoredCallUnit
        || !call.claim_transfers.is_empty()
        || !call.requirement_obligations.is_empty()
        || !call.crash_continuations.is_empty()
        || call.call_plan != expected
        || (!exclusive
            && record.is_none()
            && (semantic.access != StructuralAccess::SharedBorrow || !semantic.path.is_empty()))
        || target.place != semantic.place
        || target.access != semantic.access
        || target.path != semantic.path
        || (!exclusive && record.is_none() && target.root_structural_type != target.structural_type)
        || target.shape != shape
        || (!exclusive && record.is_none() && target.source_byte_offset != 0)
        || target.fixed_array_length != byte_view.map(|(_, length)| length)
        || target.element_stride != byte_view.map(|_| 1)
        || Some(&target.destination) != expected.parameters.get(argument_index)
    {
        return None;
    }
    match &target.source {
        target_operations::TargetStructuralArgumentSource::StructuralHome { psi_operation } => {
            let (producer, _) = record_home?;
            if record.is_none() || *psi_operation != producer || producer == operation {
                return None;
            }
        }
        target_operations::TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
            psi_operation,
        } => {
            let (producer, _, _, _) = local?;
            if *psi_operation != producer || producer == operation {
                return None;
            }
        }
        target_operations::TargetStructuralArgumentSource::Placement(placement) => {
            let parameter = signature
                .parameters
                .iter()
                .find(|parameter| parameter.semantic.place == semantic.place)?;
            if target.root_structural_type != parameter.semantic.structural_type
                || *placement != parameter.target.placement
            {
                return None;
            }
        }
        target_operations::TargetStructuralArgumentSource::EstablishedByteView { .. }
        | target_operations::TargetStructuralArgumentSource::BlockParameter { .. } => {
            if exclusive {
                return None;
            }
            crate::selection::established_view_input::accepts(source, operation, target)?;
        }
    }
    Some(())
}

/// Reconstruct the pointer displacement from source layout, never from a
/// coincidentally matching destination ABI or supplied byte offset alone.
fn exclusive_projection_shape(
    source: &LegalizedScalarFunction,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
) -> Option<ValueShape> {
    let signature = source.structural.as_ref()?;
    let parameter = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == semantic.place)?;
    if !matches!(
        (parameter.semantic.access, semantic.access),
        (
            StructuralAccess::MutableBorrow,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        ) | (
            StructuralAccess::WriteOnlyBorrow,
            StructuralAccess::WriteOnlyBorrow
        ) | (
            StructuralAccess::SharedBorrow,
            StructuralAccess::SharedBorrow
        )
    ) {
        return None;
    }
    let (referent, offset) = crate::structural_reference_input::project(
        parameter.semantic.structural_type,
        &semantic.path,
        &signature.structural_types,
    )?;
    let shape = crate::structural_reference_input::shape(referent, &signature.structural_types)?;
    (target.root_structural_type == parameter.semantic.structural_type
        && target.structural_type == referent
        && target.source_byte_offset == offset)
        .then_some(ValueShape::borrowed_reference(
            shape.byte_size,
            shape.alignment,
        ))
}
