//! Exact semantic shape, value backing, and ABI custody for aggregates.
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use semantic_vocabulary::PlaceId;
use terminal_psi::{StructuralAccess, StructuralTypeShape};

/// Owned aggregate inputs only need addressable storage when an operation observes a
/// field, observes a projected case, lends the original value, or a case dispatch
/// reads its tag and payloads. Pure whole-value transport keeps its ABI
/// fragments; a materialized home becomes the value's storage for later uses.
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
                        StructuralTypeShape::Record { .. }
                            | StructuralTypeShape::Sum { .. }
                            | StructuralTypeShape::Mixed { .. }
                            | StructuralTypeShape::FixedArray { .. }
                    )
            })
    }) {
        return false;
    }
    let dispatched = source.blocks.iter().any(|block| {
        matches!(
            &block.terminator,
            legalized_operations::LegalizedScalarTerminator::StructuralCase {
                source: legalized_operations::LegalizedStructuralCaseSource::Parameter { declaration },
                ..
            } if declaration.place == place
        )
    });
    dispatched || source.blocks.iter().flat_map(|block| &block.instructions).any(|row| {
        match &row.kind {
            LegalizedScalarInstructionKind::StructuralScalarFieldRead { source, .. }
            | LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength { source, .. }
            | LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead { source, .. } => source.place == place,
            LegalizedScalarInstructionKind::StructuralCaseMembership { source, path, .. } => *source == place && !path.is_empty(),
            // Projected primitive reads need the same addressable owned home
            // as field observations, including arrays passed in ABI registers.
            LegalizedScalarInstructionKind::PrimitiveScalarRead { source, .. }
            | LegalizedScalarInstructionKind::StructuralLeafCopy { source, .. } => *source == place,
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
    let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return None;
    };
    let payloads = cases
        .iter()
        .map(|case| {
            case.fields
                .iter()
                .map(|field| {
                    if field.relevance.is_erased() {
                        return None;
                    }
                    crate::structural_inputs::structural_reference_input::field_shape(
                        &field.field_type,
                        &signature.structural_types,
                        &mut Vec::new(),
                    )
                })
                .collect::<Option<Vec<_>>>()
        })
        .collect::<Option<Vec<_>>>()?;
    let layout = calling_conventions::evaluate_conventional_sum_layout(&[], &payloads).ok()?;
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
        terminal_psi::StructuralTypeShape::Sum { .. }
        | terminal_psi::StructuralTypeShape::Mixed { .. } => {
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
    if semantic.access != terminal_psi::StructuralAccess::Owned
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
    if parameter.target.shape != shape
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
            let row = source
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .find(|row| row.operation == *operation)?;
            let (result, shape) = match &row.kind {
                LegalizedScalarInstructionKind::EstablishRecord { result, shape, .. } => {
                    super::record_input::fields(source, row)?;
                    (result, *shape)
                }
                LegalizedScalarInstructionKind::EstablishScalarArray { result, shape, .. } => {
                    super::scalar_array_input::elements(source, row)?;
                    (result, *shape)
                }
                LegalizedScalarInstructionKind::EstablishCase { result, layout, .. } => {
                    (result, layout.shape)
                }
                // A returned reference carrier names its custody slot only;
                // the zero-byte shape keeps the empty-aggregate return ABI.
                LegalizedScalarInstructionKind::EstablishReference { result, shape, .. } => {
                    (result, *shape)
                }
                LegalizedScalarInstructionKind::Call(call) => (
                    call.structural_result.as_ref()?,
                    call.result_placement.as_ref()?.shape,
                ),
                _ => return None,
            };
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
    if structural_type != declared.structural_type
        || multiplicity != declared.multiplicity
        || !declared.qualifications.is_empty()
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
    let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return None;
    };
    let ordinal = cases.iter().position(|case| case.id == *result_case)?;
    if cases.iter().filter(|case| case.id == *result_case).count() != 1 {
        return None;
    }
    let payloads = cases
        .iter()
        .map(|case| {
            case.fields
                .iter()
                .map(|field| {
                    if field.relevance.is_erased() {
                        return None;
                    }
                    crate::structural_inputs::structural_reference_input::field_shape(
                        &field.field_type,
                        &signature.structural_types,
                        &mut Vec::new(),
                    )
                })
                .collect::<Option<Vec<_>>>()
        })
        .collect::<Option<Vec<_>>>()?;
    if calling_conventions::evaluate_conventional_sum_layout(&[], &payloads).ok()? != *layout
        || fields.len() != cases[ordinal].fields.len()
    {
        return None;
    }
    fields
        .iter()
        .zip(&cases[ordinal].fields)
        .zip(&layout.cases[ordinal].fields)
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
    if parameter.access != terminal_psi::StructuralAccess::Owned
        || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
    {
        return None;
    }
    let declaration = source
        .structural
        .as_ref()?
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parameter.structural_type)?;
    if !matches!(
        declaration.shape,
        terminal_psi::StructuralTypeShape::Record { .. }
            | terminal_psi::StructuralTypeShape::Sum { .. }
    ) {
        return None;
    }
    // A successor carries the complete owned payload, including nested cases.
    crate::structural_inputs::structural_reference_input::owned_aggregate_shape(
        parameter.structural_type,
        &source.structural.as_ref()?.structural_types,
    )
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

/// Locate a nominal aggregate's original operation-result home. This is a
/// declaration/layout rejoin; source replay separately establishes availability.
pub(super) fn home(
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
            let result = match &row.kind {
                LegalizedScalarInstructionKind::EstablishScalarArray { result, .. }
                | LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
                    result,
                    ..
                }
                | LegalizedScalarInstructionKind::EstablishZeroedStructural { result, .. }
                | LegalizedScalarInstructionKind::EstablishRecord { result, .. }
                | LegalizedScalarInstructionKind::EstablishCase { result, .. } => result,
                LegalizedScalarInstructionKind::Call(call) => call.structural_result.as_ref()?,
                _ => return None,
            };
            (result.place == place).then_some((row.operation, result))
        });
    let (operation, result) = selected.next()?;
    if selected.next().is_some()
        || result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !result.claims.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !source
            .structural
            .as_ref()?
            .structural_types
            .iter()
            .any(|declaration| {
                declaration.id == result.structural_type
                    && matches!(
                        declaration.shape,
                        StructuralTypeShape::Record { .. }
                            | StructuralTypeShape::Sum { .. }
                            | StructuralTypeShape::FixedArray { .. }
                            | StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                            )
                    )
            })
    {
        return None;
    }
    let signature = source.structural.as_ref()?;
    if !signature.structural_places.iter().any(|declaration| {
        declaration.id == place
            && declaration.kind
                == semantic_vocabulary::StructuralPlaceKind::OperationResult {
                    producer: operation,
                    structural_type: result.structural_type,
                }
    }) {
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
    if matching.next().is_some()
        || parameter.access != StructuralAccess::Owned
        || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !parameter.qualifications.is_empty()
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
    let Some(signature) = source.structural.as_ref() else {
        return false;
    };
    let mut declarations = signature
        .structural_places
        .iter()
        .filter(|place| place.id == expected.place);
    let Some(declaration) = declarations.next() else {
        return false;
    };
    if declarations.next().is_some()
        || declaration.kind
            != (semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer,
                structural_type: expected.structural_type,
            })
    {
        return false;
    }
    let mut producers = source
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|row| {
            let result = match &row.kind {
                LegalizedScalarInstructionKind::EstablishRecord { result, .. }
                | LegalizedScalarInstructionKind::EstablishZeroedStructural { result, .. }
                | LegalizedScalarInstructionKind::EstablishScalarArray { result, .. }
                | LegalizedScalarInstructionKind::EstablishCase { result, .. }
                | LegalizedScalarInstructionKind::StructuralLeafCopy { result, .. }
                | LegalizedScalarInstructionKind::HostedReadByte { result, .. } => result,
                LegalizedScalarInstructionKind::Call(call) => call.structural_result.as_ref()?,
                _ => return None,
            };
            (result.place == expected.place).then_some((row.operation, result))
        });
    producers
        .next()
        .is_some_and(|(operation, result)| operation == producer && result == expected)
        && producers.next().is_none()
}
