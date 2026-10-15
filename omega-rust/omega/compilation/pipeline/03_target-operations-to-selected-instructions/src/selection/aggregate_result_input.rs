//! Exact semantic shape, value backing, and ABI custody for aggregates.
//!
//! A bounded byte owner returns its live length word and inline backing through
//! the aggregate ABI, not a borrowed pointer/length descriptor. Its primitive
//! predicate roster changes no layout. Psi proves the normal result's domains;
//! this stage retains exact source custody and the declared result's ABI.
//! Matching widths alone cannot substitute its type, owner or producer.
//!
//! `operation_result` joins produced storage to its exact declaration once.
//! Consumers add their ownership and layout checks; producer computation is
//! validated at its own instruction, not recognized again by each consumer.
//! A home lookup preserves the result's domain metadata without interpreting
//! it as a storage restriction: an authorized boundary result occupies the
//! same home as any other value of its declared representation.
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use semantic_vocabulary::PlaceId;
use terminal_psi::{StructuralAccess, StructuralTypeShape, structural_case_position};

/// Entry materializes every observed owned arrival in a private value home,
/// regardless of its ABI backing. Consumers use the same demand as entry;
/// neither register placement nor an incoming pointer determines ownership.
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
        parameter.target.shape.byte_size != 0 && parameter_home_required(source, declaration.place),
    )
}

/// Owned aggregate inputs only need addressable storage when an operation observes a
/// field or byte contents, writes a field, observes a projected case, lends the original value,
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
                            | terminal_psi::sum_or_mixed_shape_pattern!()
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
    let cleanup_borrow = source.blocks.iter().any(|block| block.terminator.cleanup_calls().any(|(_, call)|
        call.arguments.iter().any(|argument|
            matches!(argument, legalized_operations::LegalizedScalarArgument::Structural { semantic, target, .. }
                if semantic.place == place && argument_transport_requires_home(target)))));
    forwarded || dispatched || cleanup_borrow || source.instructions().any(|row| {
        match &row.kind {
            LegalizedScalarInstructionKind::StructuralScalarFieldRead { source, .. }
            | LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength { source, .. }
            | LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead { source, .. }
            | LegalizedScalarInstructionKind::ElementViewLength { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceLength { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceSubslice { source, .. }
            | LegalizedScalarInstructionKind::ElementViewSubslice { source, .. } => source.place == place,
            LegalizedScalarInstructionKind::StructuralCaseMembership { source, path, .. } => *source == place && !path.is_empty(),
            // Projected primitive reads need the same addressable owned home
            // as field observations, including arrays passed in ABI registers.
            LegalizedScalarInstructionKind::PrimitiveScalarRead { source, .. }
            | LegalizedScalarInstructionKind::StructuralLeafCopy { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceRead { source, .. } => *source == place,
            LegalizedScalarInstructionKind::EstablishStructuralArray { elements, .. } => elements.iter().any(|argument|
                argument.place == place && !argument.path.is_empty()),
            LegalizedScalarInstructionKind::EstablishRecord { fields, .. }
            | LegalizedScalarInstructionKind::EstablishCase { fields, .. } => fields.iter().any(|field|
                matches!(&field.value, terminal_psi::RecordFieldValue::Structural(argument)
                    if argument.place == place && !argument.path.is_empty())),
            LegalizedScalarInstructionKind::EstablishReference { source, .. }
            | LegalizedScalarInstructionKind::EstablishPlacedReference { source, .. }
            // A collection view lends the owner's backing too. Its incoming
            // ABI fragments need a home even when no scalar field read or
            // later call independently happens to demand one.
            | LegalizedScalarInstructionKind::EstablishElementView { source, .. } => source.place == place,
            LegalizedScalarInstructionKind::AtomicAccess { location, .. }
                | LegalizedScalarInstructionKind::PlacedScalarAccess { location, .. } => location.place == place,
            // Byte operations read the owner's length and backing, not a
            // pointer encoded in its first incoming register fragment.
            LegalizedScalarInstructionKind::ByteSequenceStore { destination, source, .. } => *destination == place || *source == place,
            LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore { destination, source, .. } => destination.place == place || *source == place,
            // Reseating register-carried owned storage requires an addressable
            // home even when no separate read or projection exposed it first.
            LegalizedScalarInstructionKind::StoreStructuralField { destination, .. } => destination.place() == place,
            // A scalar store also writes the current owned value, so even a
            // store-only entry needs the same home as a field observation.
            LegalizedScalarInstructionKind::StructuralScalarFieldStore { destination, .. }
            | LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { destination, .. } => destination.place() == place,
            LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation { left, right, .. } => left.source() == place || right.source() == place,
            LegalizedScalarInstructionKind::Call(call) => call.arguments.iter().any(|argument| {
                matches!(argument, legalized_operations::LegalizedScalarArgument::Structural { semantic, .. }
                    if semantic.place == place && semantic.access != StructuralAccess::Owned)
            }),
            LegalizedScalarInstructionKind::NormalizedForeignCall(call) => call
                .structural_arguments
                .iter()
                .any(|argument| argument.place == place && argument_transport_requires_home(argument)),
            _ => false,
        }
    })
}

/// A register fragment can be forwarded as bytes only when the outgoing
/// read starts at that fragment and fits within it. A different grouping or
/// projection needs an addressable source; this depends on transport geometry,
/// not on the internal/foreign call mechanism.
fn argument_transport_requires_home(
    argument: &target_operations::TargetStructuralArgument,
) -> bool {
    if argument.access != StructuralAccess::Owned {
        return true;
    }
    let target_operations::TargetStructuralArgumentSource::Placement(source) = &argument.source
    else {
        return false;
    };
    if !source.locations.iter().all(|location| {
        matches!(
            location,
            calling_conventions::ValueLocation::Register { .. }
        )
    }) {
        return false;
    }
    let can_forward =
        |offset: u32, size: u32| {
            let Some(offset) = argument.source_byte_offset.checked_add(offset) else {
                return false;
            };
            source.locations.iter().any(|location| matches!(
            location,
            calling_conventions::ValueLocation::Register { value_byte_offset, byte_size, .. }
                if *value_byte_offset == offset && *byte_size >= size
        ))
        };
    if indirect_argument(&argument.destination).is_some() {
        return (0..argument.shape.byte_size)
            .step_by(8)
            .any(|offset| !can_forward(offset, (argument.shape.byte_size - offset).min(8)));
    }
    argument
        .destination
        .locations
        .iter()
        .any(|location| match location {
            calling_conventions::ValueLocation::Register {
                value_byte_offset,
                byte_size,
                ..
            }
            | calling_conventions::ValueLocation::Stack {
                value_byte_offset,
                byte_size,
                ..
            } => !can_forward(*value_byte_offset, *byte_size),
            _ => true,
        })
}

/// Reconstruct nominal tag identity from the retained readable root contract.
/// This lookup establishes shape and layout, not occurrence liveness. A tag
/// observation neither copies nor consumes payload claims; their multiplicity
/// and qualifications do not change its storage geometry. Source custody and
/// readable access remain separate from this declaration/layout join.
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
    crate::structural_inputs::structural_reference_input::ReferenceSourceGeometry,
)> {
    let signature = source.structural.as_ref()?;
    let parameter = signature
        .parameters
        .iter()
        .map(|parameter| &parameter.semantic)
        .chain(source.block_structural_parameters())
        .find(|parameter| parameter.place == place);
    let identity = if let Some(parameter) = parameter {
        if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
            return None;
        }
        parameter.structural_type
    } else {
        // Observation consumes the established result contract, not a list of
        // ways its storage might have been initialized. The common lookup
        // authenticates its unique producer and exact place declaration.
        let (_, result) = operation_result(source, place)?;
        result.structural_type
    };
    let geometry = crate::structural_inputs::structural_reference_input::reference_source_geometry(
        identity,
        path,
        &signature.structural_types,
    )?;
    if !geometry.readable
        || geometry
            .pointer_slots
            .iter()
            .any(|slot| slot.scalar_descriptor)
    {
        return None;
    }
    let byte_offset = geometry.byte_offset;
    let declaration = signature.structural_type(geometry.selected)?;
    // A mixed shape keeps the sum's tag word at offset 0; its runtime common
    // fields follow the tag, so they take part in the layout the tag guards.
    let (common, cases) = match &declaration.shape {
        terminal_psi::StructuralTypeShape::Sum { cases } => (&[][..], cases),
        terminal_psi::StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases),
        _ => return None,
    };
    let common = live_field_shapes(common.iter(), &signature.structural_types)?;
    // Erased payload members hold no storage, so they shape no payload slot.
    let payloads = cases
        .iter()
        .map(|case| live_field_shapes(case.fields.iter(), &signature.structural_types))
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
        .map(|tag| (tag, byte_offset, geometry))
}

/// Physical completion storage is shared by internal and foreign calls. Their
/// signature validators establish different ABI classes; this query preserves
/// each validated placement and the exact produced place instead of converting
/// a foreign aggregate into a scalar merely because it fits one register.
pub(super) fn call_completion<'a>(
    source: &LegalizedScalarFunction,
    instruction: &'a LegalizedScalarInstruction,
) -> Option<(
    &'a terminal_psi::StructuralOperationResult,
    &'a calling_conventions::ValuePlacement,
    calling_conventions::CallingPolicy,
)> {
    match &instruction.kind {
        LegalizedScalarInstructionKind::Call(call) => {
            let (result, placement) = call_result(source, call)?;
            Some((result, placement, call.call_plan.policy))
        }
        LegalizedScalarInstructionKind::NormalizedForeignCall(call) => {
            let target_operations::TargetBoundaryResult::Structural(home) = &call.result else {
                return None;
            };
            let (producer, result) = home.operation_result()?;
            if producer != instruction.operation
                || operation_result(source, result.place) != Some((producer, result))
            {
                return None;
            }
            Some((
                result,
                call.binding.boundary_entry_plan.call.result.as_ref()?,
                call.binding.boundary_entry_plan.call.policy,
            ))
        }
        _ => None,
    }
}

pub(super) fn call_result<'a>(
    source: &LegalizedScalarFunction,
    call: &'a legalized_operations::LegalizedScalarCall,
) -> Option<(
    &'a terminal_psi::StructuralOperationResult,
    &'a calling_conventions::ValuePlacement,
)> {
    let result = call.structural_result.as_ref()?;
    // Qualifications, multiplicity and claims are custody evidence carried
    // on the result, not part of the result home's physical shape; the
    // optimization unit's ownership replay already authenticated them.
    let declaration = terminal_psi::find_structural_type(
        &source.structural.as_ref()?.structural_types,
        result.structural_type,
    )?;
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
        ) => crate::structural_inputs::structural_reference_input::owned_storage_shape(
            result.structural_type,
            &source.structural.as_ref()?.structural_types,
        )?,
        _ if terminal_semantics::descriptor_results::is_descriptor(
            source.structural.as_ref()?.structural_types.iter(),
            result.structural_type,
        ) && matches!(
            result.multiplicity,
            terminal_psi::nonlinear_multiplicity_pair_pattern!()
        ) =>
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

/// Descriptor values use the same return transport regardless of where the
/// descriptor was established. A local slot stores pointer/length only; its
/// existence never authorizes the backing to escape.
pub(super) fn returned_descriptor<'a>(
    source: &'a LegalizedScalarFunction,
    value: &legalized_operations::LegalizedScalarReturnValue,
) -> Option<(
    semantic_vocabulary::PlaceId,
    &'a calling_conventions::ValuePlacement,
    Option<selected_instructions::LocalStorageSlotId>,
)> {
    let legalized_operations::LegalizedScalarReturnValue::SharedDescriptor { place } = value else {
        return None;
    };
    let signature = source.structural.as_ref()?;
    let declared = signature.result.as_ref()?;
    let identity = super::established_view_input::view_type(source, *place)
        .or_else(|| super::established_view_input::element_view_type(source, *place))?;
    let placement = source.call_plan.result.as_ref()?;
    if identity != declared.structural_type
        || !matches!(
            declared.multiplicity,
            terminal_psi::nonlinear_multiplicity_pair_pattern!()
        )
        || !declared.projected_qualifications.is_empty()
        || !terminal_semantics::descriptor_results::is_descriptor(
            signature.structural_types.iter(),
            identity,
        )
        || placement.shape != calling_conventions::ValueShape::integer(16, 8)
        || !(direct_fragments(placement)
            || indirect_result(placement, source.call_plan.policy).is_some())
    {
        return None;
    }
    if signature
        .parameters
        .iter()
        .any(|parameter| parameter.semantic.place == *place)
    {
        return Some((*place, placement, None));
    }
    for block in &source.blocks {
        if block.structural_parameter(*place).is_some() {
            return Some((
                *place,
                placement,
                Some(
                    selected_instructions::LocalStorageSlotId::StructuralBlockParameter {
                        block: block.id,
                        place: *place,
                    },
                ),
            ));
        }
        for row in &block.instructions {
            if row
                .kind
                .get_structural_result()
                .is_some_and(|result| result.place == *place)
                || matches!(&row.kind,
                    LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
                        destination, ..
                    } if destination.id == *place)
            {
                return Some((
                    *place,
                    placement,
                    Some(selected_instructions::LocalStorageSlotId::Structural {
                        operation: row.operation,
                        place: *place,
                    }),
                ));
            }
        }
    }
    None
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
    if semantic.access != terminal_psi::StructuralAccess::Owned
        || semantic.structural_type != declared.structural_type
        || semantic.multiplicity != declared.multiplicity
    {
        return None;
    }
    let placement = source.call_plan.result.as_ref()?;
    let shape = crate::structural_inputs::structural_reference_input::parameter_shape(
        semantic,
        &signature.structural_types,
    )?;
    if parameter.target.shape != shape
        || parameter.target.placement.shape != shape
        || placement.shape != shape
        || !(direct_fragments(placement)
            || indirect_result(placement, source.call_plan.policy).is_some())
        || !(inline_argument_fragments(&parameter.target.placement)
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
/// The ABI's stack alignment is distinct from the owned value's home alignment.
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
            || *byte_size == 0
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
    if indirect_argument(placement).is_some()
        || super::scalar_call_abi::empty_aggregate_placement(placement)
    {
        return true;
    }
    if !placement.shape.alignment.is_power_of_two()
        || matches!(
            placement.shape.class,
            calling_conventions::ValueClass::BorrowedReference
                | calling_conventions::ValueClass::BorrowedScalarReference
        )
        || placement.shape.byte_size == 0
    {
        return false;
    }
    // The validated plan owns bank choice. Transport requires only complete
    // nonoverlapping payload fragments that the exact-width access primitives
    // can move; neither a register count nor an aggregate class selects a lane.
    let mut ranges = Vec::with_capacity(placement.locations.len());
    for location in &placement.locations {
        let (offset, byte_size) = match location {
            calling_conventions::ValueLocation::Register {
                value_byte_offset,
                byte_size,
                ..
            } if (1..=8).contains(byte_size) => (*value_byte_offset, *byte_size),
            calling_conventions::ValueLocation::Stack {
                value_byte_offset,
                byte_size,
                stack_byte_offset,
                alignment,
            } if alignment.is_power_of_two()
                && stack_byte_offset.is_multiple_of(u32::from(*alignment))
                && stack_byte_offset.checked_add(*byte_size).is_some() =>
            {
                (*value_byte_offset, *byte_size)
            }
            _ => return false,
        };
        if byte_size == 0 {
            return false;
        }
        ranges.push((offset, byte_size));
    }
    ranges.sort_unstable_by_key(|(offset, _)| *offset);
    let mut end = 0_u32;
    for (offset, byte_size) in ranges {
        if offset != end {
            return false;
        }
        let Some(next) = end.checked_add(byte_size) else {
            return false;
        };
        end = next;
    }
    end == placement.shape.byte_size
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
    if matches!(
        placement.shape.class,
        calling_conventions::ValueClass::BorrowedReference
            | calling_conventions::ValueClass::BorrowedScalarReference
    ) || *byte_size == 0
        || *byte_size != placement.shape.byte_size
        || *alignment != placement.shape.alignment
        || !alignment.is_power_of_two()
        // The copy offset starts the reserved home. Absolute value alignment
        // is formed inside its padding; the fixed ABI SP remains 16-aligned.
        || !copy_offset.is_multiple_of(u32::from((*alignment).clamp(8, 16)))
    {
        return None;
    }
    copy_offset
        .checked_add(*byte_size)?
        .checked_add(u32::from(alignment.saturating_sub(16)))?;
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
            // Minted qualification rosters and claims are custody evidence
            // carried on the result row; the verbatim `result != returned`
            // check is the honest comparison — neither is part of home shape.
            if result != returned {
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
                .block(*block)?
                .structural_parameter(declaration.place)?;
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
    if structural_type != declared.structural_type || multiplicity != declared.multiplicity {
        return None;
    }
    // Psi owns result membership. Qualification labels add no bytes to the
    // exact source home or the result ABI; they need not have identical rosters.
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
    // A constructor's claims describe its transferred obligations, not its
    // storage layout. Projected memberships likewise add no bytes. As for
    // record fields, rejoin each owned operand and the
    // selected payload's declared geometry without granting fresh custody.
    if row.result.is_some() || !result.qualifications.is_empty() {
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
    let common_shapes = live_field_shapes(common.iter().copied(), &signature.structural_types)?;
    let ordinal = structural_case_position(cases, *result_case)?;
    if cases.iter().filter(|case| case.id == *result_case).count() != 1 {
        return None;
    }
    // Erased payload members hold no storage: the layout and the roster both
    // skip them, matching legalization's `sum_type_layout`.
    let payloads = cases
        .iter()
        .map(|case| live_field_shapes(case.fields.iter(), &signature.structural_types))
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
        terminal_psi::reading_borrow_pair_pattern!()
    ) {
        if !terminal_semantics::view_descriptor_parameter(
            parameter,
            signature.structural_types.iter(),
        ) {
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
    if parameter.access != terminal_psi::StructuralAccess::Owned {
        return None;
    }
    // A successor carries the complete owned payload, including a transferred
    // reference descriptor or an aggregate's nested fields. Predicate
    // qualifications change no geometry and remain in the semantic contract.
    // Storage transport preserves established contents; it does not grant
    // call-copy permission to nested descriptors or erased declarations.
    crate::structural_inputs::structural_reference_input::stored_value_shape(
        parameter.structural_type,
        &signature.structural_types,
    )
}

/// Transfer a stored block value according to its declared representation.
/// Ordinary values and existing view descriptors are copied as complete stored
/// values. An inline backing-to-view presentation instead writes the backing
/// address into that descriptor; it never copies the referent's first word.
/// Construction and independent replay supply their own live storage maps.
pub(crate) struct BlockParameterProjection {
    pub(crate) transport: selected_instructions::SelectedStructuralTransport,
    /// Ordered pointer loads before the selected storage. The edge snapshots
    /// the final descriptor or address before replacing any destination home.
    pub(crate) descriptor_offsets: Vec<u32>,
}

pub(super) fn block_parameter_transport(
    source: &LegalizedScalarFunction,
    target: semantic_vocabulary::BlockId,
    parameter: &terminal_psi::StructuralParameterDeclaration,
    argument: &terminal_psi::StructuralArgument,
    pointers: &[(PlaceId, selected_instructions::VirtualRegisterId)],
    local_slots: &[selected_instructions::SelectedLocalStorageSlot],
) -> Option<BlockParameterProjection> {
    use selected_instructions::{LocalStorageSlotId, SelectedStructuralTransport};
    use terminal_semantics::BorrowedReferentPresentation;
    if argument.access != parameter.access
        || !(if argument.access == StructuralAccess::Owned {
            abstract_operations::control_flow::address_joins::is_static_projection(&argument.path)
        } else {
            abstract_operations::control_flow::address_joins::is_address_projection(&argument.path)
        })
    {
        return None;
    }
    let mut descriptor_offsets = Vec::new();
    let mut presented_type = None;
    let source_byte_offset = if argument.access == StructuralAccess::Owned {
        let declarations = &source.structural.as_ref()?.structural_types;
        let root = super::address_join_input::root_type(
            source,
            argument.place,
            argument.access,
            &argument.path,
        )?;
        let (selected, offset) =
            crate::structural_inputs::structural_reference_input::leaf_copy_projection(
                root,
                &argument.path,
                declarations,
            )?;
        if selected != parameter.structural_type {
            return None;
        }
        let root_shape = crate::structural_inputs::structural_reference_input::stored_value_shape(
            root,
            declarations,
        )?;
        let selected_shape = block_parameter_shape(source, parameter)?;
        if offset.checked_add(selected_shape.byte_size)? > root_shape.byte_size
            || offset % u32::from(selected_shape.alignment) != 0
        {
            return None;
        }
        offset
    } else {
        let declarations = &source.structural.as_ref()?.structural_types;
        let root = super::established_view_input::view_type(source, argument.place)
            .or_else(|| super::established_view_input::element_view_type(source, argument.place))
            .or_else(|| super::reference_input::root_type(source, argument.place))?;
        let geometry =
            crate::structural_inputs::structural_reference_input::descriptor_source_geometry(
                root,
                &argument.path,
                declarations,
            )?;
        if !geometry.readable
            || !geometry.elements.is_empty()
            || geometry
                .pointer_slots
                .iter()
                .any(|slot| !slot.indices.is_empty())
        {
            return None;
        }
        presented_type = Some(geometry.selected);
        descriptor_offsets.extend(geometry.pointer_slots.iter().map(|slot| slot.byte_offset));
        geometry.byte_offset
    };
    let shape = block_parameter_shape(source, parameter)?;
    if shape.byte_size == 0 {
        return Some(BlockParameterProjection {
            transport: SelectedStructuralTransport::Unused,
            descriptor_offsets,
        });
    }
    let destination = LocalStorageSlotId::StructuralBlockParameter {
        block: target,
        place: parameter.place,
    };
    let presentation = if parameter.access == StructuralAccess::Owned {
        BorrowedReferentPresentation::Unchanged
    } else {
        let actual = presented_type?;
        terminal_semantics::borrowed_referent_presentation(
            source.structural.as_ref()?.structural_types.iter(),
            actual,
            parameter.structural_type,
        )?
    };
    let transport = match presentation {
        BorrowedReferentPresentation::Unchanged => {
            let pointer = pointers
                .iter()
                .find(|(place, _)| *place == argument.place)?
                .1;
            SelectedStructuralTransport::WholeValue {
                argument: pointer,
                source_byte_offset,
                destination,
                byte_size: shape.byte_size,
                alignment: shape.alignment,
            }
        }
        BorrowedReferentPresentation::AddressOfBacking { byte_count } => {
            let byte_count = u32::try_from(byte_count).ok()?;
            let base = super::address_join_input::storage_base(
                argument.place,
                0,
                byte_count,
                pointers,
                local_slots,
            )?;
            SelectedStructuralTransport::Address {
                base,
                byte_offset: 0,
                byte_count,
                destination,
            }
        }
    };
    Some(BlockParameterProjection {
        transport,
        descriptor_offsets,
    })
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
    let mut selected = source.instructions().filter_map(|row| {
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
    // Qualifications and their establishment routes describe the value's
    // checked meaning, not a different storage home. In particular a boundary
    // call can establish a domain on an ordinary record. Keep that metadata on
    // the exact result instead of making storage depend on whether its producer
    // establishes a predicate, a routed domain, or neither. Claims likewise
    // remain on the result; finding its home grants no permission to consume it.
    let signature = source.structural.as_ref()?;
    // Shared descriptor copies may be unrestricted; exclusive descriptors
    // remain affine. Their checked multiplicity changes custody, not the
    // addressable home holding the stored pointer.
    // Layout admission, not a second nominal-kind roster, decides whether
    // this completed value has a native home. In particular a mixed value's
    // common fields, discriminator and overlay belong to one ordinary home.
    // The producer/declaration and permission checks above remain independent.
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
    if matching.next().is_some()
        || parameter.access != StructuralAccess::Owned
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

/// The non-erased fields' declared value shapes, in roster order.
fn live_field_shapes<'field>(
    fields: impl Iterator<Item = &'field terminal_psi::StructuralFieldDeclaration>,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<Vec<calling_conventions::ValueShape>> {
    fields
        .filter(|field| !field.relevance.is_erased())
        .map(|field| {
            crate::structural_inputs::structural_reference_input::field_shape(
                &field.field_type,
                declarations,
                &mut Vec::new(),
            )
        })
        .collect()
}
