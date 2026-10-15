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
    // Transfers index structural arguments, not the interleaved ABI roster.
    // A loan cannot consume a claim, including through a stored-reference
    // crossing. Owned siblings keep their own call-local transfers.
    let structural_argument_index = call.arguments[..argument_index]
        .iter()
        .filter(|argument| matches!(argument, LegalizedScalarArgument::Structural { .. }))
        .count();
    if call
        .claim_transfers
        .iter()
        .any(|transfer| transfer.argument_index as usize == structural_argument_index)
    {
        return None;
    }
    if semantic
        .path
        .iter()
        .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Referent))
        || crate::selection::reference_input::borrowed_argument(source, semantic, target, indices)
            .is_some()
    {
        return validate_reference_argument(
            source,
            call,
            operation,
            argument_index,
            semantic,
            target,
            indices,
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
    let parameters = crate::structural_inputs::structural_unit_input::parameter_views(signature);
    let exclusive = matches!(
        semantic.access,
        terminal_psi::exclusive_borrow_pair_pattern!()
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
                        terminal_psi::mutating_access_pair_pattern!()
                    ) || parameter.semantic.access == semantic.access
                })
                .map(|parameter| parameter.semantic.structural_type)
        });
    // Every owned aggregate home lends its original storage. Storage geometry
    // already distinguishes aggregates from scalar and borrowed descriptors;
    // a bounded byte owner needs no second whole-place, readable-only route.
    // Access and source custody stay independent of that physical distinction.
    let aggregate = aggregate_root
        .filter(|root| {
            crate::structural_inputs::structural_reference_input::owned_storage_shape(
                *root,
                &signature.structural_types,
            )
            .is_some()
        })
        .and_then(|root| {
            let (selected, offset, elements) =
                crate::structural_inputs::structural_reference_input::runtime_borrowed_projection(
                    root,
                    &semantic.path,
                    semantic.access,
                    Some(target.structural_type),
                    &signature.structural_types,
                )?;
            let shape =
                crate::structural_inputs::structural_reference_input::internal_borrow_shape(
                    selected,
                    &signature.structural_types,
                )?;
            (target.root_structural_type == root
                && matches!(
                    terminal_semantics::borrowed_referent_presentation(
                        signature.structural_types.iter(),
                        selected,
                        target.structural_type,
                    ),
                    Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
                )
                && target.source_byte_offset == offset
                && crate::structural_inputs::structural_reference_input::runtime_indices_match(
                    indices, &elements,
                ))
            .then_some(shape)
        });
    // A fixed array presents its backing from an incoming parameter or
    // from a local home lent through this argument's own borrow — Terminal's
    // array-result loan. The home keeps its owned access; `aggregate_home`
    // rejoins its producer below.
    let local_loan = aggregate_home
        .filter(|(_, result)| {
            matches!(
                semantic.access,
                terminal_psi::reading_borrow_pair_pattern!()
            ) && result.multiplicity != terminal_psi::StructuralMultiplicity::Linear
        })
        .map(|(_, result)| terminal_psi::StructuralParameterDeclaration {
            place: result.place,
            position: 0,
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
        .or(address_join.map(|(_, joined)| joined))
        // A bounded view arriving at a state block carries the same backing
        // pointer as an incoming formal. Its exact declared extent supplies a
        // dynamic call descriptor; block-origin custody is rejoined below.
        .or_else(|| {
            source.block_structural_parameters().find(|parameter| {
                parameter.place == semantic.place
                    && signature.structural_types.iter().any(|declaration| {
                        declaration.id == parameter.structural_type
                            && matches!(
                                declaration.shape,
                                terminal_psi::StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                                )
                            )
                    })
            })
        });
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
    let byte_field = crate::selection::byte_owner_input::view_argument(
        source, operation, semantic, target, indices,
    );
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
            crate::structural_inputs::structural_reference_input::runtime_borrowed_projection(
                view_type,
                &semantic.path,
                semantic.access,
                Some(target.structural_type),
                &signature.structural_types,
            )?;
                if target.root_structural_type != view_type
                    || !terminal_semantics::borrowed_referent_matches(
                        signature.structural_types.iter(),
                        selected,
                        target.structural_type,
                    )
                    || target.source_byte_offset != offset
                    || target.fixed_array_length.is_some()
                    || target.element_stride.is_some()
                    || !crate::structural_inputs::structural_reference_input::runtime_indices_match(
                        indices, &elements,
                    )
                {
                    return None;
                }
                let shape =
                    crate::structural_inputs::structural_reference_input::internal_borrow_shape(
                        selected,
                        &signature.structural_types,
                    )?;
                Some(shape)
            });
    let window = byte_view
        .map(|(offset, _)| offset)
        .or(element_view.map(|(offset, _, _)| offset));
    // Descriptor geometry follows the exact destination declaration, not
    // whether its backing arrived as a parameter, local, or control-flow join.
    let destination_shape =
        crate::structural_inputs::structural_reference_input::internal_borrow_shape(
            target.structural_type,
            &signature.structural_types,
        )?;
    // One place projected inside a borrowed root the caller holds (an
    // incoming parameter or a state-block arrival), shared or exclusive.
    let projected_loan = projected_loan_shape(source, semantic, target, indices);
    // A loan arriving as a block parameter and forwarded whole as its own
    // declared type keeps its carrier. The source arm below rejoins that block
    // parameter's declared access, type and position.
    let block_forward = semantic.path.is_empty()
        && target.root_structural_type == target.structural_type
        && target.source_byte_offset == 0
        && indices.is_empty()
        && matches!(
            target.source,
            target_operations::TargetStructuralArgumentSource::BlockParameter { .. }
        );
    let shape = if byte_field.is_some() {
        destination_shape
    } else if let Some((_, joined)) = address_join {
        if !borrow_access_permitted(joined.access, semantic.access) {
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
                crate::structural_inputs::structural_reference_input::runtime_borrowed_projection(
                    joined.structural_type,
                    &semantic.path,
                    semantic.access,
                    Some(target.structural_type),
                    &signature.structural_types,
                )?;
            if target.root_structural_type != joined.structural_type
                || !terminal_semantics::borrowed_referent_matches(
                    signature.structural_types.iter(),
                    selected,
                    target.structural_type,
                )
                || target.source_byte_offset != offset
                || !crate::structural_inputs::structural_reference_input::runtime_indices_match(
                    indices, &elements,
                )
            {
                return None;
            }
            crate::structural_inputs::structural_reference_input::internal_borrow_shape(
                selected,
                &signature.structural_types,
            )?
        }
    } else if let Some(shape) = aggregate {
        shape
    } else if block_forward && aggregate_block_home.is_none() {
        destination_shape
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
    } else if let Some((_, result, _, _)) = local {
        if target.structural_type != result.structural_type
            || target.root_structural_type != result.structural_type
            || !semantic.path.is_empty()
            || target.source_byte_offset != 0
        {
            return None;
        }
        crate::structural_inputs::structural_reference_input::internal_borrow_shape(
            result.structural_type,
            &signature.structural_types,
        )?
    } else if let Some(shape) = projected_loan {
        shape
    } else if exclusive
        || signature.structural_types.iter().any(|declaration| {
            declaration.id == target.structural_type
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
                )
        })
    {
        return None;
    } else {
        destination_shape
    };
    let expected = expected_signature_plan(source, call, argument_index, shape)?;
    // Attachment names the enclosing declaration, not an extra argument or
    // a different pointer ABI for scalar-returning callers. Rejoin that type;
    // every argument has its own explicit declaration and placement.
    // The checks below still reconstruct every incoming/outgoing placement,
    // referent, projection, and access mode independently.
    let no_indirect_home = !exclusive
        && address_join.is_none()
        && aggregate.is_none()
        && byte_view.is_none()
        && byte_field.is_none()
        && byte_owner.is_none()
        && element_view.is_none()
        && established_view_loan.is_none()
        && projected_loan.is_none();
    if source.attachment.is_some_and(|attachment| {
        !terminal_psi::find_structural_type(&signature.structural_types, attachment).is_some()
    }) || (!parameters.is_empty()
        && !crate::structural_inputs::structural_unit_input::accepts_graph(
            &source.call_plan,
            &parameters,
            &signature.structural_types,
        ))
        || call.call_plan != expected
        || (no_indirect_home
            && (semantic.access != StructuralAccess::SharedBorrow || !semantic.path.is_empty()))
        || target.place != semantic.place
        || target.access != semantic.access
        || target.path != semantic.path
        || (no_indirect_home && target.root_structural_type != target.structural_type)
        || target.shape != shape
        || (no_indirect_home && target.source_byte_offset != 0)
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
                && crate::selection::established_view_input::view_access(source, semantic.place)
                    .is_some_and(|access| {
                        access == semantic.access
                            || access == StructuralAccess::MutableBorrow
                                && semantic.access == StructuralAccess::SharedBorrow
                    })
                && crate::selection::established_view_input::completed_descriptor(
                    source,
                    semantic.place,
                )
                .is_some_and(|(actual, _)| actual == producer);
            if (aggregate.is_none() && !home_view && !descriptor_home)
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
            // The exact block home retains owned backing. Shared, mutable,
            // and write-only calls borrow that same storage; their access and
            // outgoing ABI were checked above independently of its origin.
            let (owner, declaration) = aggregate_block_home?;
            if *block != owner || *place != declaration.place {
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

/// Reconstruct a reference leaf's address independently of its producer family.
fn validate_reference_argument(
    source: &LegalizedScalarFunction,
    call: &LegalizedScalarCall,
    operation: semantic_vocabulary::OperationId,
    argument_index: usize,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
) -> Option<()> {
    if matches!(target.source, target_operations::TargetStructuralArgumentSource::StructuralHome { psi_operation } if psi_operation == operation)
    {
        return None;
    }
    crate::selection::reference_input::borrowed_argument(source, semantic, target, indices)?;
    let shape = target.shape;
    let expected = expected_signature_plan(source, call, argument_index, shape)?;
    (call.call_plan == expected
        && Some(&target.destination) == expected.parameters.get(argument_index))
    .then_some(())
}

/// Entry and block loans retain identical authority at a call. An address
/// or matching ABI shape cannot supply permission absent from that loan.
fn borrow_access_permitted(source: StructuralAccess, requested: StructuralAccess) -> bool {
    use StructuralAccess::{MutableBorrow, SharedBorrow, WriteOnlyBorrow};
    matches!(
        (source, requested),
        (
            MutableBorrow,
            SharedBorrow | MutableBorrow | WriteOnlyBorrow
        ) | (SharedBorrow, SharedBorrow)
            | (WriteOnlyBorrow, WriteOnlyBorrow)
    )
}

/// A loan of one place projected inside a borrowed root the caller holds:
/// an incoming parameter or a state-block arrival. Reconstruct the pointer
/// displacement from source layout, never from a coincidentally matching
/// destination ABI or supplied byte offset alone. Runtime selectors rejoin
/// the target's indices; a view root's element sits behind its backing
/// pointer, which emission resolves before applying the offset.
fn projected_loan_shape(
    source: &LegalizedScalarFunction,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
) -> Option<ValueShape> {
    let signature = source.structural.as_ref()?;
    let root = crate::selection::arrival_parameter_input::parameter(source, semantic.place)?;
    if !borrow_access_permitted(root.access, semantic.access) {
        return None;
    }
    let (referent, offset, elements) =
        crate::structural_inputs::structural_reference_input::runtime_borrowed_projection(
            root.structural_type,
            &semantic.path,
            semantic.access,
            Some(target.structural_type),
            &signature.structural_types,
        )?;
    let shape = crate::structural_inputs::structural_reference_input::internal_borrow_shape(
        referent,
        &signature.structural_types,
    )?;
    (target.root_structural_type == root.structural_type
        && target.structural_type == referent
        && target.source_byte_offset == offset
        && crate::structural_inputs::structural_reference_input::runtime_indices_match(
            indices, &elements,
        ))
    .then_some(shape)
}

/// The call plan the argument rosters imply when the `argument_index` slot
/// claims `shape`; `None` when a scalar's declared placement disagrees with
/// its value type or plan evaluation rejects the signature.
fn expected_signature_plan(
    source: &LegalizedScalarFunction,
    call: &LegalizedScalarCall,
    argument_index: usize,
    shape: calling_conventions::ValueShape,
) -> Option<calling_conventions::CallPlan> {
    evaluate_call_plan(
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
    .ok()
}
