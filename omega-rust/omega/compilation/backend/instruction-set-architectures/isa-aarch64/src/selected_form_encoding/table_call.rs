//! Foreign table dispatch after ordinary ABI argument transport. IP1 (x17)
//! holds the address calculation and callee; IP0 (x16) may independently carry
//! a dispatch-only table. Protocol table arguments remain untouched for entry.

use super::{
    Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
    ValidatedAarch64SelectedFormEncoding,
};
use register_model::{RegisterOperandAccess, RegisterViewId, views_with_access};
use selected_instructions::{MachineAlternativeKey, SelectedNormalizedForeignCall};

pub fn encode_aarch64_selected_table_call_form(
    record: &SelectedNormalizedForeignCall,
    alternative: MachineAlternativeKey,
    views: &[RegisterViewId],
    accesses: &[RegisterOperandAccess],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let (base, offset) =
        table_address(record).ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
    let mut bytes = Vec::with_capacity(20);
    // A fixed full-u32 offset form also handles unaligned packed fields and
    // large tables; it does not silently constrain layout to scaled imm12.
    for word in [
        0xd280_0011 | ((offset & 0xffff) << 5),
        0xf2a0_0011 | ((offset >> 16) << 5),
        0x8b11_0011 | (u32::from(base) << 5),
        0xf940_0231,
        0xd63f_0220,
    ] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    validate_aarch64_selected_table_call_form(record, alternative, views, accesses, &bytes)
}

pub fn validate_aarch64_selected_table_call_form(
    record: &SelectedNormalizedForeignCall,
    alternative: MachineAlternativeKey,
    views: &[RegisterViewId],
    accesses: &[RegisterOperandAccess],
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let invalid = || Aarch64SelectedFormEncodingError::EncodedFormMismatch;
    let (base, offset) = table_address(record).ok_or_else(invalid)?;
    let words = bytes
        .chunks_exact(4)
        .map(|word| {
            <[u8; 4]>::try_from(word)
                .map(u32::from_le_bytes)
                .map_err(|_| invalid())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let [low, high, add, load, call] = words.as_slice() else {
        return Err(invalid());
    };
    if bytes.len() != 20
        || low & 0xffe0_001f != 0xd280_0011
        || high & 0xffe0_001f != 0xf2a0_0011
        || ((low >> 5) & 0xffff) != offset & 0xffff
        || ((high >> 5) & 0xffff) != offset >> 16
        || *add != 0x8b11_0011 | (u32::from(base) << 5)
        || *load != 0xf940_0231
        || *call != 0xd63f_0220
    {
        return Err(invalid());
    }
    let row = crate::machine_effects::normalized_foreign_constraint(record).ok_or_else(invalid)?;
    if views.len() != row.operands.len()
        || accesses.len() != row.operands.len()
        || row
            .operands
            .iter()
            .zip(views.iter().zip(accesses))
            .any(|(operand, (view, access))| {
                operand.fixed_view != Some(*view) || operand.access != *access
            })
    {
        return Err(invalid());
    }
    let declaration =
        crate::aarch64_normalized_foreign_call_declaration(record, &row).ok_or_else(invalid)?;
    let [form] = declaration.alternatives.as_slice() else {
        return Err(invalid());
    };
    if form.key != alternative {
        return Err(invalid());
    }
    let registers = |access| views_with_access(views, accesses, access);
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: registers(RegisterOperandAccess::Use),
            register_writes: registers(RegisterOperandAccess::Def),
            writes_nzcv: true,
            encoded: form.encoded.clone(),
        },
    })
}

pub(crate) fn table_address(record: &SelectedNormalizedForeignCall) -> Option<(u8, u32)> {
    use calling_conventions::{MachineRegister, ValueLocation};
    let (byte_offset, dispatch_only) = record.call.binding.table_dispatch()?;
    if !record.call.callbacks.is_empty() {
        return None;
    }
    let plan = &record.call.binding.boundary_entry_plan.call;
    let scratch = |location: &ValueLocation| {
        location.direct_register() == Some(MachineRegister::Aarch64X(17))
            || dispatch_only && location.direct_register() == Some(MachineRegister::Aarch64X(16))
    };
    if plan
        .parameters
        .iter()
        .flat_map(|parameter| &parameter.locations)
        .any(scratch)
        || plan
            .result
            .iter()
            .flat_map(|result| &result.locations)
            .any(|location| matches!(location, ValueLocation::Indirect { .. }) && scratch(location))
    {
        return None;
    }
    let operands = record.call.binding.operand_placements();
    let parameter = operands.get(0)?;
    let [
        ValueLocation::Register {
            register: MachineRegister::Aarch64X(register),
            byte_size: 8,
            value_byte_offset: 0,
        },
    ] = parameter.locations.as_slice()
    else {
        return None;
    };
    (*register <= 30 && *register != 17).then_some((*register, byte_offset))
}
