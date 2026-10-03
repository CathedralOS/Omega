//! Exact semantic shape, value backing, and ABI custody for aggregates.
//!
//! A bounded byte owner returns its live length word and inline backing through
//! the aggregate ABI, not a borrowed pointer/length descriptor. Its primitive
//! predicate roster changes no layout, but must still match the declared normal
//! result exactly; equal byte widths cannot substitute semantic custody.
//!
//! `operation_result` joins produced storage to its exact declaration once.
//! Consumers add their ownership and layout checks; producer computation is
//! validated at its own instruction, not recognized again by each consumer.
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use semantic_vocabulary::PlaceId;
use terminal_psi::{StructuralAccess, StructuralTypeShape};

/// Entry captures register fragments in an owned local home, but retains an
/// address for inline-stack and indirect ABI value copies. Semantic ownership
/// does not choose between those physical homes. Consumers must make the same
/// distinction without requesting another copy just to observe the value.
pub(super) fn owned_parameter_uses_local_home(
    source: &LegalizedScalarFunction,
    declaration: &terminal_psi::StructuralParameterDeclaration,
) -> Option<bool> {
    let mut parameters = source
        .structural
        .as_ref()?
        .parameters
        .iter()
        .filter(|parameter| parameter.semantic.place == declaration.place);
    let parameter = parameters.next()?;
    if parameters.next().is_some()
        || &parameter.semantic != declaration
        || declaration.access != StructuralAccess::Owned
    {
        return None;
    }
    Some(
        !parameter.target.placement.locations.is_empty()
            && parameter.target.placement.locations.iter().all(|location| {
                matches!(
                    location,
                    calling_conventions::ValueLocation::Register { .. }
                )
            }),
    )
}

/// Owned aggregate inputs only need addressable storage when an operation observes a
/// field or byte contents, observes a projected case, lends the original value,
/// or a case dispatch reads its tag and payloads. State-edge transport copies
/// from an address into the successor's owned home, so even an otherwise
/// unobserved incoming value must be materialized for that edge. Direct call
/// and return transport can retain ABI fragments.
pub(super) fn parameter_home_required(source: &LegalizedScalarFunction, place: PlaceId) -> bool {
    let Some(signature) = &source.structural else {
        return false;
    };
    if !signature.parameters.iter().any(|parameter| {
        parameter.semantic.place == place
            && parameter.semantic.access == StructuralAccess::Owned
            && signature.structural_types.iter().any(|declaration| {
                declaration.id == parameter.semantic.structural_type
                    && matches!(
                        declaration.shape,
                        StructuralTypeShape::Reference { .. }
                            | StructuralTypeShape::Record { .. }
                            | StructuralTypeShape::Sum { .. }
                            | StructuralTypeShape::Mixed { .. }
                            | StructuralTypeShape::FixedArray { .. }
                            | StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                            )
                    )
            })
    }) {
        return false;
    }
    let binds = |successor: &legalized_operations::LegalizedScalarSuccessor| {
        successor
            .structural_bindings
            .iter()
            .any(|binding| binding.argument.place == place)
    };
    let forwarded = source.blocks.iter().any(|block| match &block.terminator {
        legalized_operations::LegalizedScalarTerminator::Jump { successor, .. } => binds(successor),
        legalized_operations::LegalizedScalarTerminator::Conditional {
            when_true,
            when_false,
            ..
        } => binds(when_true) || binds(when_false),
        _ => false,
    });
    let dispatched = source.blocks.iter().any(|block| {
        matches!(
            &block.terminator,
            legalized_operations::LegalizedScalarTerminator::StructuralCase {
                source: legalized_operations::LegalizedStructuralCaseSource::Parameter { declaration },
                ..
            } if declaration.place == place
        )
    });
    let cleanup_borrow = source.blocks.iter().any(|block| matches!(&block.terminator,
        legalized_operations::LegalizedScalarTerminator::ReturnCleanup { calls, .. }
            if calls.iter().any(|(_, call)| call.arguments.iter().any(|argument|
                matches!(argument, legalized_operations::LegalizedScalarArgument::Structural { semantic, .. }
                    if semantic.place == place && semantic.access != StructuralAccess::Owned)))));
    forwarded || dispatched || cleanup_borrow || source.blocks.iter().flat_map(|block| &block.instructions).any(|row| {
        match &row.kind {
            LegalizedScalarInstructionKind::StructuralScalarFieldRead { source, .. }
            | LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength { source, .. }
            | LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead { source, .. } => source.place == place,
            LegalizedScalarInstructionKind::StructuralCaseMembership { source, path, .. } => *source == place && !path.is_empty(),
            // Projected primitive reads need the same addressable owned home
            // as field observations, including arrays passed in ABI registers.
            LegalizedScalarInstructionKind::PrimitiveScalarRead { source, .. }
            | LegalizedScalarInstructionKind::StructuralLeafCopy { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceLength { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceRead { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceSubslice { source, .. } => *source == place,
            LegalizedScalarInstructionKind::EstablishReference { source, .. } => source.place == place,
            // Byte operations read the owner's length and backing, not a
            // pointer encoded in its first incoming register fragment.
            LegalizedScalarInstructionKind::ByteSequenceStore { destination, source, .. } => *destination == place || *source == place,
            LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore { destination, source, .. } => destination.place == place || *source == place,
            LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation { left, right, .. } => left.source() == place || right.source() == place,
            LegalizedScalarInstructionKind::Call(call) => call.arguments.iter().any(|argument| {
                matches!(argument, legalized_operations::LegalizedScalarArgument::Structural { semantic, .. }
                    if semantic.place == place && semantic.access != StructuralAccess::Owned)
            }),
            _ => false,
        }
    })
}

/// Reconstruct nominal tag identity from the retained readable root contract.
/// This lookup establishes shape and layout, not occurrence liveness: optimized
/// ownership/dominance replay and exact legalized-source replay remain required.
/// A path through a runtime element (`items[i]`) also yields each element's
/// selector, obligation, stride and extent, rebuilt from declarations.
#[allow(clippy::type_complexity)]
pub(super) fn membership_layout(
    source: &LegalizedScalarFunction,
    place: semantic_vocabulary::PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    case: semantic_vocabulary::StructuralCaseId,
) -> Option<(
    u32,
    u32,
    Vec<crate::structural_inputs::structural_reference_input::RuntimeElement>,
)> {
    let signature = source.structural.as_ref()?;
    let parameter = signature
        .parameters
        .iter()
        .map(|parameter| &parameter.semantic)
        .chain(
            source
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == place);
    let identity = if let Some(parameter) = parameter {
        if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
            || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || !signature.entry_claims.is_empty()
        {
            return None;
        }
        parameter.structural_type
    } else {
        let mut producers = source
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|row| {
                let result = match &row.kind {
                    LegalizedScalarInstructionKind::EstablishCase { result, .. }
                    | LegalizedScalarInstructionKind::EstablishRecord { result, .. }
                    | LegalizedScalarInstructionKind::EstablishScalarArray { result, .. }
                    | LegalizedScalarInstructionKind::HostedReadByte { result, .. } => result,
                    LegalizedScalarInstructionKind::Call(call) => {
                        call.structural_result.as_ref()?
                    }
                    _ => return None,
                };
                (result.place == place).then_some((row.operation, result))
            });
        let (operation, result) = producers.next()?;
        if producers.next().is_some()
            || result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            || !result.claims.is_empty()
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !signature.structural_places.iter().any(|declared| {
                declared.id == place
                    && declared.kind
                        == semantic_vocabulary::StructuralPlaceKind::OperationResult {
                            producer: operation,
                            structural_type: result.structural_type,
                        }
            })
        {
            return None;
        }
        result.structural_type
    };
    // Whole sums keep their existing payload-layout admission. Resolving a
    // nonempty path additionally requires the supported enclosing storage shape.
    let (identity, byte_offset, elements) = if path.is_empty() {
        (identity, 0, Vec::new())
    } else if terminal_psi::is_static_structural_path(path) {
        let (identity, byte_offset) =
            crate::structural_inputs::structural_reference_input::project(
                identity,
                path,
                &signature.structural_types,
            )?;
        (identity, byte_offset, Vec::new())
    } else {
        crate::structural_inputs::structural_reference_input::runtime_projection(
            identity,
            path,
            &signature.structural_types,
        )?
    };
    let declaration = signature
        .structural_types
        .iter()
        .find(|declared| declared.id == identity)?;
    // A mixed shape keeps the sum's tag word at offset 0; its runtime common
    // fields follow the tag, so they take part in the layout the tag guards.
    let (common, cases) = match &declaration.shape {
        terminal_psi::StructuralTypeShape::Sum { cases } => (&[][..], cases),
        terminal_psi::StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases),
        _ => return None,
    };
    let common = common
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .map(|field| {
            crate::structural_inputs::structural_reference_input::field_shape(
                &field.field_type,
                &signature.structural_types,
                &mut Vec::new(),
            )
        })
        .collect::<Option<Vec<_>>>()?;
    // Erased payload members hold no storage, so they shape no payload slot.
    let payloads = cases
        .iter()
        .map(|case| {
            case.fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .map(|field| {
                    crate::structural_inputs::structural_reference_input::field_shape(
                        &field.field_type,
                        &signature.structural_types,
                        &mut Vec::new(),
                    )
                })
                .collect::<Option<Vec<_>>>()
        })
        .collect::<Option<Vec<_>>>()?;
    let layout = calling_conventions::evaluate_conventional_sum_layout(&common, &payloads).ok()?;
    if layout.tag_byte_offset != 0
        || layout.tag_shape != calling_conventions::ValueShape::integer(4, 4)
    {
        return None;
    }
    cases
        .iter()
        .position(|candidate| candidate.id == case)
        .and_then(|ordinal| u32::try_from(ordinal).ok())
        .map(|tag| (tag, byte_offset, elements))
}

pub(super) fn call_result<'a>(
    source: &LegalizedScalarFunction,
    call: &'a legalized_operations::LegalizedScalarCall,
) -> Option<(
    &'a terminal_psi::StructuralOperationResult,
    &'a calling_conventions::ValuePlacement,
)> {
    let result = call.structural_result.as_ref()?;
    // The qualification roster is custody evidence carried on the result,
    // not part of the result home's physical shape; only linearity and
    // claims still reject.
    if result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !result.claims.is_empty()
    {
        return None;
    }
    let declaration = source
        .structural
        .as_ref()?
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)?;
    let shape = match &declaration.shape {
        terminal_psi::StructuralTypeShape::Record { .. }
        | terminal_psi::StructuralTypeShape::FixedArray { .. } => {
            crate::structural_inputs::structural_reference_input::primitive_array_shape(
                result.structural_type,
                &source.structural.as_ref()?.structural_types,
            )
            .or_else(|| {
                crate::structural_inputs::structural_reference_input::shape(
                    result.structural_type,
                    &source.structural.as_ref()?.structural_types,
                )
            })?
        }
        terminal_psi::StructuralTypeShape::Reference { .. }
        | terminal_psi::StructuralTypeShape::Sum { .. }
        | terminal_psi::StructuralTypeShape::Mixed { .. } => {
            crate::structural_inputs::structural_reference_input::shape(
                result.structural_type,
                &source.structural.as_ref()?.structural_types,
            )?
        }
        terminal_psi::StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
        ) => crate::structural_inputs::structural_reference_input::owned_aggregate_shape(
            result.structural_type,
            &source.structural.as_ref()?.structural_types,
        )?,
        _ if terminal_semantics::descriptor_results::is_shared_descriptor(
            source.structural.as_ref()?.structural_types.iter(),
            result.structural_type,
        ) && result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted =>
        {
            crate::structural_inputs::structural_reference_input::shape(
                result.structural_type,
                &source.structural.as_ref()?.structural_types,
            )?
        }
        _ => return None,
    };
    let placement = call.result_placement.as_ref()?;
    if call.call_plan.result.as_ref() != Some(placement)
        || placement.shape != shape
        || !(direct_fragments(placement)
            || indirect_result(placement, call.call_plan.policy).is_some())
    {
        return None;
    }
    Some((result, placement))
}

pub(super) fn returned_parameter<'a>(
    source: &'a LegalizedScalarFunction,
    value: &legalized_operations::LegalizedScalarReturnValue,
) -> Option<(
    &'a legalized_operations::LegalizedCallUnitParameter,
    &'a calling_conventions::ValuePlacement,
)> {
    let legalized_operations::LegalizedScalarReturnValue::StructuralParameter { place } = value
    else {
        return None;
    };
    let signature = source.structural.as_ref()?;
    let declared = signature.result.as_ref()?;
    let parameter = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == *place)?;
    let semantic = &parameter.semantic;
    let descriptor = semantic.access == terminal_psi::StructuralAccess::SharedBorrow
        && semantic.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        && terminal_semantics::descriptor_results::is_shared_descriptor(
            signature.structural_types.iter(),
            semantic.structural_type,
        );
    if semantic.access != terminal_psi::StructuralAccess::Owned && !descriptor
        || semantic.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || semantic.structural_type != declared.structural_type
        || semantic.multiplicity != declared.multiplicity
        || semantic.is_self
        || !semantic.qualifications.is_empty()
        || !semantic.projected_qualifications.is_empty()
        || !declared.qualifications.is_empty()
        || !declared.projected_qualifications.is_empty()
    {
        return None;
    }
    let placement = source.call_plan.result.as_ref()?;
    let shape = crate::structural_inputs::structural_reference_input::parameter_shape(
        semantic,
        &signature.structural_types,
    )?;
    let payload_shape = if descriptor {
        calling_conventions::ValueShape::integer(shape.byte_size, shape.alignment)
    } else {
        shape
    };
    let borrowed_pointer = descriptor
        && matches!(parameter.target.placement.locations.as_slice(),
        [calling_conventions::ValueLocation::Indirect {
            copy_stack_byte_offset: None, byte_size, alignment, ..
        }] if *byte_size == shape.byte_size && *alignment == shape.alignment);
    if parameter.target.shape != shape
        || parameter.target.placement.shape != shape
        || placement.shape != payload_shape
        || !(direct_fragments(placement)
            || indirect_result(placement, source.call_plan.policy).is_some())
        || !(borrowed_pointer
            || inline_argument_fragments(&parameter.target.placement)
            || crate::structural_inputs::structural_unit_input::owned_indirect_pointer(
                semantic,
                &parameter.target.placement,
            )
            .is_some())
    {
        return None;
    }
    Some((parameter, placement))
}

/// Inline arguments may occupy registers or a contiguous incoming stack extent.
/// Result admission remains separate: a stack argument does not admit a hidden result pointer.
pub(super) fn inline_argument_fragments(placement: &calling_conventions::ValuePlacement) -> bool {
    if !placement.shape.alignment.is_power_of_two() {
        return false;
    }
    if direct_fragments(placement) {
        return true;
    }
    if placement.shape.class != calling_conventions::ValueClass::Integer
        || placement.shape.byte_size == 0
    {
        return false;
    }
    let Some(calling_conventions::ValueLocation::Stack {
        stack_byte_offset: base,
        ..
    }) = placement.locations.first()
    else {
        return false;
    };
    if !base.is_multiple_of(u32::from(placement.shape.alignment)) {
        return false;
    }
    let mut offset = 0u32;
    for location in &placement.locations {
        let calling_conventions::ValueLocation::Stack {
            stack_byte_offset,
            value_byte_offset,
            byte_size,
            alignment,
        } = location
        else {
            return false;
        };
        if *value_byte_offset != offset
            || !(1..=8).contains(byte_size)
            || !alignment.is_power_of_two()
            || !stack_byte_offset.is_multiple_of(u32::from(*alignment))
            || base.checked_add(offset) != Some(*stack_byte_offset)
        {
            return false;
        }
        let Some(next) = offset.checked_add(*byte_size) else {
            return false;
        };
        offset = next;
    }
    offset == placement.shape.byte_size
}

/// Complete call-plan replay fixes register choice and disjoint stack offsets.
/// This classifier distinguishes an owned value copy from a borrowed pointer.
pub(super) fn owned_argument_placement(placement: &calling_conventions::ValuePlacement) -> bool {
    inline_argument_fragments(placement) || indirect_argument(placement).is_some()
}

pub(super) fn bounded_owned_byte_type(
    source: &LegalizedScalarFunction,
    structural_type: semantic_vocabulary::StructuralTypeId,
) -> bool {
    source.structural.as_ref().is_some_and(|signature| {
        signature.structural_types.iter().any(|declaration| {
            declaration.id == structural_type
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                )
        })
    })
}

pub(super) fn indirect_argument(
    placement: &calling_conventions::ValuePlacement,
) -> Option<(calling_conventions::IndirectPointerLocation, u32)> {
    let [
        calling_conventions::ValueLocation::Indirect {
            pointer,
            copy_stack_byte_offset: Some(copy_offset),
            byte_size,
            alignment,
        },
    ] = placement.locations.as_slice()
    else {
        return None;
    };
    if placement.shape.class != calling_conventions::ValueClass::Integer
        || *byte_size == 0
        || *byte_size != placement.shape.byte_size
        || *alignment != placement.shape.alignment
        || !alignment.is_power_of_two()
        || !copy_offset.is_multiple_of(u32::from(*alignment))
    {
        return None;
    }
    copy_offset.checked_add(*byte_size)?;
    Some((*pointer, *copy_offset))
}

pub(super) fn direct_fragments(placement: &calling_conventions::ValuePlacement) -> bool {
    if super::scalar_call_abi::empty_aggregate_placement(placement) {
        return true;
    }
    if placement.shape.class != calling_conventions::ValueClass::Integer
        || !(1..=2).contains(&placement.locations.len())
    {
        return false;
    }
    let mut offset = 0;
    for location in &placement.locations {
        let calling_conventions::ValueLocation::Register {
            value_byte_offset,
            byte_size,
            ..
        } = location
        else {
            return false;
        };
        if *value_byte_offset != offset || !(1..=8).contains(byte_size) {
            return false;
        }
        let Some(next) = offset.checked_add(*byte_size) else {
            return false;
        };
        offset = next;
    }
    offset == placement.shape.byte_size
}

pub(super) fn returned<'a>(
    source: &'a LegalizedScalarFunction,
    value: &legalized_operations::LegalizedScalarReturnValue,
) -> Option<(
    selected_instructions::LocalStorageSlotId,
    &'a calling_conventions::ValuePlacement,
)> {
    let legalized_operations::LegalizedScalarReturnValue::Structural { source: owner } = value
    else {
        return None;
    };
    let declared = source.structural.as_ref()?.result.as_ref()?;
    let (slot, shape, structural_type, multiplicity) = match owner {
        legalized_operations::LegalizedStructuralCaseSource::OperationResult {
            operation,
            result: returned,
        } => {
            // Return transport observes the exact completed home, regardless
            // of which operation produced its payload. Each instruction has
            // already replayed its computation; return custody rejoins the
            // producer, declaration and payload geometry rather than a second
            // list of accepted producer spellings.
            let (producer, result) = home(source, returned.place)?;
            if producer != *operation {
                return None;
            }
            let shape = crate::structural_inputs::structural_reference_input::shape(
                result.structural_type,
                &source.structural.as_ref()?.structural_types,
            )?;
            // Minted qualification rosters are custody evidence carried on
            // the result row; the verbatim `result != returned` check is the
            // honest comparison — ensurances are not part of home shape.
            if result != returned || !result.claims.is_empty() {
                return None;
            }
            (
                selected_instructions::LocalStorageSlotId::Structural {
                    operation: *operation,
                    place: result.place,
                },
                shape,
                result.structural_type,
                result.multiplicity,
            )
        }
        legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
            block,
            declaration,
        } => {
            let parameter = source
                .blocks
                .iter()
                .find(|candidate| candidate.id == *block)?
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == declaration.place)?;
            if parameter != declaration {
                return None;
            }
            (
                selected_instructions::LocalStorageSlotId::StructuralBlockParameter {
                    block: *block,
                    place: declaration.place,
                },
                block_parameter_shape(source, parameter)?,
                parameter.structural_type,
                parameter.multiplicity,
            )
        }
        // A returned parameter uses `StructuralParameter`, never this owner;
        // a borrowed dispatch source has no local custody slot at all.
        legalized_operations::LegalizedStructuralCaseSource::Parameter { .. }
        | legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter { .. } => {
            return None;
        }
    };
    let bounded_owned_bytes = matches!(
        owner,
        legalized_operations::LegalizedStructuralCaseSource::OperationResult { result, .. }
            if bounded_owned_byte_type(source, result.structural_type)
    );
    let exact_qualified_owned_bytes = matches!(
        owner,
        legalized_operations::LegalizedStructuralCaseSource::OperationResult { result, .. }
            if result.structural_type == declared.structural_type
                && result.multiplicity == declared.multiplicity
                && result.qualifications == declared.qualifications
                && result.projected_qualifications == declared.projected_qualifications
                && result.qualification_establishments.is_empty()
                && result.claims.is_empty()
                && bounded_owned_byte_type(source, result.structural_type)
    );
    if structural_type != declared.structural_type
        || multiplicity != declared.multiplicity
        || ((bounded_owned_bytes || !declared.qualifications.is_empty())
            && !exact_qualified_owned_bytes)
        || !declared.projected_qualifications.is_empty()
        || declared.multiplicity == terminal_psi::StructuralMultiplicity::Linear
    {
        return None;
    }
    let placement = source.call_plan.result.as_ref()?;
    if placement.shape != shape
        || !(direct_fragments(placement)
            || indirect_result(placement, source.call_plan.policy).is_some())
    {
        return None;
    }
    Some((slot, placement))
}

pub(super) fn fields(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
) -> Option<(
    usize,
    Vec<(
        u32,
        calling_conventions::ValueShape,
        Option<semantic_vocabulary::ScalarType>,
    )>,
)> {
    let LegalizedScalarInstructionKind::EstablishCase {
        result,
        result_case,
        fields,
        layout,
    } = &row.kind
    else {
        return None;
    };
    if row.result.is_some()
        || result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !result.claims.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
    {
        return None;
    }
    let signature = source.structural.as_ref()?;
    let mut declarations = signature
        .structural_types
        .iter()
        .filter(|declaration| declaration.id == result.structural_type);
    let declaration = declarations.next()?;
    if declarations.next().is_some() {
        return None;
    }
    // A mixed case stores its runtime common fields, then the selected
    // payload; the conventional layout places the common fields beside the
    // tag and every case payload after them.
    let (common, cases) = match &declaration.shape {
        terminal_psi::StructuralTypeShape::Sum { cases } => (&[][..], cases),
        terminal_psi::StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases),
        _ => return None,
    };
    let common = common
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .collect::<Vec<_>>();
    let common_shapes = common
        .iter()
        .map(|field| {
            crate::structural_inputs::structural_reference_input::field_shape(
                &field.field_type,
                &signature.structural_types,
                &mut Vec::new(),
            )
        })
        .collect::<Option<Vec<_>>>()?;
    let ordinal = cases.iter().position(|case| case.id == *result_case)?;
    if cases.iter().filter(|case| case.id == *result_case).count() != 1 {
        return None;
    }
    // Erased payload members hold no storage: the layout and the roster both
    // skip them, matching legalization's `sum_type_layout`.
    let payloads = cases
        .iter()
        .map(|case| {
            case.fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .map(|field| {
                    crate::structural_inputs::structural_reference_input::field_shape(
                        &field.field_type,
                        &signature.structural_types,
                        &mut Vec::new(),
                    )
                })
                .collect::<Option<Vec<_>>>()
        })
        .collect::<Option<Vec<_>>>()?;
    let payload = cases[ordinal]
        .fields
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .collect::<Vec<_>>();
    if calling_conventions::evaluate_conventional_sum_layout(&common_shapes, &payloads).ok()?
        != *layout
        || fields.len() != common.len() + payload.len()
    {
        return None;
    }
    fields
        .iter()
        .zip(common.iter().copied().chain(payload))
        .zip(
            layout
                .common_fields
                .iter()
                .chain(&layout.cases[ordinal].fields),
        )
        .map(|((field, declaration), placed)| {
            let shape = super::record_input::field_shape(source, result, field, declaration)?;
            (shape == placed.shape).then_some((
                placed.byte_offset,
                shape,
                declaration.field_type.scalar_type(),
            ))
        })
        .collect::<Option<Vec<_>>>()
        .map(|fields| (ordinal, fields))
}

/// Stored block arrivals use their declared value layout, independently of a call ABI.
pub(super) fn block_parameter_shape(
    source: &LegalizedScalarFunction,
    parameter: &terminal_psi::StructuralParameterDeclaration,
) -> Option<calling_conventions::ValueShape> {
    let signature = source.structural.as_ref()?;
    if matches!(
        parameter.access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) {
        let declaration = signature
            .structural_types
            .iter()
            .find(|declaration| declaration.id == parameter.structural_type)?;
        if parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            || !parameter.projected_qualifications.is_empty()
            || !(declaration.shape.is_borrowed_byte_carrier()
                || matches!(declaration.shape, StructuralTypeShape::ElementView { .. }))
        {
            return None;
        }
        // A stored view carries its descriptor, not a copy of its referent.
        // Capacity may live in the type (one pointer) or in the value (pointer
        // and length). Layout answers only storage; edge checking retains the
        // access, qualification and backing-custody obligations independently.
        return crate::structural_inputs::structural_reference_input::shape(
            parameter.structural_type,
            &signature.structural_types,
        );
    }
    let owned_bounded_bytes =
        abstract_operations::control_flow::owner_loans::is_owned_bounded_byte_parameter_in(
            parameter,
            &signature.structural_types,
        );
    if parameter.access != terminal_psi::StructuralAccess::Owned
        || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || (!owned_bounded_bytes && !parameter.qualifications.is_empty())
        || !parameter.projected_qualifications.is_empty()
    {
        return None;
    }
    // A successor carries the complete owned payload: nested aggregate fields,
    // or a bounded owner's live length word and inline byte backing. Predicate
    // qualifications change no geometry and remain in the semantic contract.
    // Storage transport preserves established contents; it does not grant
    // call-copy permission to nested descriptors or erased declarations.
    crate::structural_inputs::structural_reference_input::owned_storage_shape(
        parameter.structural_type,
        &signature.structural_types,
    )
}

/// Transfer a stored block value according to its declared representation.
/// Ordinary values and existing view descriptors are copied as complete stored
/// values. An inline backing-to-view presentation instead writes the backing
/// address into that descriptor; it never copies the referent's first word.
/// Construction and independent replay supply their own live storage maps.
pub(super) fn block_parameter_transport(
    source: &LegalizedScalarFunction,
    target: semantic_vocabulary::BlockId,
    parameter: &terminal_psi::StructuralParameterDeclaration,
    argument: &terminal_psi::StructuralArgument,
    pointers: &[(PlaceId, selected_instructions::VirtualRegisterId)],
    local_slots: &[selected_instructions::SelectedLocalStorageSlot],
) -> Option<selected_instructions::SelectedStructuralTransport> {
    use selected_instructions::{LocalStorageSlotId, SelectedStructuralTransport};
    use terminal_semantics::BorrowedReferentPresentation;
    if argument.access != parameter.access || !argument.path.is_empty() {
        return None;
    }
    let shape = block_parameter_shape(source, parameter)?;
    if shape.byte_size == 0 {
        return Some(SelectedStructuralTransport::Unused);
    }
    let destination = LocalStorageSlotId::StructuralBlockParameter {
        block: target,
        place: parameter.place,
    };
    let presentation = if parameter.access == StructuralAccess::Owned {
        BorrowedReferentPresentation::Unchanged
    } else {
        let actual = super::established_view_input::view_type(source, argument.place)
            .or_else(|| super::established_view_input::element_view_type(source, argument.place))
            .or_else(|| super::address_join_input::root_type(source, argument.place, argument.access))?;
        terminal_semantics::borrowed_referent_presentation(
            source.structural.as_ref()?.structural_types.iter(),
            actual,
            parameter.structural_type,
        )?
    };
    match presentation {
        BorrowedReferentPresentation::Unchanged => {
            let pointer = pointers.iter().find(|(place, _)| *place == argument.place)?.1;
            Some(SelectedStructuralTransport::WholeValue {
                argument: pointer,
                destination,
                byte_size: shape.byte_size,
                alignment: shape.alignment,
            })
        }
        BorrowedReferentPresentation::AddressOfBacking { byte_count } => {
            let byte_count = u32::try_from(byte_count).ok()?;
            let base = super::address_join_input::storage_base(
                argument.place, 0, byte_count, pointers, local_slots,
            )?;
            Some(SelectedStructuralTransport::Address {
                base,
                byte_offset: 0,
                byte_count,
                destination,
            })
        }
    }
}

/// The ABI planner, rather than an arbitrary pointer register, owns the hidden destination.
pub(super) fn indirect_result(
    placement: &calling_conventions::ValuePlacement,
    policy: calling_conventions::CallingPolicy,
) -> Option<calling_conventions::MachineRegister> {
    use calling_conventions::{
        CallSignature, IndirectPointerLocation, ValueLocation, evaluate_call_plan,
    };
    if placement.shape.class != calling_conventions::ValueClass::Integer
        || placement.shape.byte_size == 0
        || !placement.shape.alignment.is_power_of_two()
    {
        return None;
    }
    let [
        ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(register),
            copy_stack_byte_offset: None,
            byte_size,
            alignment,
        },
    ] = placement.locations.as_slice()
    else {
        return None;
    };
    if *byte_size != placement.shape.byte_size || *alignment != placement.shape.alignment {
        return None;
    }
    let canonical = evaluate_call_plan(
        policy,
        &CallSignature {
            parameters: Vec::new(),
            result: Some(placement.shape),
        },
    )
    .ok()?;
    (canonical.result.as_ref() == Some(placement)).then_some(*register)
}

/// Join one produced value to its unique instruction and exact place catalog
/// entry. The operation validates its own computation during construction and
/// replay; storage consumers must not recognize that computation again. This
/// query grants no ownership, qualification, projection or layout authority.
pub(super) fn operation_result(
    source: &LegalizedScalarFunction,
    place: semantic_vocabulary::PlaceId,
) -> Option<(
    semantic_vocabulary::OperationId,
    &terminal_psi::StructuralOperationResult,
)> {
    let mut selected = source
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|row| {
            let result = row.kind.get_structural_result()?;
            (result.place == place).then_some((row, result))
        });
    let (row, result) = selected.next()?;
    let operation = row.operation;
    let mut declarations = source
        .structural
        .as_ref()?
        .structural_places
        .iter()
        .filter(|declaration| declaration.id == place);
    let declaration = declarations.next()?;
    if selected.next().is_some()
        || declarations.next().is_some()
        || declaration.kind
            != (semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation,
                structural_type: result.structural_type,
            })
    {
        return None;
    }
    Some((operation, result))
}

/// Locate a nominal aggregate's original operation-result home. This is a
/// declaration/layout rejoin; source replay separately establishes availability.
/// A copied aggregate owns the same readable home as a constructed value;
/// how its bytes were initialized does not change primitive-read geometry.
pub(super) fn home(
    source: &LegalizedScalarFunction,
    place: semantic_vocabulary::PlaceId,
) -> Option<(
    semantic_vocabulary::OperationId,
    &terminal_psi::StructuralOperationResult,
)> {
    let (operation, result) = operation_result(source, place)?;
    if result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !result.claims.is_empty()
        // Primitive predicates do not change a bounded owner's `{length,
        // bytes}` home. Producer replay already authenticated the exact result
        // roster; this storage query must not reclassify that roster by which
        // operation happened to produce it. Other aggregate carriers retain
        // their established unqualified-home boundary.
        || (!result.qualifications.is_empty()
            && (!bounded_owned_byte_type(source, result.structural_type)
                || !result.qualification_establishments.is_empty()))
        || !result.projected_qualifications.is_empty()
        || !source
            .structural
            .as_ref()?
            .structural_types
            .iter()
            .any(|declaration| {
                declaration.id == result.structural_type
                    && (terminal_semantics::descriptor_results::is_shared_descriptor(
                        std::iter::once(declaration), declaration.id,
                    ) || matches!(
                        declaration.shape,
                        StructuralTypeShape::Reference { .. }
                            | StructuralTypeShape::Record { .. }
                            | StructuralTypeShape::Sum { .. }
                            | StructuralTypeShape::FixedArray { .. }
                            | StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                            )
                    ))
            })
    {
        return None;
    }
    let signature = source.structural.as_ref()?;
    if signature.structural_types.iter().any(|declaration| {
        declaration.id == result.structural_type
            && matches!(declaration.shape, StructuralTypeShape::Reference { .. })
    }) && (result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
        || !result.qualification_establishments.is_empty())
    {
        return None;
    }
    crate::structural_inputs::structural_reference_input::shape(
        result.structural_type,
        &signature.structural_types,
    )?;
    Some((operation, result))
}

/// A selected aggregate lives in the join's own home. Reconstruct its declaration
/// and exact place coordinate; source replay separately checks availability.
pub(super) fn block_home(
    source: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<(
    semantic_vocabulary::BlockId,
    &terminal_psi::StructuralParameterDeclaration,
)> {
    let mut matching = source.blocks.iter().flat_map(|block| {
        block
            .structural_parameters
            .iter()
            .filter(move |parameter| parameter.place == place)
            .map(move |parameter| (block.id, parameter))
    });
    let (block, parameter) = matching.next()?;
    let signature = source.structural.as_ref()?;
    let owned_bounded_bytes =
        abstract_operations::control_flow::owner_loans::is_owned_bounded_byte_parameter_in(
            parameter,
            &signature.structural_types,
        );
    if matching.next().is_some()
        || parameter.access != StructuralAccess::Owned
        || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || (!owned_bounded_bytes && !parameter.qualifications.is_empty())
        || !parameter.projected_qualifications.is_empty()
        || !signature.structural_places.iter().any(|declaration| {
            declaration.id == place
                && declaration.kind
                    == semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                        block,
                        position: parameter.position,
                    }
        })
    {
        return None;
    }
    block_parameter_shape(source, parameter)?;
    Some((block, parameter))
}

/// Rejoin a store's owned destination to its exact producing row and declared
/// place. This establishes identity only; current ownership and dominance are
/// independently checked on the optimized source before legalization.
pub(super) fn owned_store_destination(
    source: &LegalizedScalarFunction,
    producer: semantic_vocabulary::OperationId,
    expected: &terminal_psi::StructuralOperationResult,
) -> bool {
    operation_result(source, expected.place) == Some((producer, expected))
}
