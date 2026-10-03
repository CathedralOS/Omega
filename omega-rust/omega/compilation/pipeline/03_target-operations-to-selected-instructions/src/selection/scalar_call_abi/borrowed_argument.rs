//! Rejoin borrowed call arguments to their original referents and exact geometry.
use super::{
    CallSignature, LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarFunction,
    StructuralAccess, ValueShape, evaluate_call_plan, scalar_shape,
};
use crate::selection::scalar_call_abi::scalar_value_type;

/// Rejoin the original parameter or earlier literal before pointer transport.
/// Callee declarations and source-call correspondence remain legalization inputs.
pub(super) fn validate_borrowed_argument(
    source: &LegalizedScalarFunction,
    call: &LegalizedScalarCall,
    operation: semantic_vocabulary::OperationId,
    argument_index: usize,
) -> Option<()> {
    let signature = source.structural.as_ref()?;
    let LegalizedScalarArgument::Structural {
        semantic,
        target,
        indices,
    } = call.arguments.get(argument_index)?
    else {
        return None;
    };
    // A whole returned record and its children borrow through the completed
    // carrier's address word. Primitive static referents keep their route below.
    if (semantic.path.len() > 1
        || signature.structural_types.iter().any(|declaration| {
            declaration.id == target.structural_type
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::Record { .. }
                )
        }))
        && matches!(
            semantic.path.first(),
            Some(terminal_psi::StructuralPathSegment::Referent)
        )
    {
        return validate_returned_record_projection(
            source,
            call,
            operation,
            argument_index,
            semantic,
            target,
            indices,
            signature,
        );
    }
    // A `.., Referent` argument transports its referent root's pointer; the
    // carrier place names loan custody only and is never a pointer source.
    if matches!(
        semantic.path.last(),
        Some(terminal_psi::StructuralPathSegment::Referent)
    ) {
        return validate_referent_argument(
            source,
            call,
            operation,
            argument_index,
            semantic,
            target,
            signature,
        );
    }
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
        .map(
            |parameter| crate::structural_inputs::structural_unit_input::Parameter {
                semantic: &parameter.semantic,
                target: &parameter.target,
            },
        )
        .collect::<Vec<_>>();
    let exclusive = matches!(
        semantic.access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    );
    let local = crate::selection::primitive_local_input::local(source, semantic.place);
    let aggregate_home = crate::selection::aggregate_result_input::home(source, semantic.place);
    let aggregate_block_home =
        crate::selection::aggregate_result_input::block_home(source, semantic.place);
    let aggregate_root = aggregate_home
        .map(|(_, result)| result.structural_type)
        .or_else(|| aggregate_block_home.map(|(_, declaration)| declaration.structural_type))
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
    let aggregate = aggregate_root
        .filter(|root| {
            signature.structural_types.iter().any(|declaration| {
                declaration.id == *root
                    && matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::Record { .. }
                            | terminal_psi::StructuralTypeShape::Sum { .. }
                            | terminal_psi::StructuralTypeShape::Mixed { .. }
                            // A whole-place borrow of a produced fixed array
                            // rejoins the same aggregate home an aggregate
                            // borrow does.
                            | terminal_psi::StructuralTypeShape::FixedArray { .. }
                    )
            })
        })
        .and_then(|root| {
            let (selected, offset, elements) =
                crate::structural_inputs::structural_reference_input::runtime_projection(
                    root,
                    &semantic.path,
                    &signature.structural_types,
                )?;
            let shape = crate::structural_inputs::structural_reference_input::shape(
                selected,
                &signature.structural_types,
            )?;
            (target.root_structural_type == root
                && target.structural_type == selected
                && target.source_byte_offset == offset
                && crate::structural_inputs::structural_reference_input::runtime_indices_match(
                    indices, &elements,
                ))
            .then_some(ValueShape::borrowed_reference(
                shape.byte_size,
                shape.alignment,
            ))
        });
    // A whole owned bounded byte home lends its `{length, bytes}` storage to
    // a shared bounded-owner formal; the callee reads the owner layout in
    // place, so the pointer names the home itself at offset zero.
    let owner_home = aggregate_home
        .filter(|(_, result)| {
            semantic.path.is_empty()
                && semantic.access == StructuralAccess::SharedBorrow
                && target.root_structural_type == result.structural_type
                && target.structural_type == result.structural_type
                && target.source_byte_offset == 0
                && indices.is_empty()
                && signature.structural_types.iter().any(|declaration| {
                    declaration.id == result.structural_type
                        && matches!(
                            declaration.shape,
                            terminal_psi::StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                            )
                        )
                })
        })
        .and_then(|(_, result)| {
            crate::structural_inputs::structural_reference_input::shape(
                result.structural_type,
                &signature.structural_types,
            )
        })
        .map(|shape| ValueShape::borrowed_reference(shape.byte_size, shape.alignment));
    // A fixed array presents its backing from an incoming parameter or
    // from a local home lent through this argument's own borrow — Terminal's
    // array-result loan. The home keeps its owned access; `aggregate_home`
    // rejoins its producer below.
    let local_loan = aggregate_home
        .filter(|(_, result)| {
            matches!(
                semantic.access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            ) && result.multiplicity != terminal_psi::StructuralMultiplicity::Linear
        })
        .map(|(_, result)| terminal_psi::StructuralParameterDeclaration {
            place: result.place,
            position: 0,
            is_self: false,
            structural_type: result.structural_type,
            multiplicity: result.multiplicity,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    // An address join lends from its joined referent through the address
    // its block entry loaded; it is neither a home nor a descriptor view, yet
    // presents a fixed-array window of that referent as a parameter does.
    let address_join = crate::selection::address_join_input::join(source, semantic.place);
    let view_root = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == semantic.place)
        .map(|parameter| &parameter.semantic)
        .or(local_loan.as_ref())
        .or(address_join.map(|(_, joined)| joined));
    let byte_view = view_root.and_then(|root| {
        crate::structural_inputs::structural_reference_input::fixed_byte_array_view(
            root,
            semantic,
            target.structural_type,
            &signature.structural_types,
        )
    });
    // A bounded inline byte field presents as a borrowed view too; unlike the
    // fixed array it has no projected carrier identity, so the field's own
    // record geometry supplies the offset and the callee's type supplies the
    // view identity.
    let byte_field =
        crate::selection::byte_owner_input::view_argument(source, operation, semantic, target);
    // A stored shared view field lends the descriptor it already holds: the
    // pointer addresses that field, and the formal's view type is its referent.
    let stored_view = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == semantic.place)
        .and_then(|parameter| {
            crate::structural_inputs::structural_reference_input::stored_view_field_argument(
                &parameter.semantic,
                semantic,
                target.structural_type,
                &signature.structural_types,
            )
        })
        .filter(|offset| *offset == target.source_byte_offset && indices.is_empty());
    let byte_owner = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == semantic.place)
        .and_then(|parameter| {
            crate::structural_inputs::structural_reference_input::bounded_byte_field_owner(
                &parameter.semantic,
                semantic,
                target.structural_type,
                &signature.structural_types,
            )
        });
    let element_view = view_root.and_then(|root| {
        crate::structural_inputs::structural_reference_input::fixed_element_array_view(
            root,
            semantic,
            target.structural_type,
            &signature.structural_types,
        )
    });
    // Whole descriptors retain their exact carrier geometry; element loans
    // additionally carry a referent-relative projection. Their source is the
    // authenticated view chain, not an ordinary exclusive storage parameter.
    let established_view_loan =
        crate::selection::established_view_input::element_view_type(source, semantic.place)
            .or_else(|| {
                if !semantic.path.is_empty() {
                    return None;
                }
                crate::selection::established_view_input::view_access(source, semantic.place)?;
                crate::selection::established_view_input::view_type(source, semantic.place)
            })
            .and_then(|view_type| {
                let (selected, offset, elements) =
                    crate::structural_inputs::structural_reference_input::runtime_projection(
                        view_type,
                        &semantic.path,
                        &signature.structural_types,
                    )?;
                if target.root_structural_type != view_type
                    || target.structural_type != selected
                    || target.source_byte_offset != offset
                    || target.fixed_array_length.is_some()
                    || target.element_stride.is_some()
                    || !crate::structural_inputs::structural_reference_input::runtime_indices_match(
                        indices, &elements,
                    )
                {
                    return None;
                }
                let referent = crate::structural_inputs::structural_reference_input::shape(
                    selected,
                    &signature.structural_types,
                )?;
                Some(ValueShape::borrowed_reference(
                    referent.byte_size,
                    referent.alignment,
                ))
            });
    let window = byte_view
        .map(|(offset, _)| offset)
        .or(element_view.map(|(offset, _, _)| offset));
    // Descriptor geometry follows the exact destination declaration, not
    // whether its backing arrived as a parameter, local, or control-flow join.
    let destination_carrier = crate::structural_inputs::structural_reference_input::shape(
        target.structural_type,
        &signature.structural_types,
    )?;
    let destination_shape = ValueShape::borrowed_reference(
        destination_carrier.byte_size,
        destination_carrier.alignment,
    );
    let shape = if byte_field.is_some() {
        destination_shape
    } else if let Some((_, joined)) = address_join {
        if !matches!(
            semantic.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) || semantic.access == StructuralAccess::MutableBorrow
            && joined.access != StructuralAccess::MutableBorrow
        {
            return None;
        }
        if let Some(offset) = window {
            // The window's backing keeps its real root; the descriptor names
            // the callee's view type, checked with the length word below.
            if target.root_structural_type != joined.structural_type
                || target.source_byte_offset != offset
                || !indices.is_empty()
            {
                return None;
            }
            destination_shape
        } else {
            let (selected, offset, elements) =
                crate::structural_inputs::structural_reference_input::runtime_projection(
                    joined.structural_type,
                    &semantic.path,
                    &signature.structural_types,
                )?;
            if target.root_structural_type != joined.structural_type
                || target.structural_type != selected
                || target.source_byte_offset != offset
                || !crate::structural_inputs::structural_reference_input::runtime_indices_match(
                    indices, &elements,
                )
            {
                return None;
            }
            let referent = crate::structural_inputs::structural_reference_input::shape(
                selected,
                &signature.structural_types,
            )?;
            ValueShape::borrowed_reference(referent.byte_size, referent.alignment)
        }
    } else if let Some(shape) = aggregate {
        shape
    } else if let Some(shape) = owner_home {
        shape
    } else if let Some((offset, _)) = byte_view {
        if offset != target.source_byte_offset {
            return None;
        }
        destination_shape
    } else if let Some((offset, capacity)) = byte_owner {
        if offset != target.source_byte_offset {
            return None;
        }
        ValueShape::borrowed_reference(u32::try_from(capacity.checked_add(8)?).ok()?, 8)
    } else if let Some((offset, _)) = byte_field {
        if offset != target.source_byte_offset {
            return None;
        }
        destination_shape
    } else if let Some((offset, _, _)) = element_view {
        if offset != target.source_byte_offset {
            return None;
        }
        destination_shape
    } else if let Some(shape) = established_view_loan {
        shape
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
        destination_shape
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
            // The placement shape is the ABI input; reference-bearing
            // structural results have no physical home to rejoin here.
            result: call
                .result_placement
                .as_ref()
                .map(|placement| placement.shape),
        },
    )
    .ok()?;
    // Attachment names the enclosing declaration, not an extra argument or
    // a different pointer ABI for scalar-returning callers. Rejoin that type
    // and any explicit receiver; static attached methods have no receiver.
    // The checks below still reconstruct every incoming/outgoing placement,
    // referent, projection, and access mode independently.
    if source.attachment.is_some_and(|attachment| {
        !signature
            .structural_types
            .iter()
            .any(|declaration| declaration.id == attachment)
            || signature.parameters.iter().any(|parameter| {
                parameter.semantic.is_self && parameter.semantic.structural_type != attachment
            })
    }) || !signature.entry_claims.is_empty()
        || (!parameters.is_empty()
            && !crate::structural_inputs::unobserved_owned_input::accepts(source)
            && !crate::structural_inputs::structural_unit_input::accepts_graph(
                &source.call_plan,
                &parameters,
                &signature.structural_types,
            ))
        || !call.claim_transfers.is_empty()
        || call.call_plan != expected
        || (!exclusive
            && address_join.is_none()
            && aggregate.is_none()
            && owner_home.is_none()
            && byte_view.is_none()
            && byte_field.is_none()
            && stored_view.is_none()
            && byte_owner.is_none()
            && element_view.is_none()
            && established_view_loan.is_none()
            && (semantic.access != StructuralAccess::SharedBorrow || !semantic.path.is_empty()))
        || target.place != semantic.place
        || target.access != semantic.access
        || target.path != semantic.path
        || (!exclusive
            && address_join.is_none()
            && aggregate.is_none()
            && owner_home.is_none()
            && byte_view.is_none()
            && byte_field.is_none()
            && stored_view.is_none()
            && byte_owner.is_none()
            && element_view.is_none()
            && established_view_loan.is_none()
            && target.root_structural_type != target.structural_type)
        || target.shape != shape
        || (!exclusive
            && address_join.is_none()
            && aggregate.is_none()
            && owner_home.is_none()
            && byte_view.is_none()
            && byte_field.is_none()
            && stored_view.is_none()
            && byte_owner.is_none()
            && element_view.is_none()
            && established_view_loan.is_none()
            && target.source_byte_offset != 0)
        || target.fixed_array_length
            != byte_view
                .map(|(_, length)| length)
                .or(element_view.map(|(_, length, _)| length))
        || target.element_stride
            != byte_view
                .map(|_| 1)
                .or(element_view.map(|(_, _, stride)| stride))
        || Some(&target.destination) != expected.parameters.get(argument_index)
    {
        return None;
    }
    match &target.source {
        _ if byte_field.is_some() => {}
        target_operations::TargetStructuralArgumentSource::StructuralHome { psi_operation } => {
            let (producer, _) = aggregate_home?;
            let home_view = local_loan.is_some() && (byte_view.is_some() || element_view.is_some());
            let descriptor_home = established_view_loan.is_some()
                && semantic.access == StructuralAccess::SharedBorrow
                && crate::selection::established_view_input::descriptor_result(
                    source,
                    semantic.place,
                )
                .is_some_and(|(actual, _)| actual == producer);
            if (aggregate.is_none() && !home_view && owner_home.is_none() && !descriptor_home)
                || *psi_operation != producer
                || producer == operation
            {
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
        target_operations::TargetStructuralArgumentSource::BlockParameter { block, place }
            if address_join.is_some() =>
        {
            let (owner, joined) = address_join?;
            if *block != owner || *place != joined.place {
                return None;
            }
        }
        target_operations::TargetStructuralArgumentSource::BlockParameter { block, place }
            if aggregate.is_some() =>
        {
            let (owner, declaration) = aggregate_block_home?;
            if semantic.access != StructuralAccess::SharedBorrow
                || *block != owner
                || *place != declaration.place
            {
                return None;
            }
        }
        target_operations::TargetStructuralArgumentSource::EstablishedElementView { .. }
            if established_view_loan.is_some() =>
        {
            // A descriptor-rooted element loan loans mutable or shared —
            // exclusive is exactly the mutable view's own authority.
            if semantic.path.is_empty() {
                crate::selection::established_view_input::accepts(source, operation, target)?;
            } else {
                crate::selection::established_view_input::accepts_view_loan(
                    source, operation, target,
                )?;
            }
        }
        target_operations::TargetStructuralArgumentSource::EstablishedByteView { .. }
        | target_operations::TargetStructuralArgumentSource::EstablishedElementView { .. }
        | target_operations::TargetStructuralArgumentSource::BlockParameter { .. } => {
            crate::selection::established_view_input::accepts(source, operation, target)?;
        }
    }
    Some(())
}

/// Validate the physical pointer row for a child borrowed through one
/// returned mutable-record carrier. Selection checks the completed carrier
/// home and referent-relative geometry. The possible origins remain guarded by
/// `validate_optimized_selection_custody`, which re-legalizes this immutable
/// plan from the raw target/reference-result roster before publication.
fn validate_returned_record_projection(
    source: &LegalizedScalarFunction,
    call: &LegalizedScalarCall,
    operation: semantic_vocabulary::OperationId,
    argument_index: usize,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    signature: &legalized_operations::LegalizedStructuralContract,
) -> Option<()> {
    let (terminal_psi::StructuralPathSegment::Referent, suffix) = semantic.path.split_first()?
    else {
        return None;
    };
    if !suffix.iter().all(|segment| {
        matches!(
            segment,
            terminal_psi::StructuralPathSegment::Field(_)
                | terminal_psi::StructuralPathSegment::FixedIndex(_)
        )
    }) || semantic.access == StructuralAccess::Owned
        || target.access != semantic.access
        || target.fixed_array_length.is_some()
        || target.element_stride.is_some()
        || !indices.is_empty()
        || target.path != semantic.path
    {
        return None;
    }
    let mut producers = source
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match &instruction.kind {
            legalized_operations::LegalizedScalarInstructionKind::Call(produced)
                if instruction.operation != operation
                    && produced
                        .structural_result
                        .as_ref()
                        .is_some_and(|result| result.place == semantic.place) =>
            {
                Some((instruction.operation, produced))
            }
            _ => None,
        });
    let (producer, produced) = producers.next()?;
    if producers.next().is_some() {
        return None;
    }
    let result = produced.structural_result.as_ref()?;
    if producer == operation
        || result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.qualification_establishments.is_empty()
        || !result.claims.is_empty()
    {
        return None;
    }
    let (referent, access) = signature
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)
        .and_then(|declaration| match &declaration.shape {
            terminal_psi::StructuralTypeShape::Reference { referent, access } => {
                Some((*referent, *access))
            }
            _ => None,
        })?;
    if access != StructuralAccess::MutableBorrow
        || !signature.structural_types.iter().any(|declaration| {
            declaration.id == referent
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::Record { .. }
                )
        })
    {
        return None;
    }
    // Rejoin the actual producer and full authored crossing. A static ingress
    // re-root is no longer an alternative representation of this argument:
    // selection must load the address returned by the executed callee.
    crate::selection::reference_input::borrowed_argument(source, semantic, target)?;
    let shape = target.shape;
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
                        let value_shape = scalar_shape(scalar_value_type(source, *value)?)?;
                        (placement.shape == value_shape).then_some(value_shape)
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
            result: call
                .result_placement
                .as_ref()
                .map(|placement| placement.shape),
        },
    )
    .ok()?;
    (call.call_plan == expected
        && Some(&target.destination) == expected.parameters.get(argument_index))
    .then_some(())
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
    let (referent, offset) = crate::structural_inputs::structural_reference_input::project(
        parameter.semantic.structural_type,
        &semantic.path,
        &signature.structural_types,
    )?;
    let shape = crate::structural_inputs::structural_reference_input::shape(
        referent,
        &signature.structural_types,
    )?;
    (target.root_structural_type == parameter.semantic.structural_type
        && target.structural_type == referent
        && target.source_byte_offset == offset)
        .then_some(ValueShape::borrowed_reference(
            shape.byte_size,
            shape.alignment,
        ))
}

/// A `.., Referent` argument transports its referent root's pointer; the
/// carrier place names loan custody only. Legalization's custody replay
/// resolved the root; this join re-derives the same ABI identity from the
/// retained argument metadata alone.
fn validate_referent_argument(
    source: &LegalizedScalarFunction,
    call: &LegalizedScalarCall,
    operation: semantic_vocabulary::OperationId,
    argument_index: usize,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
    signature: &legalized_operations::LegalizedStructuralContract,
) -> Option<()> {
    let (last, carrier_path) = semantic.path.split_last()?;
    if !matches!(last, terminal_psi::StructuralPathSegment::Referent)
        || !carrier_path
            .iter()
            .all(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Field(_)))
        || semantic.access == StructuralAccess::Owned
        || target.access != semantic.access
        || target.path != semantic.path
        || target.source_byte_offset != 0
        || target.fixed_array_length.is_some()
        || target.element_stride.is_some()
        || target.root_structural_type != target.structural_type
    {
        return None;
    }
    let scalar = signature
        .structural_types
        .iter()
        .find(|declaration| declaration.id == target.structural_type)
        .and_then(|declaration| match declaration.shape {
            terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar) => Some(scalar),
            _ => None,
        })?;
    let referent = scalar_shape(scalar)?;
    let shape = ValueShape::borrowed_reference(referent.byte_size, referent.alignment);
    if target.shape != shape || target.destination.shape != shape {
        return None;
    }
    match &target.source {
        target_operations::TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
            psi_operation,
        } => {
            let (producer, result, local_scalar, _) =
                crate::selection::primitive_local_input::local(source, target.place)?;
            if *psi_operation != producer
                || producer == operation
                || result.structural_type != target.structural_type
                || local_scalar != scalar
            {
                return None;
            }
        }
        target_operations::TargetStructuralArgumentSource::Placement(placement) => {
            let parameter = signature
                .parameters
                .iter()
                .find(|parameter| parameter.semantic.place == target.place)?;
            let allowed = match parameter.semantic.access {
                StructuralAccess::MutableBorrow => true,
                StructuralAccess::SharedBorrow => semantic.access == StructuralAccess::SharedBorrow,
                StructuralAccess::WriteOnlyBorrow => {
                    semantic.access == StructuralAccess::WriteOnlyBorrow
                }
                StructuralAccess::Owned => false,
            };
            if !allowed
                || parameter.semantic.structural_type != target.structural_type
                || parameter.semantic.is_self
                || *placement != parameter.target.placement
            {
                return None;
            }
        }
        _ => return None,
    }
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
            result: call
                .result_placement
                .as_ref()
                .map(|placement| placement.shape),
        },
    )
    .ok()?;
    if call.call_plan != expected
        || Some(&target.destination) != expected.parameters.get(argument_index)
    {
        return None;
    }
    Some(())
}
