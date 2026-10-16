//! Named foreign-table dispatch after ordinary ABI argument transport.
//! R11 is the ABI's volatile scratch, not a wire parameter. Loading through
//! the retained field offset preserves the table argument for protocol calls.

use super::{
    ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError,
    X86_64SelectedFormFootprint,
};
use register_model::{RegisterOperandAccess, RegisterViewId};
use selected_instructions::{MachineAlternativeKey, SelectedNormalizedForeignCall};

pub fn encode_x86_64_selected_table_call_form(
    record: &SelectedNormalizedForeignCall,
    alternative: MachineAlternativeKey,
    views: &[RegisterViewId],
    accesses: &[RegisterOperandAccess],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let (register, offset) =
        table_address(record).ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
    let mut bytes = vec![0x4c | (register >> 3), 0x8b, 0x98 | (register & 7)];
    bytes.extend_from_slice(&offset.to_le_bytes());
    bytes.extend_from_slice(&[0x41, 0xff, 0xd3]);
    validate_x86_64_selected_table_call_form(record, alternative, views, accesses, &bytes)
}

pub fn validate_x86_64_selected_table_call_form(
    record: &SelectedNormalizedForeignCall,
    alternative: MachineAlternativeKey,
    views: &[RegisterViewId],
    accesses: &[RegisterOperandAccess],
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let invalid = || X86_64SelectedFormEncodingError::EncodedFormMismatch;
    let (register, offset) = table_address(record).ok_or_else(invalid)?;
    if bytes.len() != 10
        || bytes[0] != 0x4c | (register >> 3)
        || bytes[1] != 0x8b
        || bytes[2] != 0x98 | (register & 7)
        || bytes[7..] != [0x41, 0xff, 0xd3]
        || u32::from_le_bytes(bytes[3..7].try_into().map_err(|_| invalid())?) != offset
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
        crate::x86_64_normalized_foreign_call_declaration(record, &row).ok_or_else(invalid)?;
    let [form] = declaration.alternatives.as_slice() else {
        return Err(invalid());
    };
    if form.key != alternative {
        return Err(invalid());
    }
    let registers = |access| {
        views
            .iter()
            .zip(accesses)
            .filter_map(|(view, actual)| (*actual == access).then_some(*view))
            .collect()
    };
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: X86_64SelectedFormFootprint {
            register_reads: registers(RegisterOperandAccess::Use),
            register_writes: registers(RegisterOperandAccess::Def),
            writes_rflags: true,
            encoded: form.encoded.clone(),
        },
    })
}

pub(crate) fn table_address(record: &SelectedNormalizedForeignCall) -> Option<(u8, u32)> {
    let (byte_offset, _) = record.call.binding.table_dispatch()?;
    // Callbacks require relocatable prefixes, which the current table form
    // cannot yet publish. Refuse them before producing incomplete bytes.
    if !record.call.callbacks.is_empty() || byte_offset > i32::MAX as u32 {
        return None;
    }
    // The load destroys R11 before the callee receives its arguments. A
    // custom placement must not use that scratch for a wire argument or
    // hidden result pointer, even when its calling-policy name is native.
    let plan = &record.call.binding.boundary_entry_plan.call;
    let is_scratch = |location: &calling_conventions::ValueLocation| {
        location
            .direct_register()
            .and_then(super::normalized_foreign_call::x86_64_machine_register_encoding)
            == Some(11)
    };
    if plan
        .parameters
        .iter()
        .flat_map(|parameter| &parameter.locations)
        .any(is_scratch)
        || plan
            .result
            .iter()
            .flat_map(|result| &result.locations)
            .any(|location| {
                matches!(
                    location,
                    calling_conventions::ValueLocation::Indirect { .. }
                ) && is_scratch(location)
            })
    {
        return None;
    }
    let operands = record.call.binding.operand_placements();
    let parameter = operands.get(0)?;
    let [
        calling_conventions::ValueLocation::Register {
            register,
            byte_size: 8,
            value_byte_offset: 0,
        },
    ] = parameter.locations.as_slice()
    else {
        return None;
    };
    let register = super::normalized_foreign_call::x86_64_machine_register_encoding(*register)?;
    // Native pointer arguments never occupy an SP register; this fixed form
    // deliberately has no SIB byte or stack-pointer interpretation.
    (register & 7 != 4).then_some((register, byte_offset))
}
