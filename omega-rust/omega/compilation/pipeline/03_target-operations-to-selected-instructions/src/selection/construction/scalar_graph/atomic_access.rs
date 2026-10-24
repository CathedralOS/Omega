//! Selection of one indivisible atomic event.
//! Read/write footprint roles name the same instruction. They describe its
//! storage reach, not separate ordinary memory operations. Narrow observed
//! values are normalized only after the event; no extra memory read is allowed.
use super::{Builder, SelectedInstructionKind, SelectedInstructionProvenance};
use crate::SelectedInstructionError;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use selected_instructions::{
    MachineSemanticKind, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
};
use semantic_vocabulary::ScalarType;

pub(super) fn emit(
    function: usize,
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<bool, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::AtomicAccess {
        location,
        scalar_type,
        field_byte_offset,
        event,
        outcome,
        ..
    } = &row.kind
    else {
        return Ok(false);
    };
    let (form, geometry) = crate::selection::atomic_access::form(source, row)
        .ok_or(SelectedInstructionError::custody())?;
    // The catalog admitting no constraint for an atomic form names the
    // target's implementation limit, not a producer or consumer defect.
    let constraint = builder
        .constraints
        .keys
        .for_semantic(MachineSemanticKind::AtomicAccess(form))
        .ok_or_else(|| SelectedInstructionError::unsupported_shape(function))?;
    let scalar_type = ScalarType::Integer(*scalar_type);
    let base = super::structural::place_pointer(builder, row, location.place, 0)?;
    let (referent, _) = super::structural::runtime_address::cross_references(
        builder,
        row,
        location.place,
        base,
        &[],
        &geometry.pointer_slots,
    )?;
    let address =
        super::structural::transport_register(builder, location.place, geometry.byte_offset)?;
    let address = super::structural::transport_at_byte_offset(
        builder,
        referent,
        address,
        geometry.byte_offset,
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            ..Default::default()
        },
    )?;
    let mut registers = vec![address];
    let values = event.operands();
    for value in &values {
        let (_, register, _, actual_type) = builder
            .resolve(*value)
            .ok_or(SelectedInstructionError::custody())?;
        if actual_type != scalar_type {
            return Err(SelectedInstructionError::custody());
        }
        registers.push(register);
    }
    let raw = if let Some(result) = row.result {
        let register = builder.register(result.value, result.definition_site, scalar_type)?;
        registers.push(register);
        Some(register)
    } else if let Some((result, layout)) = outcome {
        let register = super::structural::transport_register(
            builder,
            result.place,
            layout.cases[0].fields[0].byte_offset,
        )?;
        registers.push(register);
        Some(register)
    } else {
        None
    };
    // An RMW has both roles, but there is exactly one event and one native
    // observed result. Ordering lives in the checked machine effect, not here.
    for (needed, role) in [
        (
            event.observes_resident(),
            SelectedMemoryAccessRole::ReadPlace,
        ),
        (
            event.modifies_resident(),
            SelectedMemoryAccessRole::WritePlace,
        ),
    ] {
        if needed {
            super::structural::memory_at_origin(
                builder,
                SelectedMemoryAccessOrigin::Operation(row.operation),
                location.place,
                *field_byte_offset,
                u32::from(form.width.bytes()),
                role,
            )?;
        }
    }
    builder.emit(
        SelectedInstructionKind::AtomicAccess { form },
        constraint,
        &registers,
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: values
                .into_iter()
                .chain(row.result.map(|result| result.value))
                .collect(),
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    if let (Some(raw), Some(result)) = (raw, row.result) {
        let normalization =
            crate::selection::scalar_call_abi::integer_carrier_normalization(scalar_type);
        let output = if normalization == SelectedInstructionKind::CopyI64 {
            raw
        } else {
            let output = builder.register(result.value, result.definition_site, scalar_type)?;
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
            .push((result.value, output, result.definition_site, scalar_type));
    }
    if outcome.is_some() {
        let raw = raw.ok_or(SelectedInstructionError::custody())?;
        let (_, expected, _, _) = builder
            .resolve(values_for_outcome(event)?)
            .ok_or(SelectedInstructionError::custody())?;
        super::structural::establish_atomic_outcome(
            source,
            row,
            builder,
            raw,
            expected,
            scalar_type,
        )?;
    }
    Ok(true)
}

fn values_for_outcome(
    event: &terminal_psi::AtomicAccessEvent,
) -> Result<semantic_vocabulary::ValueId, SelectedInstructionError> {
    match event {
        terminal_psi::AtomicAccessEvent::CompareExchangeOnce { expected, .. } => Ok(*expected),
        _ => Err(SelectedInstructionError::custody()),
    }
}
