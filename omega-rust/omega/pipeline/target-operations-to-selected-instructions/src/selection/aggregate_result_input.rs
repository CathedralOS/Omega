//! Exact semantic shape and direct ABI custody for local aggregate results.
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};

/// Reconstruct nominal tag identity from the retained readable root contract.
/// This lookup establishes shape and layout, not occurrence liveness: optimized
/// ownership/dominance replay and exact legalized-source replay remain required.
pub(super) fn membership_tag(
    source: &LegalizedScalarFunction,
    place: semantic_vocabulary::PlaceId,
    case: semantic_vocabulary::StructuralCaseId,
) -> Option<u32> {
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
                    LegalizedScalarInstructionKind::EstablishScalarCase { result, .. }
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
                    super::scalar_call_abi::scalar_shape(field.field_type.scalar_type()?)
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
}

/// These places are created inside the graph, not copied from incoming parameters.
/// Each constructor/call and its complete storage is checked at its instruction.
pub(super) fn has_local_aggregates(source: &LegalizedScalarFunction) -> bool {
    source
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .any(|row| match &row.kind {
            LegalizedScalarInstructionKind::EstablishRecord { .. } => {
                super::record_input::fields(source, row).is_some()
            }
            LegalizedScalarInstructionKind::EstablishScalarArray { .. } => {
                super::scalar_array_input::elements(source, row).is_some()
            }
            LegalizedScalarInstructionKind::EstablishScalarCase { .. } => {
                fields(source, row).is_some()
            }
            LegalizedScalarInstructionKind::Call(call) => call_result(source, call).is_some(),
            _ => false,
        })
}

pub(super) fn call_result<'a>(
    source: &LegalizedScalarFunction,
    call: &'a legalized_operations::LegalizedScalarCall,
) -> Option<(
    &'a terminal_psi::StructuralOperationResult,
    &'a calling_conventions::ValuePlacement,
)> {
    let result = call.structural_result.as_ref()?;
    if result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !result.claims.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
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
        terminal_psi::StructuralTypeShape::Record { .. } => {
            crate::structural_reference_input::shape(
                result.structural_type,
                &source.structural.as_ref()?.structural_types,
            )?
        }
        terminal_psi::StructuralTypeShape::FixedArray { .. } => {
            super::scalar_array_input::shape(source, result.structural_type)?.2
        }
        terminal_psi::StructuralTypeShape::Sum { cases } => {
            let payloads = cases
                .iter()
                .map(|case| {
                    case.fields
                        .iter()
                        .map(|field| {
                            if field.relevance.is_erased() {
                                return None;
                            }
                            field
                                .field_type
                                .scalar_type()
                                .and_then(super::scalar_call_abi::scalar_shape)
                        })
                        .collect::<Option<Vec<_>>>()
                })
                .collect::<Option<Vec<_>>>()?;
            let layout =
                calling_conventions::evaluate_conventional_sum_layout(&[], &payloads).ok()?;
            layout.shape
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
    let shape =
        crate::structural_reference_input::parameter_shape(semantic, &signature.structural_types)?;
    if parameter.target.shape != shape
        || placement.shape != shape
        || !(direct_fragments(placement)
            || indirect_result(placement, source.call_plan.policy).is_some())
        || !(inline_argument_fragments(&parameter.target.placement)
            || crate::structural_unit_input::owned_indirect_pointer(
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
    let mut offset = 0u16;
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
            || base.checked_add(u32::from(offset)) != Some(*stack_byte_offset)
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
    copy_offset.checked_add(u32::from(*byte_size))?;
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
                LegalizedScalarInstructionKind::EstablishScalarCase { result, layout, .. } => {
                    (result, layout.shape)
                }
                LegalizedScalarInstructionKind::Call(call) => (
                    call.structural_result.as_ref()?,
                    call.result_placement.as_ref()?.shape,
                ),
                _ => return None,
            };
            if result != returned
                || !result.claims.is_empty()
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
            {
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

pub(super) fn fields<'a>(
    source: &'a LegalizedScalarFunction,
    row: &'a LegalizedScalarInstruction,
) -> Option<(usize, &'a [terminal_psi::StructuralFieldDeclaration])> {
    let LegalizedScalarInstructionKind::EstablishScalarCase {
        result,
        result_case,
        fields,
        layout,
    } = &row.kind
    else {
        return None;
    };
    if row.result.is_some()
        || !result.claims.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
    {
        return None;
    }
    let signature = source.structural.as_ref()?;
    let declaration = signature
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)?;
    let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return None;
    };
    let ordinal = cases.iter().position(|case| case.id == *result_case)?;
    let case = &cases[ordinal];
    if fields.len() != case.fields.len()
        || layout.cases.len() != cases.len()
        || !layout.common_fields.is_empty()
        || layout.tag_byte_offset != 0
        || layout.tag_shape != calling_conventions::ValueShape::integer(4, 4)
    {
        return None;
    }
    let placed = &layout.cases[ordinal].fields;
    if placed.len() != fields.len()
        || fields
            .iter()
            .zip(&case.fields)
            .zip(placed)
            .any(|((field, declaration), placed)| {
                field.field != declaration.id
                    || declaration
                        .field_type
                        .scalar_type()
                        .and_then(super::scalar_call_abi::scalar_shape)
                        != Some(placed.shape)
            })
    {
        return None;
    }
    Some((ordinal, &case.fields))
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
    if let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape {
        if fields.iter().any(|field| {
            field.relevance.is_erased()
                || !matches!(
                    field.field_type,
                    terminal_psi::StructuralFieldType::Scalar(_)
                        | terminal_psi::StructuralFieldType::IeeeFloat(_)
                )
        }) {
            return None;
        }
        return crate::structural_reference_input::shape(
            parameter.structural_type,
            &source.structural.as_ref()?.structural_types,
        );
    }
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
                    let scalar @ semantic_vocabulary::ScalarType::Integer(_) =
                        field.field_type.scalar_type()?
                    else {
                        return None;
                    };
                    super::scalar_call_abi::scalar_shape(scalar)
                })
                .collect::<Option<Vec<_>>>()
        })
        .collect::<Option<Vec<_>>>()?;
    Some(
        calling_conventions::evaluate_conventional_sum_layout(&[], &payloads)
            .ok()?
            .shape,
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
