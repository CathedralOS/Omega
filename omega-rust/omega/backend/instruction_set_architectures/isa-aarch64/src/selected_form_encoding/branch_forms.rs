//! Encoding and validating the branch forms: nonzero, unsigned and signed
//! less-than branches, and the fused compare-with-zero CBNZ form.

use crate::aarch64_physical_register_model;
use crate::selected_form_encoding::decoding::footprint;
use crate::selected_form_encoding::selected_forms::resolve_registers;
use crate::selected_form_encoding::{
    Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
    ValidatedAarch64SelectedFormEncoding,
};
use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstructionKind,
};

/// Encode the canonical layout-resolved realization of
/// `ConditionalBranchNonZero`. AArch64 conditional-branch displacement is
/// measured from the branch instruction address and scaled by four bytes.
pub fn encode_aarch64_selected_nonzero_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction: i64,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchNonZero,
    )?;
    let word_displacement = branch_word_displacement(byte_displacement_from_instruction)?;
    let word = 0x5400_0001 | (((word_displacement as u32) & 0x7ffff) << 5);
    let bytes = word.to_le_bytes();
    validate_aarch64_selected_nonzero_branch_form(
        physical,
        alternative,
        byte_displacement_from_instruction,
        &bytes,
    )
}

pub fn validate_aarch64_selected_nonzero_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction: i64,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchNonZero,
    )?;
    branch_word_displacement(byte_displacement_from_instruction)?;
    let word = bytes
        .try_into()
        .ok()
        .map(u32::from_le_bytes)
        .filter(|word| word & 0xff00_001f == 0x5400_0001)
        .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
    let encoded_imm19 = ((word >> 5) & 0x7ffff) as i32;
    let decoded_words = (encoded_imm19 << 13) >> 13;
    let decoded_bytes = i64::from(decoded_words) * 4;
    if decoded_bytes != byte_displacement_from_instruction {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(SelectedInstructionKind::ConditionalBranchNonZero, &[]),
    })
}

/// Encode the canonical AArch64 unsigned-lower conditional branch. `B.LO`
/// uses condition code `0b0011`, with its signed imm19 measured from the
/// branch instruction address and scaled by four bytes.
pub fn encode_aarch64_selected_u64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction: i64,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchU64LessThan,
    )?;
    let word_displacement = branch_word_displacement(byte_displacement_from_instruction)?;
    let word = 0x5400_0003 | (((word_displacement as u32) & 0x7ffff) << 5);
    validate_aarch64_selected_u64_less_than_branch_form(
        physical,
        alternative,
        byte_displacement_from_instruction,
        &word.to_le_bytes(),
    )
}

/// Independently decode exactly one canonical AArch64 `B.LO imm19`.
pub fn validate_aarch64_selected_u64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction: i64,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchU64LessThan,
    )?;
    branch_word_displacement(byte_displacement_from_instruction)?;
    let word = bytes
        .try_into()
        .ok()
        .map(u32::from_le_bytes)
        .filter(|word| word & 0xff00_001f == 0x5400_0003)
        .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
    let encoded_imm19 = ((word >> 5) & 0x7ffff) as i32;
    let decoded_words = (encoded_imm19 << 13) >> 13;
    if i64::from(decoded_words) * 4 != byte_displacement_from_instruction {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(SelectedInstructionKind::ConditionalBranchU64LessThan, &[]),
    })
}

/// Encode the canonical AArch64 signed-less-than conditional branch. `B.LT`
/// uses condition code `0b1011`, with its signed imm19 measured from the
/// branch instruction address and scaled by four bytes.
pub fn encode_aarch64_selected_i64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction: i64,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchI64LessThan,
    )?;
    let word_displacement = branch_word_displacement(byte_displacement_from_instruction)?;
    let word = 0x5400_000b | (((word_displacement as u32) & 0x7ffff) << 5);
    validate_aarch64_selected_i64_less_than_branch_form(
        physical,
        alternative,
        byte_displacement_from_instruction,
        &word.to_le_bytes(),
    )
}

/// Independently decode exactly one canonical AArch64 `B.LT imm19`.
pub fn validate_aarch64_selected_i64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction: i64,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchI64LessThan,
    )?;
    branch_word_displacement(byte_displacement_from_instruction)?;
    let word = bytes
        .try_into()
        .ok()
        .map(u32::from_le_bytes)
        .filter(|word| word & 0xff00_001f == 0x5400_000b)
        .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
    let encoded_imm19 = ((word >> 5) & 0x7ffff) as i32;
    let decoded_words = (encoded_imm19 << 13) >> 13;
    if i64::from(decoded_words) * 4 != byte_displacement_from_instruction {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(SelectedInstructionKind::ConditionalBranchI64LessThan, &[]),
    })
}

/// Encode the widened layout-resolved realization of `ConditionalBranchNonZero`
/// for taken edges beyond the `B.cond` imm19 range: `B.EQ +8` falls through to
/// an unconditional `B` that carries the signed imm26 displacement measured
/// from the second word's own address.
pub fn encode_aarch64_selected_nonzero_widened_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction: i64,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    encode_widened_branch(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchNonZero,
        SelectedInstructionKind::ConditionalBranchNonZero,
        0x0,
        byte_displacement_from_instruction,
    )
}

/// Independently decode the widened `B.EQ +8; B target` pair.
pub fn validate_aarch64_selected_nonzero_widened_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction: i64,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_widened_branch(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchNonZero,
        SelectedInstructionKind::ConditionalBranchNonZero,
        0x0,
        byte_displacement_from_instruction,
        bytes,
    )
}

/// Encode the widened unsigned-lower conditional branch: `B.HS +8` (the inverse
/// of `B.LO`, condition code `0b0010`) skips to the unconditional `B`.
pub fn encode_aarch64_selected_u64_less_than_widened_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction: i64,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    encode_widened_branch(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchU64LessThan,
        SelectedInstructionKind::ConditionalBranchU64LessThan,
        0x2,
        byte_displacement_from_instruction,
    )
}

/// Independently decode the widened `B.HS +8; B target` pair.
pub fn validate_aarch64_selected_u64_less_than_widened_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction: i64,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_widened_branch(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchU64LessThan,
        SelectedInstructionKind::ConditionalBranchU64LessThan,
        0x2,
        byte_displacement_from_instruction,
        bytes,
    )
}

/// Encode the widened signed-less-than conditional branch: `B.GE +8` (the
/// inverse of `B.LT`, condition code `0b1010`) skips to the unconditional `B`.
pub fn encode_aarch64_selected_i64_less_than_widened_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction: i64,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    encode_widened_branch(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchI64LessThan,
        SelectedInstructionKind::ConditionalBranchI64LessThan,
        0xa,
        byte_displacement_from_instruction,
    )
}

/// Independently decode the widened `B.GE +8; B target` pair.
pub fn validate_aarch64_selected_i64_less_than_widened_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction: i64,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_widened_branch(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchI64LessThan,
        SelectedInstructionKind::ConditionalBranchI64LessThan,
        0xa,
        byte_displacement_from_instruction,
        bytes,
    )
}

fn encode_widened_branch(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    family: MachineAlternativeFamily,
    kind: SelectedInstructionKind,
    inverted_condition: u32,
    byte_displacement_from_instruction: i64,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_branch_request(physical, alternative, family)?;
    let word_displacement = widened_branch_word_displacement(byte_displacement_from_instruction)?;
    let skip = 0x5400_0000 | (2 << 5) | inverted_condition;
    let target = 0x1400_0000 | ((word_displacement as u32) & 0x03ff_ffff);
    let mut bytes = Vec::with_capacity(8);
    bytes.extend_from_slice(&skip.to_le_bytes());
    bytes.extend_from_slice(&target.to_le_bytes());
    validate_widened_branch(
        physical,
        alternative,
        family,
        kind,
        inverted_condition,
        byte_displacement_from_instruction,
        &bytes,
    )
}

fn validate_widened_branch(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    family: MachineAlternativeFamily,
    kind: SelectedInstructionKind,
    inverted_condition: u32,
    byte_displacement_from_instruction: i64,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_branch_request(physical, alternative, family)?;
    widened_branch_word_displacement(byte_displacement_from_instruction)?;
    let bytes: [u8; 8] = bytes
        .try_into()
        .map_err(|_| Aarch64SelectedFormEncodingError::MalformedEncoding)?;
    let first = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    if first != (0x5400_0000 | (2 << 5) | inverted_condition) {
        return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
    }
    let second = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if second & 0xfc00_0000 != 0x1400_0000 {
        return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
    }
    let decoded = i64::from(((second & 0x03ff_ffff) << 6) as i32 >> 6) * 4;
    if decoded != byte_displacement_from_instruction - 4 {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(kind, &[]),
    })
}

fn widened_branch_word_displacement(
    byte_displacement: i64,
) -> Result<i32, Aarch64SelectedFormEncodingError> {
    if byte_displacement % 4 != 0 {
        return Err(Aarch64SelectedFormEncodingError::BranchDisplacementMisaligned);
    }
    // The unconditional `B` sits one word into the row, so its own displacement
    // is the row displacement minus one word.
    let words = byte_displacement / 4 - 1;
    if !(-(1_i64 << 25)..(1_i64 << 25)).contains(&words) {
        return Err(Aarch64SelectedFormEncodingError::BranchDisplacementOutsideImm26);
    }
    Ok(words as i32)
}

fn validate_branch_request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    family: MachineAlternativeFamily,
) -> Result<(), Aarch64SelectedFormEncodingError> {
    if physical.model() != &aarch64_physical_register_model() {
        return Err(Aarch64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    if alternative != (MachineAlternativeKey { family, variant: 0 }) {
        return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
    }
    Ok(())
}

fn branch_word_displacement(
    byte_displacement: i64,
) -> Result<i32, Aarch64SelectedFormEncodingError> {
    if byte_displacement % 4 != 0 {
        return Err(Aarch64SelectedFormEncodingError::BranchDisplacementMisaligned);
    }
    let words = byte_displacement / 4;
    if !(-(1_i64 << 18)..(1_i64 << 18)).contains(&words) {
        return Err(Aarch64SelectedFormEncodingError::BranchDisplacementOutsideImm19);
    }
    Ok(words as i32)
}

/// Encode the layout-resolved AArch64 realization selected by
/// `Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1`. The displacement is
/// measured from the `CBNZ` instruction address and scaled by four bytes.
pub fn encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
    physical: &ValidatedPhysicalRegisterModel,
    source: RegisterViewId,
    byte_displacement_from_instruction: i64,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let register = validate_cbnz_request(physical, source)?;
    let word_displacement = branch_word_displacement(byte_displacement_from_instruction)?;
    let word = 0xb500_0000 | (((word_displacement as u32) & 0x7ffff) << 5) | u32::from(register);
    validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
        physical,
        source,
        byte_displacement_from_instruction,
        &word.to_le_bytes(),
    )
}

/// Independently decode and validate the exact 64-bit `CBNZ` realization.
pub fn validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
    physical: &ValidatedPhysicalRegisterModel,
    source: RegisterViewId,
    byte_displacement_from_instruction: i64,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let register = validate_cbnz_request(physical, source)?;
    branch_word_displacement(byte_displacement_from_instruction)?;
    let word = bytes
        .try_into()
        .ok()
        .map(u32::from_le_bytes)
        .filter(|word| word & 0xff00_0000 == 0xb500_0000)
        .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
    if word & 0x1f != u32::from(register) {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let encoded_imm19 = ((word >> 5) & 0x7ffff) as i32;
    let decoded_words = (encoded_imm19 << 13) >> 13;
    if i64::from(decoded_words) * 4 != byte_displacement_from_instruction {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: cbnz_footprint(source),
    })
}

fn validate_cbnz_request(
    physical: &ValidatedPhysicalRegisterModel,
    source: RegisterViewId,
) -> Result<u8, Aarch64SelectedFormEncodingError> {
    if physical.model() != &aarch64_physical_register_model() {
        return Err(Aarch64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    let registers = resolve_registers(physical, &[source])?;
    registers
        .first()
        .copied()
        .ok_or(Aarch64SelectedFormEncodingError::OperandCountMismatch)
}

fn cbnz_footprint(source: RegisterViewId) -> Aarch64SelectedFormFootprint {
    let physical = aarch64_physical_register_model();
    let pc = physical.view_named("pc").unwrap().units.clone();
    Aarch64SelectedFormFootprint {
        register_reads: vec![source],
        register_writes: vec![],
        writes_nzcv: false,
        encoded: MachineEncodedEffects {
            // The selected branch has no operand zero. The optimizer artifact
            // separately qualifies this read by the compare instruction and
            // operand that own it.
            external_operand_reads: vec![],
            external_operand_writes: vec![],
            implicit_unit_uses: pc.clone(),
            implicit_unit_defs: pc,
            implicit_unit_clobbers: vec![],
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: MachineEncodedControlEffect::ConditionalRelativeBranchV1,
        },
    }
}
