//! Source and target joins for borrowed arguments in ordinary helper calls.
use crate::LegalizationError;
use abstract_operations::AbstractOperationPlan;
use calling_conventions::{CallPlan, ValueShape};
use optimization_unit::PsiOptimizationFunction;
use target_operations::{TargetOperationPlan, TargetStructuralArgument};
use terminal_psi::{StructuralAccess, StructuralArgument, find_structural_type};

mod array_homes;
mod byte_views;
mod exclusive;

/// Rejoin one authored argument using its declaration ordinal and the shared call plan.
pub(in crate::legalization) fn argument_at(
    semantic: &StructuralArgument,
    position: usize,
    call_operation: semantic_vocabulary::OperationId,
    caller: &PsiOptimizationFunction,
    called: abstract_operations::AbstractCallSignature<'_>,
    call: &CallPlan,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    custody: &super::reference_custody::Custody,
) -> Result<TargetStructuralArgument, LegalizationError> {
    let declaration = crate::legalization::scalar_graph_input::call_signature::parameter(
        called,
        position,
        semantic.place,
    )?;
    let destination_parameter = &declaration;
    let parameter_ordinal = called
        .scalar_parameter_count()
        .checked_add(position)
        .ok_or(LegalizationError::custody())?;
    // A reference crossing retains the exact carrier that stores its address.
    if semantic
        .path
        .iter()
        .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Referent))
    {
        return super::reference_custody::referent_argument(
            semantic,
            destination_parameter,
            call.parameters
                .get(parameter_ordinal)
                .ok_or(LegalizationError::custody())?,
            caller,
            native,
            call_operation,
            custody,
            &plan.structural_types,
        );
    }
    // An address join lends a projection of its joined referent; its carrier is
    // neither a producer home nor an entrance parameter.
    if super::address_joins::parameter(caller, semantic.place, plan).is_some() {
        return super::address_joins::call_argument(
            semantic,
            destination_parameter,
            call_operation,
            caller,
            call,
            parameter_ordinal,
            native,
            plan,
        );
    }
    // Aggregate transport rejoins the existing home for every access mode.
    // Records, arrays and byte owners retain their ordinary owned geometry:
    // whether a root arrived or was constructed cannot select a borrow ABI.
    // In particular, write-only access does not make a local home an incoming
    // pointer, and it must not allocate a referent copy.
    if semantic.access == StructuralAccess::Owned
        || crate::structural_inputs::structural_reference_input::owned_storage_shape(
            destination_parameter.structural_type,
            &plan.structural_types,
        )
        .is_some()
    {
        return super::aggregate_results::call_argument(
            semantic,
            position,
            call_operation,
            caller,
            called,
            call,
            native,
            plan,
            custody,
        );
    }
    // A typed subtree borrows its root's retained storage, whether the root
    // arrived or was established here. This includes stored descriptors and
    // declared byte owners, not only scalar leaves. A descriptor endpoint
    // forwards its existing payload in every admitted access mode; only a
    // different owner-to-view presentation needs the adapter below.
    let root_type = caller
        .arrival_parameter(semantic.place)
        .map(|parameter| parameter.structural_type)
        .or_else(|| {
            caller
                .structural_place(semantic.place)
                .and_then(|place| place.kind.produced_structural_type())
        });
    if !semantic.path.is_empty()
        && root_type.is_some_and(|root| {
            crate::structural_inputs::structural_reference_input::runtime_borrowed_projection(
                root,
                &semantic.path,
                semantic.access,
                Some(destination_parameter.structural_type),
                &plan.structural_types,
            )
            .is_some_and(|(selected, _, _)| {
                matches!(
                    terminal_semantics::borrowed_referent_presentation(
                        plan.structural_types.iter(),
                        selected,
                        destination_parameter.structural_type,
                    ),
                    Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
                )
            })
        })
    {
        return super::aggregate_results::borrowed_arguments::reconstruct(
            semantic,
            position,
            call_operation,
            caller,
            called,
            call,
            native,
            plan,
        );
    }
    if let Some(argument) = byte_views::argument(
        semantic,
        call_operation,
        caller,
        destination_parameter,
        call,
        parameter_ordinal,
        native,
        plan,
    )? {
        return Ok(argument);
    }
    // A dominating local array lends its producing home through a
    // borrowed byte or element view: the same presentation an incoming
    // parameter's fixed-array window takes, rejoined to the producer.
    if let Some(argument) = array_homes::argument(
        semantic,
        call_operation,
        caller,
        destination_parameter,
        call,
        parameter_ordinal,
        plan,
        native,
    )? {
        return Ok(argument);
    }
    // Whole borrowed descriptors have one source/placement join regardless of
    // whether the caller lends shared or exclusive access. Referent projections
    // continue through their storage-geometry owner below.
    if semantic.path.is_empty()
        && plan.structural_types.iter().any(|declaration| {
            declaration.id == destination_parameter.structural_type
                && (declaration.shape.is_borrowed_byte_carrier()
                    || matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::ElementView { .. }
                    ))
        })
    {
        return view_argument(
            semantic,
            call_operation,
            caller,
            destination_parameter,
            call,
            parameter_ordinal,
            native,
            &plan.structural_types,
        );
    }
    // A mutable byte-view field has no arrival parameter: the record owns the
    // descriptor and the call lends that field. Replay the same home, path,
    // and offset the lowering already emitted.
    if semantic.path.len() == 1
        && matches!(
            semantic.path[0],
            terminal_psi::StructuralPathSegment::Field(_)
        )
        && matches!(
            semantic.access,
            terminal_psi::exclusive_borrow_pair_pattern!()
        )
        && caller.arrival_parameter(semantic.place).is_none()
    {
        return stored_mutable_view_field(
            semantic,
            call_operation,
            caller,
            destination_parameter,
            call,
            parameter_ordinal,
            native,
            plan,
        );
    }
    if super::primitive_locals::producer(caller, semantic.place).is_some()
        || matches!(
            semantic.access,
            terminal_psi::exclusive_borrow_pair_pattern!()
        )
        || semantic.access == StructuralAccess::SharedBorrow
            && super::primitive_locals::scalar(
                &plan.structural_types,
                destination_parameter.structural_type,
            )
            .is_some()
    {
        return primitive_argument(
            semantic,
            call_operation,
            caller,
            destination_parameter,
            call,
            parameter_ordinal,
            native,
            plan,
        );
    }
    if caller.structural_parameters.iter().any(|source| {
        terminal_semantics::fixed_byte_array_window(
            plan.structural_types.iter(),
            source,
            semantic,
            destination_parameter,
        )
        .is_some()
    }) {
        return primitive_argument(
            semantic,
            call_operation,
            caller,
            destination_parameter,
            call,
            parameter_ordinal,
            native,
            plan,
        );
    }
    // A shared argument lends its arrival's storage at a static projection:
    // the whole root (a `&mut [T; N]` parameter lent as `&[T; N]`) or a field
    // or element of it. Nested records, stored byte views, and bounded inline
    // byte fields all reconstruct through the same exclusive-argument custody
    // checks.
    if semantic.access == StructuralAccess::SharedBorrow
        && semantic.path.iter().all(|segment| {
            matches!(
                segment,
                terminal_psi::StructuralPathSegment::Field(_)
                    | terminal_psi::StructuralPathSegment::FixedIndex(_)
            )
        })
    {
        return primitive_argument(
            semantic,
            call_operation,
            caller,
            destination_parameter,
            call,
            parameter_ordinal,
            native,
            plan,
        );
    }
    // A runtime-indexed shared borrow loans one array element: the aggregate
    // route resolves each selector's dominating operand source and the
    // element stride onto the reconstructed argument.
    if semantic.access == StructuralAccess::SharedBorrow
        && semantic.path.iter().any(|segment| {
            matches!(
                segment,
                terminal_psi::StructuralPathSegment::RuntimeIndex { .. }
            )
        })
    {
        return super::aggregate_results::call_argument(
            semantic,
            position,
            call_operation,
            caller,
            called,
            call,
            native,
            plan,
            custody,
        );
    }
    Err(LegalizationError::custody())
}

/// Replay a mutable byte-view field loan from the record that stores it.
///
/// Lowering emits the record place, the field path, and the record
/// establishment as the source. Selection admits that loan. This replay has
/// to reproduce it: the field is not an arrival parameter.
fn stored_mutable_view_field(
    semantic: &StructuralArgument,
    call_operation: semantic_vocabulary::OperationId,
    caller: &PsiOptimizationFunction,
    destination_parameter: &terminal_psi::StructuralParameterDeclaration,
    call: &CallPlan,
    parameter_ordinal: usize,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
) -> Result<TargetStructuralArgument, LegalizationError> {
    let terminal_psi::StructuralPathSegment::Field(identity) = semantic.path[0] else {
        return Err(LegalizationError::custody());
    };
    let _child =
        established_record_field_place(caller, &plan.structural_types, semantic.place, identity)
            .ok_or(LegalizationError::custody())?;
    let declaration = caller
        .structural_place(semantic.place)
        .ok_or(LegalizationError::custody())?;
    let semantic_vocabulary::StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return Err(LegalizationError::custody());
    };
    let (field_shape, offset, elements) =
        crate::structural_inputs::structural_reference_input::collection_projection(
            structural_type,
            &semantic.path,
            &plan.structural_types,
        )
        .ok_or(LegalizationError::custody())?;
    if !elements.is_empty()
        || !matches!(
            field_shape.as_ref(),
            terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
            )
        )
        || destination_parameter.access != semantic.access
        || !terminal_semantics::view_descriptor_parameter(
            destination_parameter,
            plan.structural_types.iter(),
        )
    {
        return Err(LegalizationError::custody());
    }
    let placement = call
        .parameters
        .get(parameter_ordinal)
        .ok_or(LegalizationError::custody())?;
    Ok(TargetStructuralArgument {
        place: semantic.place,
        access: semantic.access,
        path: semantic.path.clone(),
        root_structural_type: structural_type,
        structural_type: destination_parameter.structural_type,
        shape: placement.shape,
        source_byte_offset: offset,
        fixed_array_length: None,
        element_stride: None,
        indices: super::target::control_flow::sources::call_indices(
            native,
            caller,
            target_operations::CallSiteOwner::Operation(call_operation),
            &[],
        )?,
        source: target_operations::TargetStructuralArgumentSource::StructuralHome {
            psi_operation: producer,
        },
        destination: placement.clone(),
    })
}

/// The place an `EstablishRecord` stores whole into `field`.
///
/// The initializer may be the owned descriptor or the loan that filled it.
/// A pathed initializer is a projection, not this field's own place.
fn established_record_field_place(
    caller: &PsiOptimizationFunction,
    types: &[terminal_psi::StructuralTypeDeclaration],
    place: semantic_vocabulary::PlaceId,
    field_identity: terminal_psi::TerminalIdentityIndex,
) -> Option<semantic_vocabulary::PlaceId> {
    let declaration = caller.structural_place(place)?;
    let semantic_vocabulary::StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return None;
    };
    let terminal_psi::StructuralTypeShape::Record {
        fields: record_fields,
        ..
    } = &find_structural_type(types, structural_type)?.shape
    else {
        return None;
    };
    let field = record_fields
        .iter()
        .find(|field| field.identity == field_identity)?
        .id;
    for node in caller.block_nodes() {
        let abstract_operations::AbstractOperation::EstablishRecord {
            psi_operation,
            result,
            fields,
            ..
        } = &node.operation
        else {
            continue;
        };
        if *psi_operation != producer || result.place != place {
            continue;
        }
        let initializer = fields.iter().find(|item| item.field == field)?;
        let terminal_psi::RecordFieldValue::Structural(argument) = &initializer.value else {
            return None;
        };
        return argument.path.is_empty().then_some(argument.place);
    }
    None
}

/// Rejoin one whole descriptor and the authority its current source minted.
fn view_argument(
    semantic: &StructuralArgument,
    call_operation: semantic_vocabulary::OperationId,
    caller: &PsiOptimizationFunction,
    destination_parameter: &terminal_psi::StructuralParameterDeclaration,
    call: &CallPlan,
    parameter_ordinal: usize,
    native: &TargetOperationPlan,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<TargetStructuralArgument, LegalizationError> {
    use target_operations::TargetStructuralArgumentSource;
    let completed =
        super::byte_views::completed_descriptor(caller, semantic.place, structural_types)
            .map(|(producer, result)| (producer, result.structural_type));
    // Descriptor storage ownership does not grant its referent's access.
    // Reconstruct the backing ceiling through the same source-chain query
    // used by ordinary byte and element operations.
    let access = optimization_unit_semantics::structural_place_backing_access(
        caller,
        semantic.place,
        structural_types.iter(),
    )
    .ok_or_else(LegalizationError::custody)?;
    // A descriptor view carries pointer and extent; a bounded view over a
    // fixed array (`&[u8; N]`) carries only the pointer, its extent static.
    let carrier = find_structural_type(structural_types, destination_parameter.structural_type)
        .map(|declaration| match declaration.shape {
            terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedView { .. },
            ) => ValueShape::borrowed_reference(8, 8),
            _ => ValueShape::borrowed_reference(16, 8),
        })
        .ok_or(LegalizationError::custody())?;
    if !matches!(
        semantic.access,
        terminal_psi::reading_borrow_pair_pattern!()
    ) || !semantic.path.is_empty()
        || destination_parameter.access != semantic.access
        || !terminal_semantics::view_descriptor_parameter(
            destination_parameter,
            structural_types.iter(),
        )
        || !(access == semantic.access
            || access == StructuralAccess::MutableBorrow
                && semantic.access == StructuralAccess::SharedBorrow)
    {
        return Err(LegalizationError::custody());
    }
    // Current-graph admission already replayed the exact argument's primitive
    // byte predicates. Reconstruct the original descriptor and placement here;
    // qualification rosters do not change that physical custody.
    let (structural_type, source) = if let Some((producer, structural_type)) = completed {
        (
            structural_type,
            TargetStructuralArgumentSource::StructuralHome {
                psi_operation: producer,
            },
        )
    } else if let Some((producer, structural_type)) =
        super::literals::producer(caller, call_operation, semantic.place)
    {
        // A literal has no structural operation-result declaration. Its
        // immutable descriptor retains the literal's own establishment.
        if !find_structural_type(structural_types, structural_type)
            .is_some_and(|declaration| declaration.shape.is_borrowed_byte_carrier())
        {
            return Err(LegalizationError::custody());
        }
        (
            structural_type,
            TargetStructuralArgumentSource::EstablishedByteView {
                psi_operation: producer,
            },
        )
    } else if let Some((block, parameter)) = caller.blocks.iter().find_map(|block| {
        block
            .structural_parameter(semantic.place)
            .map(|parameter| (block, parameter))
    }) {
        if block.id == caller.entry
            || terminal_semantics::descriptor_parameter_access(parameter, structural_types.iter())
                != Some(access)
        {
            return Err(LegalizationError::custody());
        }
        (
            parameter.structural_type,
            TargetStructuralArgumentSource::BlockParameter {
                block: block.id,
                place: parameter.place,
            },
        )
    } else {
        let source = caller
            .structural_parameter(semantic.place)
            .ok_or(LegalizationError::custody())?;
        let target_caller = native
            .function(caller.machine)
            .ok_or(LegalizationError::custody())?;
        let parameters =
            super::structural_parameters(target_caller).ok_or(LegalizationError::custody())?;
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.place == semantic.place)
            .ok_or(LegalizationError::custody())?;
        if semantic.place != source.place
            || terminal_semantics::descriptor_parameter_access(source, structural_types.iter())
                != Some(access)
            || !terminal_semantics::view_descriptor_projected_qualifications(
                structural_types.iter(),
                source.structural_type,
                &source.projected_qualifications,
            )
            || parameter.place != source.place
            || parameter.structural_type != source.structural_type
            || parameter.access != source.access
            || parameter.multiplicity != source.multiplicity
            || parameter.shape != carrier
            || !terminal_semantics::view_descriptor_projected_qualifications(
                structural_types.iter(),
                parameter.structural_type,
                &parameter.projected_qualifications,
            )
        {
            return Err(LegalizationError::custody());
        }
        (source.structural_type, parameter.placement.clone().into())
    };
    if terminal_semantics::borrowed_referent_presentation(
        structural_types.iter(),
        structural_type,
        destination_parameter.structural_type,
    ) != Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
        || call
            .parameters
            .get(parameter_ordinal)
            .is_none_or(|parameter| parameter.shape != carrier)
    {
        return Err(LegalizationError::custody());
    }
    Ok(TargetStructuralArgument {
        place: semantic.place,
        access: semantic.access,
        path: Vec::new(),
        root_structural_type: structural_type,
        structural_type: destination_parameter.structural_type,
        shape: carrier,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        indices: Vec::new(),
        source,
        destination: call
            .parameters
            .get(parameter_ordinal)
            .ok_or(LegalizationError::custody())?
            .clone(),
    })
}

/// Reconstruct a primitive borrow at its actual ordered call ABI position.
pub(super) fn primitive_argument(
    semantic: &StructuralArgument,
    call_operation: semantic_vocabulary::OperationId,
    caller: &PsiOptimizationFunction,
    destination_parameter: &terminal_psi::StructuralParameterDeclaration,
    call: &CallPlan,
    parameter_ordinal: usize,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
) -> Result<TargetStructuralArgument, LegalizationError> {
    use target_operations::TargetStructuralArgumentSource;
    if let Some((producer, result, value)) =
        super::primitive_locals::producer(caller, semantic.place)
    {
        if !super::primitive_locals::valid_result(caller, producer, result)
            || !semantic.path.is_empty()
            || semantic.access == StructuralAccess::Owned
            || destination_parameter.access != semantic.access
            || destination_parameter.structural_type != result.structural_type
            || destination_parameter.multiplicity
                != terminal_psi::StructuralMultiplicity::Unrestricted
            || !destination_parameter.qualifications.is_empty()
            || !destination_parameter.projected_qualifications.is_empty()
            || super::primitive_locals::scalar(&plan.structural_types, result.structural_type)
                != Some(value.scalar_type)
        {
            return Err(LegalizationError::custody());
        }
        let shape = crate::structural_inputs::structural_reference_input::parameter_shape(
            destination_parameter,
            &plan.structural_types,
        )
        .ok_or(LegalizationError::custody())?;
        let destination = call
            .parameters
            .get(parameter_ordinal)
            .ok_or(LegalizationError::custody())?
            .clone();
        if destination.shape != shape {
            return Err(LegalizationError::custody());
        }
        return Ok(TargetStructuralArgument {
            place: semantic.place,
            access: semantic.access,
            path: Vec::new(),
            root_structural_type: result.structural_type,
            structural_type: result.structural_type,
            shape,
            source_byte_offset: 0,
            fixed_array_length: None,
            element_stride: None,
            indices: Vec::new(),
            source: TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
                psi_operation: producer,
            },
            destination,
        });
    }
    exclusive::argument(
        semantic,
        call_operation,
        caller,
        destination_parameter,
        call,
        parameter_ordinal,
        native,
        plan,
    )
}
