//! Encoding and validating the conditional-branch forms: nonzero, unsigned
//! less-than, signed less-than, and the fused compare-with-zero CBNZ form.
//!
//! One conditional-branch family row carries the condition code a canonical
//! `B.cond` word encodes, the inverted code its widened `B.<inverse> +8;
//! B target` pair skips past, and the selected instruction the realization
//! owns. Each request names the family it asserts and the form the layout
//! reserved, so the three branches share one encoder and one independent
//! decoder while a foreign alternative still refuses.

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

/// Displacements, in bytes from the branch's own address, that a one-word
/// conditional branch (`B.cond`, `CBNZ`) encodes: a signed imm19 word count,
/// so ±1 MiB. Layout and frame application realize a taken edge outside it
/// as the widened `B.<inverse> +8; B` pair, whose imm26 reaches ±128 MiB.
pub const AARCH64_CONDITIONAL_BRANCH_BYTE_REACH: std::ops::Range<i64> = -(1 << 20)..(1 << 20);

/// Which layout-resolved realization of a conditional branch a row holds.
/// AArch64 conditional-branch displacement is measured from the branch
/// instruction address and scaled by four bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64ConditionalBranchForm {
    /// One `B.cond imm19` word.
    CanonicalWordV1,
    /// `B.<inverse condition> +8; B imm26`, realized when the taken edge
    /// leaves the `B.cond` imm19 reach.
    WidenedPairV1,
}

/// The arm of the conditional-branch family one resolved alternative names.
struct Aarch64ConditionalBranchArm {
    /// Condition code the canonical `B.cond` word encodes.
    condition: u32,
    /// The inverse of that condition, which the widened pair's first word
    /// skips over.
    inverted_condition: u32,
    /// The selected instruction both realizations own.
    kind: SelectedInstructionKind,
}

fn conditional_branch_arm(family: MachineAlternativeFamily) -> Option<Aarch64ConditionalBranchArm> {
    let (condition, inverted_condition, kind) = match family {
        MachineAlternativeFamily::ConditionalBranchNonZero => (
            0x1, // NE
            0x0, // EQ
            SelectedInstructionKind::ConditionalBranchNonZero,
        ),
        MachineAlternativeFamily::ConditionalBranchU64LessThan => (
            0x3, // LO
            0x2, // HS
            SelectedInstructionKind::ConditionalBranchU64LessThan,
        ),
        MachineAlternativeFamily::ConditionalBranchI64LessThan => (
            0xb, // LT
            0xa, // GE
            SelectedInstructionKind::ConditionalBranchI64LessThan,
        ),
        _ => return None,
    };
    Some(Aarch64ConditionalBranchArm {
        condition,
        inverted_condition,
        kind,
    })
}

/// Encode the layout-resolved realization of one conditional-branch family.
/// `family` is the caller's assertion, never read back from `alternative`.
pub fn encode_aarch64_selected_conditional_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    family: MachineAlternativeFamily,
    form: Aarch64ConditionalBranchForm,
    byte_displacement_from_instruction: i64,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let arm = conditional_branch_request(physical, alternative, family)?;
    let bytes = match form {
        Aarch64ConditionalBranchForm::CanonicalWordV1 => {
            let word_displacement = branch_word_displacement(byte_displacement_from_instruction)?;
            let word = 0x5400_0000 | (((word_displacement as u32) & 0x7ffff) << 5) | arm.condition;
            word.to_le_bytes().to_vec()
        }
        Aarch64ConditionalBranchForm::WidenedPairV1 => {
            let word_displacement =
                widened_branch_word_displacement(byte_displacement_from_instruction)?;
            let skip = 0x5400_0000 | (2 << 5) | arm.inverted_condition;
            let target = 0x1400_0000 | ((word_displacement as u32) & 0x03ff_ffff);
            let mut bytes = Vec::with_capacity(8);
            crate::codec_bytes::push_encoded(&mut bytes, skip.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, target.to_le_bytes());
            bytes
        }
    };
    validate_aarch64_selected_conditional_branch_form(
        physical,
        alternative,
        family,
        form,
        byte_displacement_from_instruction,
        &bytes,
    )
}

/// Independently decode exactly the realization `form` names for `family`,
/// refusing any other word, condition code, or displacement.
pub fn validate_aarch64_selected_conditional_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    family: MachineAlternativeFamily,
    form: Aarch64ConditionalBranchForm,
    byte_displacement_from_instruction: i64,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let arm = conditional_branch_request(physical, alternative, family)?;
    match form {
        Aarch64ConditionalBranchForm::CanonicalWordV1 => {
            branch_word_displacement(byte_displacement_from_instruction)?;
            let word = bytes
                .try_into()
                .ok()
                .map(u32::from_le_bytes)
                .filter(|word| word & 0xff00_001f == 0x5400_0000 | arm.condition)
                .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
            let encoded_imm19 = ((word >> 5) & 0x7ffff) as i32;
            let decoded_words = (encoded_imm19 << 13) >> 13;
            if i64::from(decoded_words) * 4 != byte_displacement_from_instruction {
                return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
            }
            Ok(ValidatedAarch64SelectedFormEncoding {
                bytes: bytes.to_vec(),
                footprint: footprint(arm.kind, &[]),
            })
        }
        Aarch64ConditionalBranchForm::WidenedPairV1 => {
            widened_branch_word_displacement(byte_displacement_from_instruction)?;
            let bytes: [u8; 8] = bytes
                .try_into()
                .map_err(|_| Aarch64SelectedFormEncodingError::MalformedEncoding)?;
            let first = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
            if first != (0x5400_0000 | (2 << 5) | arm.inverted_condition) {
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
                footprint: footprint(arm.kind, &[]),
            })
        }
    }
}

/// The family arm one named family asserts. Both reaches of a conditional
/// branch keep the canonical alternative: the layout, not the key, says which
/// realization the row reserved.
fn conditional_branch_request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    family: MachineAlternativeFamily,
) -> Result<Aarch64ConditionalBranchArm, Aarch64SelectedFormEncodingError> {
    if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
        return Err(Aarch64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    if alternative != (MachineAlternativeKey { family, variant: 0 }) {
        return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
    }
    conditional_branch_arm(family).ok_or(Aarch64SelectedFormEncodingError::AlternativeMismatch)
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

fn branch_word_displacement(
    byte_displacement: i64,
) -> Result<i32, Aarch64SelectedFormEncodingError> {
    if byte_displacement % 4 != 0 {
        return Err(Aarch64SelectedFormEncodingError::BranchDisplacementMisaligned);
    }
    if !AARCH64_CONDITIONAL_BRANCH_BYTE_REACH.contains(&byte_displacement) {
        return Err(Aarch64SelectedFormEncodingError::BranchDisplacementOutsideImm19);
    }
    Ok((byte_displacement / 4) as i32)
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
    if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
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
