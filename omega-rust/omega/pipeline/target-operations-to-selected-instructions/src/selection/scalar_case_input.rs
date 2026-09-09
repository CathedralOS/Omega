//! Exact declaration-to-layout binding for scalar case establishment.
use legalized_operations::{LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind};

/// These places are created inside the graph, not copied from incoming parameters.
/// Each constructor/call and its complete storage is checked at its instruction.
pub(super) fn has_local_sums(source: &LegalizedScalarFunction) -> bool {
    source.blocks.iter().flat_map(|block| &block.instructions).any(|row| match &row.kind {
        LegalizedScalarInstructionKind::EstablishScalarCase { .. } => fields(source, row).is_some(),
        LegalizedScalarInstructionKind::Call(call) => call_result(source, call).is_some(),
        _ => false,
    })
}

pub(super) fn call_result<'a>(source: &LegalizedScalarFunction, call: &'a legalized_operations::LegalizedScalarCall)
    -> Option<(&'a terminal_psi::StructuralOperationResult, &'a calling_conventions::ValuePlacement)> {
    let result = call.structural_result.as_ref()?;
    if result.multiplicity == terminal_psi::StructuralMultiplicity::Linear || !result.claims.is_empty()
        || !result.qualifications.is_empty() || !result.projected_qualifications.is_empty() { return None; }
    let declaration = source.structural.as_ref()?.structural_types.iter().find(|declaration| declaration.id == result.structural_type)?;
    let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else { return None; };
    let payloads = cases.iter().map(|case| case.fields.iter().map(|field| {
        if field.relevance.is_erased() { return None; }
        field.field_type.scalar_type().and_then(super::scalar_call_abi::scalar_shape)
    }).collect::<Option<Vec<_>>>()).collect::<Option<Vec<_>>>()?;
    let layout = calling_conventions::evaluate_conventional_sum_layout(&[], &payloads).ok()?;
    let placement = call.result_placement.as_ref()?;
    if call.call_plan.result.as_ref() != Some(placement) || placement.shape != layout.shape
        || !(1..=2).contains(&placement.locations.len()) { return None; }
    let mut offset = 0;
    for location in &placement.locations {
        let calling_conventions::ValueLocation::Register { value_byte_offset, byte_size, .. } = location else { return None; };
        if *value_byte_offset != offset || !matches!(byte_size, 4 | 8) { return None; }
        offset = offset.checked_add(*byte_size)?;
    }
    (offset == layout.shape.byte_size).then_some((result, placement))
}

pub(super) fn returned<'a>(source: &'a LegalizedScalarFunction, value: &legalized_operations::LegalizedScalarReturnValue)
    -> Option<(selected_instructions::LocalStorageSlotId, &'a calling_conventions::ValuePlacement)> {
    let legalized_operations::LegalizedScalarReturnValue::Structural { defining_operation, result: returned } = value else { return None; };
    let declared = source.structural.as_ref()?.result.as_ref()?;
    let row = source.blocks.iter().flat_map(|block| &block.instructions).find(|row| row.operation == *defining_operation)?;
    let (result, shape) = match &row.kind {
        LegalizedScalarInstructionKind::EstablishScalarCase { result, layout, .. } => (result, layout.shape),
        LegalizedScalarInstructionKind::Call(call) => {
            (call.structural_result.as_ref()?, call.result_placement.as_ref()?.shape)
        }
        _ => return None,
    };
    if result != returned || result.structural_type != declared.structural_type
        || result.multiplicity != declared.multiplicity || !result.claims.is_empty()
        || !result.qualifications.is_empty() || !result.projected_qualifications.is_empty()
        || !declared.qualifications.is_empty() || !declared.projected_qualifications.is_empty()
        || declared.multiplicity == terminal_psi::StructuralMultiplicity::Linear { return None; }
    let placement = source.call_plan.result.as_ref()?;
    if placement.shape != shape || !(1..=2).contains(&placement.locations.len()) { return None; }
    let mut offset = 0;
    for location in &placement.locations {
        let calling_conventions::ValueLocation::Register { value_byte_offset, byte_size, .. } = location else { return None; };
        if *value_byte_offset != offset || !matches!(byte_size, 4 | 8) { return None; }
        offset = offset.checked_add(*byte_size)?;
    }
    if offset != shape.byte_size { return None; }
    Some((selected_instructions::LocalStorageSlotId::Structural { operation: *defining_operation, place: returned.place }, placement))
}

pub(super) fn fields<'a>(source: &'a LegalizedScalarFunction, row: &'a LegalizedScalarInstruction) -> Option<(usize, &'a [terminal_psi::StructuralFieldDeclaration])> {
    let LegalizedScalarInstructionKind::EstablishScalarCase { result, result_case, fields, layout } = &row.kind else { return None; };
    if row.result.is_some() || !result.claims.is_empty() || !result.qualifications.is_empty() || !result.projected_qualifications.is_empty() { return None; }
    let signature = source.structural.as_ref()?;
    let declaration = signature.structural_types.iter().find(|declaration| declaration.id == result.structural_type)?;
    let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else { return None; };
    let ordinal = cases.iter().position(|case| case.id == *result_case)?;
    let case = &cases[ordinal];
    if fields.len() != case.fields.len() || layout.cases.len() != cases.len() || !layout.common_fields.is_empty() || layout.tag_byte_offset != 0 || layout.tag_shape != calling_conventions::ValueShape::integer(4, 4) { return None; }
    let placed = &layout.cases[ordinal].fields;
    if placed.len() != fields.len() || fields.iter().zip(&case.fields).zip(placed).any(|((field, declaration), placed)| {
        field.field != declaration.id || declaration.field_type.scalar_type().and_then(super::scalar_call_abi::scalar_shape) != Some(placed.shape)
    }) { return None; }
    Some((ordinal, &case.fields))
}
