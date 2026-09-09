//! Rejoin borrowed call arguments to their original referents and exact geometry.
use super::*;

/// Rejoin the original parameter or earlier literal before pointer transport.
/// Callee declarations and source-call correspondence remain legalization inputs.
pub(super) fn validate_borrowed_argument(
    source: &LegalizedScalarFunction,
    call: &LegalizedScalarCall,
    operation: semantic_vocabulary::OperationId,
) -> Option<()> {
    let signature = source.structural.as_ref()?;
    let (last, scalars) = call.arguments.split_last()?;
    let LegalizedScalarArgument::Structural { semantic, target } = last else {
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
    let scalar_shapes = scalars
        .iter()
        .map(|argument| {
            let LegalizedScalarArgument::Scalar {
                source: value,
                placement,
            } = argument
            else {
                return None;
            };
            let shape = scalar_shape(scalar_value_type(source, *value)?)?;
            (placement.shape == shape).then_some(shape)
        })
        .collect::<Option<Vec<_>>>()?;
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
    let shape = if let Some((offset, _)) = byte_view {
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
            parameters: scalar_shapes
                .into_iter()
                .chain(std::iter::once(shape))
                .collect(),
            result: if call.structural_result.is_some() {
                Some(
                    crate::selection::scalar_case_input::call_result(source, call)?
                        .1
                        .shape,
                )
            } else {
                call.result_placement
                    .as_ref()
                    .map(|_| ValueShape::integer(8, 8))
            },
        },
    )
    .ok()?;
    // Unit entry attachments and service ceilings are retained declaration
    // metadata; they do not add ABI arguments. The scalar-result attachment
    // family remains outside this transport contract.
    if (source.attachment.is_some() && !exclusive && source.call_plan.result.is_some())
        || source.ranked.is_some()
        || !signature.entry_claims.is_empty()
        || (source.call_plan.result.is_some() && !signature.published_service_ceiling.is_empty())
        || (!parameters.is_empty()
            && !crate::unobserved_owned_input::accepts(source)
            && !crate::structural_unit_input::accepts_borrowed_view(
                &source.call_plan,
                &parameters,
                &signature.structural_types,
            )
            && !crate::structural_unit_input::accepts_write_borrow(
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
            && (semantic.access != StructuralAccess::SharedBorrow || !semantic.path.is_empty()))
        || target.place != semantic.place
        || target.access != semantic.access
        || target.path != semantic.path
        || (!exclusive && target.root_structural_type != target.structural_type)
        || target.shape != shape
        || (!exclusive && target.source_byte_offset != 0)
        || target.fixed_array_length != byte_view.map(|(_, length)| length)
        || target.element_stride != byte_view.map(|_| 1)
        || Some(&target.destination) != expected.parameters.last()
    {
        return None;
    }
    match &target.source {
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
