//! One external scalar transfer on the current descriptor pointer.
//! Geometry selects no backing; receiving legalization retained current loan custody.
use super::{Builder, SelectedInstructionKind, SelectedInstructionProvenance};
use crate::SelectedInstructionError;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use selected_instructions::{
    MachineSemanticKind, PlacedScalarForm, SelectedInstructionId, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
};

pub(super) fn emit(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<bool, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::PlacedScalarAccess {
        realization,
        location,
        indices,
        operand,
        ..
    } = &row.kind
    else {
        return Ok(false);
    };
    let invalid = || SelectedInstructionError::custody();
    let demand = realization.demand;
    let signature = source.structural.as_ref().ok_or_else(invalid)?;
    let root =
        crate::selection::reference_input::root_type(source, location.place).ok_or_else(invalid)?;
    let geometry = crate::structural_inputs::structural_reference_input::reference_source_geometry(
        root,
        &location.path,
        &signature.structural_types,
    )
    .ok_or_else(invalid)?;
    if geometry.selected != demand.carrier
        || !geometry.readable
        || !crate::structural_inputs::structural_reference_input::runtime_indices_match(
            indices,
            &geometry.elements,
        )
    {
        return Err(invalid());
    }
    let form = PlacedScalarForm {
        write: demand.operation == terminal_psi::TerminalPlacedAccessorKind::Write,
        bytes: u8::try_from(demand.transfer_width_bits / 8).map_err(|_| invalid())?,
    };
    if form.ordinary_semantic().is_none()
        || demand.transfer_width_bits != u16::from(form.bytes) * 8
        || operand.is_some() != form.write
        || row.result.is_some() == form.write
    {
        return Err(invalid());
    }
    let constraint = builder
        .constraints
        .keys
        .for_semantic(MachineSemanticKind::PlacedScalarAccess(form))
        .ok_or_else(invalid)?;
    let base = super::structural::place_pointer(builder, row, location.place, 0)?;
    let (referent, consumed) = super::structural::runtime_address::cross_references(
        builder,
        row,
        location.place,
        base,
        indices,
        &geometry.pointer_slots,
    )?;
    let pointer = super::structural::runtime_address::scale(
        builder,
        row,
        location.place,
        geometry.byte_offset,
        referent,
        &indices[consumed..],
    )?;
    let parent =
        super::structural::transport_register(builder, location.place, geometry.byte_offset)?;
    let pointer = super::structural::transport_at_byte_offset(
        builder,
        pointer,
        parent,
        geometry.byte_offset,
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            ..Default::default()
        },
    )?;
    let offset = u32::try_from(demand.byte_offset).map_err(|_| invalid())?;
    let mut registers = vec![pointer];
    if let Some(value) = operand {
        let (_, register, _, scalar) = builder.resolve(*value).ok_or_else(invalid)?;
        if scalar != demand.scalar {
            return Err(invalid());
        }
        registers.push(register);
    }
    let output = row
        .result
        .map(|result| builder.register(result.value, result.definition_site, demand.scalar))
        .transpose()?;
    if let Some(output) = output {
        registers.push(output);
    }
    let instruction = SelectedInstructionId(
        builder
            .instructions
            .len()
            .try_into()
            .map_err(|_| invalid())?,
    );
    builder.transport.memory.push(SelectedMemoryAccess {
        instruction,
        origin: SelectedMemoryAccessOrigin::Operation(row.operation),
        place: location.place,
        byte_offset: offset,
        byte_count: u32::from(form.bytes),
        role: if form.write {
            SelectedMemoryAccessRole::WritePlace
        } else {
            SelectedMemoryAccessRole::ReadPlace
        },
    });
    builder.emit(
        SelectedInstructionKind::PlacedScalarAccess {
            form,
            contract: realization.contract,
            byte_offset: offset,
        },
        constraint,
        &registers,
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: operand
                .iter()
                .copied()
                .chain(row.result.map(|result| result.value))
                .collect(),
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    if let (Some(raw), Some(result)) = (output, row.result) {
        let normalization =
            crate::selection::scalar_call_abi::integer_carrier_normalization(demand.scalar);
        let output = if normalization == SelectedInstructionKind::CopyI64 {
            raw
        } else {
            let output = builder.register(result.value, result.definition_site, demand.scalar)?;
            builder.emit(
                normalization,
                builder.constraints.keys.copy_i64,
                &[raw, output],
                SelectedInstructionProvenance {
                    values: vec![result.value],
                    ..Default::default()
                },
            )?;
            output
        };
        builder
            .definitions
            .push((result.value, output, result.definition_site, demand.scalar));
    }
    Ok(true)
}
