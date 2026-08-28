use omega_register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use omega_terminal_selected_instructions::{
    TerminalMachineAlternativeFamily, TerminalMachineAlternativeKey,
    TerminalMachineEncodedControlEffect, TerminalMachineEncodedEffects,
    TerminalMachineEncodedMemoryEffect, TerminalMachineEncodedStackEffect,
    TerminalMachineEncodedTrapBehavior, TerminalSelectedInstructionKind,
};
use psi_core::IntegerValue;

use crate::aarch64_physical_register_model;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aarch64SelectedFormFootprint {
    pub register_reads: Vec<RegisterViewId>,
    pub register_writes: Vec<RegisterViewId>,
    pub writes_nzcv: bool,
    pub encoded: TerminalMachineEncodedEffects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAarch64SelectedFormEncoding {
    bytes: Vec<u8>,
    footprint: Aarch64SelectedFormFootprint,
}

impl ValidatedAarch64SelectedFormEncoding {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn footprint(&self) -> &Aarch64SelectedFormFootprint {
        &self.footprint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aarch64SelectedFormEncodingError {
    NonCanonicalPhysicalModel,
    LayoutDependentForm,
    AlternativeMismatch,
    OperandCountMismatch,
    UnknownOrNonGpr64View(RegisterViewId),
    IntegerOutsideI64Bits,
    ImmediateOutsideU12,
    BranchDisplacementMisaligned,
    BranchDisplacementOutsideImm19,
    MalformedEncoding,
    EncodedFormMismatch,
}

impl std::fmt::Display for Aarch64SelectedFormEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid AArch64 selected-form encoding: {self:?}"
        )
    }
}

impl std::error::Error for Aarch64SelectedFormEncodingError {}

/// Encode the canonical layout-resolved realization of
/// `ConditionalBranchNonZero`. AArch64 conditional-branch displacement is
/// measured from the branch instruction address and scaled by four bytes.
pub fn encode_aarch64_terminal_selected_nonzero_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: TerminalMachineAlternativeKey,
    byte_displacement_from_instruction: i64,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_branch_request(physical, alternative)?;
    let word_displacement = branch_word_displacement(byte_displacement_from_instruction)?;
    let word = 0x5400_0001 | (((word_displacement as u32) & 0x7ffff) << 5);
    let bytes = word.to_le_bytes();
    validate_aarch64_terminal_selected_nonzero_branch_form(
        physical,
        alternative,
        byte_displacement_from_instruction,
        &bytes,
    )
}

pub fn validate_aarch64_terminal_selected_nonzero_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: TerminalMachineAlternativeKey,
    byte_displacement_from_instruction: i64,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_branch_request(physical, alternative)?;
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
        footprint: footprint(
            TerminalSelectedInstructionKind::ConditionalBranchNonZero,
            &[],
        ),
    })
}

fn validate_branch_request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: TerminalMachineAlternativeKey,
) -> Result<(), Aarch64SelectedFormEncodingError> {
    if physical.model() != &aarch64_physical_register_model() {
        return Err(Aarch64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    if alternative
        != (TerminalMachineAlternativeKey {
            family: TerminalMachineAlternativeFamily::ConditionalBranchNonZero,
            variant: 0,
        })
    {
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
pub fn encode_aarch64_terminal_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
    physical: &ValidatedPhysicalRegisterModel,
    source: RegisterViewId,
    byte_displacement_from_instruction: i64,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let register = validate_cbnz_request(physical, source)?;
    let word_displacement = branch_word_displacement(byte_displacement_from_instruction)?;
    let word = 0xb500_0000 | (((word_displacement as u32) & 0x7ffff) << 5) | u32::from(register);
    validate_aarch64_terminal_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
        physical,
        source,
        byte_displacement_from_instruction,
        &word.to_le_bytes(),
    )
}

/// Independently decode and validate the exact 64-bit `CBNZ` realization.
pub fn validate_aarch64_terminal_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
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
        encoded: TerminalMachineEncodedEffects {
            // The selected branch has no operand zero. The optimizer artifact
            // separately qualifies this read by the compare instruction and
            // operand that own it.
            external_operand_reads: vec![],
            external_operand_writes: vec![],
            implicit_unit_uses: pc.clone(),
            implicit_unit_defs: pc,
            implicit_unit_clobbers: vec![],
            memory: TerminalMachineEncodedMemoryEffect::NoneV1,
            stack: TerminalMachineEncodedStackEffect::UnchangedV1,
            trap: TerminalMachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: TerminalMachineEncodedControlEffect::ConditionalRelativeBranchV1,
        },
    }
}

pub fn encode_aarch64_terminal_selected_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: TerminalSelectedInstructionKind,
    alternative: TerminalMachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_request(physical, kind, alternative, operands)?;
    let registers = resolve_registers(physical, operands)?;
    validate_return_home(kind, &registers)?;
    let bytes = encode_unchecked(kind, &registers)?;
    validate_aarch64_terminal_selected_form_encoding(physical, kind, alternative, operands, &bytes)
}

pub fn validate_aarch64_terminal_selected_form_encoding(
    physical: &ValidatedPhysicalRegisterModel,
    kind: TerminalSelectedInstructionKind,
    alternative: TerminalMachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_request(physical, kind, alternative, operands)?;
    let registers = resolve_registers(physical, operands)?;
    validate_return_home(kind, &registers)?;
    let decoded = decode_words(bytes)?;
    validate_decoded(kind, &registers, &decoded)?;
    let canonical = encode_unchecked(kind, &registers)?;
    if bytes != canonical {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(kind, operands),
    })
}

fn validate_request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: TerminalSelectedInstructionKind,
    alternative: TerminalMachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    if physical.model() != &aarch64_physical_register_model() {
        return Err(Aarch64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    let (family, count) = family_and_operand_count(kind)?;
    if alternative != (TerminalMachineAlternativeKey { family, variant: 0 }) {
        return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
    }
    if operands.len() != count {
        return Err(Aarch64SelectedFormEncodingError::OperandCountMismatch);
    }
    Ok(())
}

fn family_and_operand_count(
    kind: TerminalSelectedInstructionKind,
) -> Result<(TerminalMachineAlternativeFamily, usize), Aarch64SelectedFormEncodingError> {
    Ok(match kind {
        TerminalSelectedInstructionKind::CompareI64Zero => {
            (TerminalMachineAlternativeFamily::CompareI64Zero, 1)
        }
        TerminalSelectedInstructionKind::MaterializeI64 { .. } => {
            (TerminalMachineAlternativeFamily::MaterializeI64, 1)
        }
        TerminalSelectedInstructionKind::CopyI64 => (TerminalMachineAlternativeFamily::CopyI64, 2),
        TerminalSelectedInstructionKind::ExactAddI64 { .. } => {
            (TerminalMachineAlternativeFamily::ExactAddI64, 3)
        }
        TerminalSelectedInstructionKind::ExactSubtractI64 { .. } => {
            (TerminalMachineAlternativeFamily::ExactSubtractI64, 3)
        }
        TerminalSelectedInstructionKind::ExactAddI64Immediate { .. } => {
            (TerminalMachineAlternativeFamily::ExactAddI64Immediate, 2)
        }
        TerminalSelectedInstructionKind::ExactSubtractI64Immediate { .. } => (
            TerminalMachineAlternativeFamily::ExactSubtractI64Immediate,
            2,
        ),
        TerminalSelectedInstructionKind::ReturnI64 => {
            (TerminalMachineAlternativeFamily::ReturnI64, 1)
        }
        TerminalSelectedInstructionKind::ReturnUnit => {
            (TerminalMachineAlternativeFamily::ReturnUnit, 0)
        }
        TerminalSelectedInstructionKind::ConditionalBranchNonZero => {
            return Err(Aarch64SelectedFormEncodingError::LayoutDependentForm);
        }
    })
}

fn validate_return_home(
    kind: TerminalSelectedInstructionKind,
    registers: &[u8],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    if matches!(kind, TerminalSelectedInstructionKind::ReturnI64) && registers != [0] {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(())
}

fn resolve_registers(
    physical: &ValidatedPhysicalRegisterModel,
    operands: &[RegisterViewId],
) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
    operands
        .iter()
        .map(|id| {
            let view = physical
                .model()
                .views
                .iter()
                .find(|view| view.id == *id)
                .ok_or(Aarch64SelectedFormEncodingError::UnknownOrNonGpr64View(*id))?;
            let Some(index) = view.name.strip_prefix('x') else {
                return Err(Aarch64SelectedFormEncodingError::UnknownOrNonGpr64View(*id));
            };
            let index = index
                .parse::<u8>()
                .ok()
                .filter(|index| *index <= 30)
                .ok_or(Aarch64SelectedFormEncodingError::UnknownOrNonGpr64View(*id))?;
            if view.bits != 64 || !view.allocatable {
                return Err(Aarch64SelectedFormEncodingError::UnknownOrNonGpr64View(*id));
            }
            Ok(index)
        })
        .collect()
}

fn integer_bits(value: IntegerValue) -> Result<u64, Aarch64SelectedFormEncodingError> {
    match value {
        IntegerValue::Signed(value) => i64::try_from(value)
            .map(|value| value as u64)
            .map_err(|_| Aarch64SelectedFormEncodingError::IntegerOutsideI64Bits),
        IntegerValue::Unsigned(value) => u64::try_from(value)
            .map_err(|_| Aarch64SelectedFormEncodingError::IntegerOutsideI64Bits),
    }
}

fn u12(value: IntegerValue) -> Result<u16, Aarch64SelectedFormEncodingError> {
    match value {
        IntegerValue::Unsigned(value) if value <= 4095 => Ok(value as u16),
        _ => Err(Aarch64SelectedFormEncodingError::ImmediateOutsideU12),
    }
}

fn encode_unchecked(
    kind: TerminalSelectedInstructionKind,
    registers: &[u8],
) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
    let mut words = Vec::new();
    match kind {
        TerminalSelectedInstructionKind::MaterializeI64 { value } => {
            append_canonical_materialization(&mut words, registers[0], integer_bits(value)?);
        }
        TerminalSelectedInstructionKind::CopyI64 => {
            words.push(0xaa00_03e0 | (u32::from(registers[0]) << 16) | u32::from(registers[1]));
        }
        TerminalSelectedInstructionKind::CompareI64Zero => {
            words.push(0xf100_001f | (u32::from(registers[0]) << 5));
        }
        TerminalSelectedInstructionKind::ExactAddI64 { .. } => {
            words.push(
                0x8b00_0000
                    | (u32::from(registers[1]) << 16)
                    | (u32::from(registers[0]) << 5)
                    | u32::from(registers[2]),
            );
        }
        TerminalSelectedInstructionKind::ExactAddI64Immediate { immediate, .. } => {
            words.push(
                0x9100_0000
                    | (u32::from(u12(immediate)?) << 10)
                    | (u32::from(registers[0]) << 5)
                    | u32::from(registers[1]),
            );
        }
        TerminalSelectedInstructionKind::ExactSubtractI64 { .. } => {
            words.push(
                0xcb00_0000
                    | (u32::from(registers[1]) << 16)
                    | (u32::from(registers[0]) << 5)
                    | u32::from(registers[2]),
            );
        }
        TerminalSelectedInstructionKind::ExactSubtractI64Immediate { immediate, .. } => {
            words.push(
                0xd100_0000
                    | (u32::from(u12(immediate)?) << 10)
                    | (u32::from(registers[0]) << 5)
                    | u32::from(registers[1]),
            );
        }
        TerminalSelectedInstructionKind::ReturnI64
        | TerminalSelectedInstructionKind::ReturnUnit => words.push(0xd65f_03c0),
        TerminalSelectedInstructionKind::ConditionalBranchNonZero => {
            return Err(Aarch64SelectedFormEncodingError::LayoutDependentForm);
        }
    }
    Ok(words.into_iter().flat_map(u32::to_le_bytes).collect())
}

fn append_canonical_materialization(words: &mut Vec<u32>, register: u8, value: u64) {
    let chunks = std::array::from_fn::<_, 4, _>(|index| ((value >> (index * 16)) & 0xffff) as u16);
    words.push(0xd280_0000 | (u32::from(chunks[0]) << 5) | u32::from(register));
    for (index, chunk) in chunks.into_iter().enumerate().skip(1) {
        if chunk != 0 {
            words.push(
                0xf280_0000
                    | (u32::try_from(index).expect("halfword index fits u32") << 21)
                    | (u32::from(chunk) << 5)
                    | u32::from(register),
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecodedWord {
    MovZ {
        register: u8,
        shift: u8,
        immediate: u16,
    },
    MovK {
        register: u8,
        shift: u8,
        immediate: u16,
    },
    Copy {
        source: u8,
        destination: u8,
    },
    CompareZero {
        source: u8,
    },
    Add {
        left: u8,
        right: u8,
        destination: u8,
    },
    AddImmediate {
        source: u8,
        immediate: u16,
        destination: u8,
    },
    Subtract {
        left: u8,
        right: u8,
        destination: u8,
    },
    SubtractImmediate {
        source: u8,
        immediate: u16,
        destination: u8,
    },
    Return,
}

fn decode_words(bytes: &[u8]) -> Result<Vec<DecodedWord>, Aarch64SelectedFormEncodingError> {
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
    }
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| decode_word(u32::from_le_bytes(*bytes)))
        .collect()
}

fn decode_word(word: u32) -> Result<DecodedWord, Aarch64SelectedFormEncodingError> {
    let register = (word & 0x1f) as u8;
    let shift = ((word >> 21) & 0x3) as u8;
    let immediate = ((word >> 5) & 0xffff) as u16;
    if word & 0xffe0_0000 == 0xd280_0000 {
        return Ok(DecodedWord::MovZ {
            register,
            shift,
            immediate,
        });
    }
    if word & 0xff80_0000 == 0xf280_0000 {
        return Ok(DecodedWord::MovK {
            register,
            shift,
            immediate,
        });
    }
    if word & 0xffe0_ffe0 == 0xaa00_03e0 {
        return Ok(DecodedWord::Copy {
            source: ((word >> 16) & 0x1f) as u8,
            destination: register,
        });
    }
    if word & 0xffff_fc1f == 0xf100_001f {
        return Ok(DecodedWord::CompareZero {
            source: ((word >> 5) & 0x1f) as u8,
        });
    }
    if word & 0xffe0_fc00 == 0x8b00_0000 {
        return Ok(DecodedWord::Add {
            left: ((word >> 5) & 0x1f) as u8,
            right: ((word >> 16) & 0x1f) as u8,
            destination: register,
        });
    }
    if word & 0xffc0_0000 == 0x9100_0000 {
        return Ok(DecodedWord::AddImmediate {
            source: ((word >> 5) & 0x1f) as u8,
            immediate: ((word >> 10) & 0xfff) as u16,
            destination: register,
        });
    }
    if word & 0xffe0_fc00 == 0xcb00_0000 {
        return Ok(DecodedWord::Subtract {
            left: ((word >> 5) & 0x1f) as u8,
            right: ((word >> 16) & 0x1f) as u8,
            destination: register,
        });
    }
    if word & 0xffc0_0000 == 0xd100_0000 {
        return Ok(DecodedWord::SubtractImmediate {
            source: ((word >> 5) & 0x1f) as u8,
            immediate: ((word >> 10) & 0xfff) as u16,
            destination: register,
        });
    }
    if word == 0xd65f_03c0 {
        return Ok(DecodedWord::Return);
    }
    Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
}

fn validate_decoded(
    kind: TerminalSelectedInstructionKind,
    registers: &[u8],
    decoded: &[DecodedWord],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    let valid = match kind {
        TerminalSelectedInstructionKind::MaterializeI64 { value } => {
            decode_materialization(decoded, registers[0]) == integer_bits(value).ok()
        }
        TerminalSelectedInstructionKind::CopyI64 => {
            decoded
                == [DecodedWord::Copy {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        TerminalSelectedInstructionKind::CompareI64Zero => {
            decoded
                == [DecodedWord::CompareZero {
                    source: registers[0],
                }]
        }
        TerminalSelectedInstructionKind::ExactAddI64 { .. } => {
            decoded
                == [DecodedWord::Add {
                    left: registers[0],
                    right: registers[1],
                    destination: registers[2],
                }]
        }
        TerminalSelectedInstructionKind::ExactAddI64Immediate { immediate, .. } => {
            decoded
                == [DecodedWord::AddImmediate {
                    source: registers[0],
                    immediate: u12(immediate)?,
                    destination: registers[1],
                }]
        }
        TerminalSelectedInstructionKind::ExactSubtractI64 { .. } => {
            decoded
                == [DecodedWord::Subtract {
                    left: registers[0],
                    right: registers[1],
                    destination: registers[2],
                }]
        }
        TerminalSelectedInstructionKind::ExactSubtractI64Immediate { immediate, .. } => {
            decoded
                == [DecodedWord::SubtractImmediate {
                    source: registers[0],
                    immediate: u12(immediate)?,
                    destination: registers[1],
                }]
        }
        TerminalSelectedInstructionKind::ReturnI64
        | TerminalSelectedInstructionKind::ReturnUnit => decoded == [DecodedWord::Return],
        TerminalSelectedInstructionKind::ConditionalBranchNonZero => false,
    };
    if valid {
        Ok(())
    } else {
        Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
    }
}

fn decode_materialization(decoded: &[DecodedWord], register: u8) -> Option<u64> {
    let (mut value, start) = match decoded.first()? {
        DecodedWord::MovZ {
            register: actual,
            shift: 0,
            immediate,
        } if *actual == register => (u64::from(*immediate), 1),
        _ => return None,
    };
    let mut previous_shift = 0;
    for word in &decoded[start..] {
        let DecodedWord::MovK {
            register: actual,
            shift,
            immediate,
        } = word
        else {
            return None;
        };
        if *actual != register || *shift <= previous_shift || *shift > 3 || *immediate == 0 {
            return None;
        }
        previous_shift = *shift;
        let shift = u64::from(*shift) * 16;
        value = (value & !(0xffff_u64 << shift)) | (u64::from(*immediate) << shift);
    }
    Some(value)
}

fn footprint(
    kind: TerminalSelectedInstructionKind,
    operands: &[RegisterViewId],
) -> Aarch64SelectedFormFootprint {
    let (reads, writes, writes_nzcv) = match kind {
        TerminalSelectedInstructionKind::MaterializeI64 { .. } => {
            (vec![], vec![operands[0]], false)
        }
        TerminalSelectedInstructionKind::CopyI64 => (vec![operands[0]], vec![operands[1]], false),
        TerminalSelectedInstructionKind::CompareI64Zero => (vec![operands[0]], vec![], true),
        TerminalSelectedInstructionKind::ExactAddI64 { .. }
        | TerminalSelectedInstructionKind::ExactSubtractI64 { .. } => {
            (vec![operands[0], operands[1]], vec![operands[2]], false)
        }
        TerminalSelectedInstructionKind::ExactAddI64Immediate { .. }
        | TerminalSelectedInstructionKind::ExactSubtractI64Immediate { .. } => {
            (vec![operands[0]], vec![operands[1]], false)
        }
        TerminalSelectedInstructionKind::ReturnI64
        | TerminalSelectedInstructionKind::ReturnUnit => (vec![], vec![], false),
        TerminalSelectedInstructionKind::ConditionalBranchNonZero => (vec![], vec![], false),
    };
    let physical = aarch64_physical_register_model();
    let units = |name: &str| physical.view_named(name).unwrap().units.clone();
    let encoded = if matches!(
        kind,
        TerminalSelectedInstructionKind::ReturnI64 | TerminalSelectedInstructionKind::ReturnUnit
    ) {
        TerminalMachineEncodedEffects {
            external_operand_reads: vec![],
            external_operand_writes: vec![],
            implicit_unit_uses: units("x30"),
            implicit_unit_defs: units("pc"),
            implicit_unit_clobbers: vec![],
            memory: TerminalMachineEncodedMemoryEffect::NoneV1,
            stack: TerminalMachineEncodedStackEffect::UnchangedV1,
            trap: TerminalMachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: TerminalMachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                target: physical.view_named("x30").unwrap().id,
            },
        }
    } else if matches!(
        kind,
        TerminalSelectedInstructionKind::ConditionalBranchNonZero
    ) {
        let mut uses = units("nzcv");
        uses.extend(units("pc"));
        uses.sort_unstable();
        uses.dedup();
        TerminalMachineEncodedEffects {
            external_operand_reads: vec![],
            external_operand_writes: vec![],
            implicit_unit_uses: uses,
            implicit_unit_defs: units("pc"),
            implicit_unit_clobbers: vec![],
            memory: TerminalMachineEncodedMemoryEffect::NoneV1,
            stack: TerminalMachineEncodedStackEffect::UnchangedV1,
            trap: TerminalMachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: TerminalMachineEncodedControlEffect::ConditionalRelativeBranchV1,
        }
    } else {
        let mut effects = TerminalMachineEncodedEffects::fallthrough_v1(
            match kind {
                TerminalSelectedInstructionKind::MaterializeI64 { .. } => vec![],
                TerminalSelectedInstructionKind::CopyI64
                | TerminalSelectedInstructionKind::CompareI64Zero
                | TerminalSelectedInstructionKind::ExactAddI64Immediate { .. }
                | TerminalSelectedInstructionKind::ExactSubtractI64Immediate { .. } => vec![0],
                TerminalSelectedInstructionKind::ExactAddI64 { .. }
                | TerminalSelectedInstructionKind::ExactSubtractI64 { .. } => vec![0, 1],
                _ => unreachable!("control forms handled separately"),
            },
            match kind {
                TerminalSelectedInstructionKind::MaterializeI64 { .. } => vec![0],
                TerminalSelectedInstructionKind::CopyI64
                | TerminalSelectedInstructionKind::ExactAddI64Immediate { .. }
                | TerminalSelectedInstructionKind::ExactSubtractI64Immediate { .. } => vec![1],
                TerminalSelectedInstructionKind::ExactAddI64 { .. }
                | TerminalSelectedInstructionKind::ExactSubtractI64 { .. } => vec![2],
                TerminalSelectedInstructionKind::CompareI64Zero => vec![],
                _ => unreachable!("control forms handled separately"),
            },
        );
        if writes_nzcv {
            effects.implicit_unit_defs = units("nzcv");
        }
        effects
    };
    Aarch64SelectedFormFootprint {
        register_reads: reads,
        register_writes: writes,
        writes_nzcv,
        encoded,
    }
}

#[cfg(test)]
mod tests {
    use omega_register_model::validate_physical_register_model;
    use omega_terminal_selected_instructions::TerminalMachineAlternativeFamily;
    use psi_core::{IntegerValue, ObligationId};

    use super::*;

    fn alternative(family: TerminalMachineAlternativeFamily) -> TerminalMachineAlternativeKey {
        TerminalMachineAlternativeKey { family, variant: 0 }
    }

    #[test]
    fn zero_seeded_materialization_is_canonical_and_decoded_independently() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let x9 = physical.model().view_named("x9").unwrap().id;
        for (value, byte_count) in [
            (IntegerValue::Unsigned(0), 4),
            (IntegerValue::Unsigned(u64::MAX as u128), 16),
            (IntegerValue::Unsigned(0x1234_0000_5678_0000), 12),
            (IntegerValue::Unsigned(0x1234_5678_9abc_def0), 16),
        ] {
            let kind = TerminalSelectedInstructionKind::MaterializeI64 { value };
            let encoded = encode_aarch64_terminal_selected_form(
                &physical,
                kind,
                alternative(TerminalMachineAlternativeFamily::MaterializeI64),
                &[x9],
            )
            .unwrap();
            assert_eq!(encoded.bytes().len(), byte_count);
            let mut corrupted = encoded.bytes().to_vec();
            corrupted[0] ^= 0x20;
            assert!(
                validate_aarch64_terminal_selected_form_encoding(
                    &physical,
                    kind,
                    alternative(TerminalMachineAlternativeFamily::MaterializeI64),
                    &[x9],
                    &corrupted,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn scalar_forms_report_exact_decoded_footprints() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let views = ["x3", "x4", "x5"].map(|name| physical.model().view_named(name).unwrap().id);
        let fact = omega_optimization_core::AcceptedObligationFactIdentity::from_bytes([7; 32]);
        let cases = [
            (
                TerminalSelectedInstructionKind::CopyI64,
                TerminalMachineAlternativeFamily::CopyI64,
                2,
            ),
            (
                TerminalSelectedInstructionKind::CompareI64Zero,
                TerminalMachineAlternativeFamily::CompareI64Zero,
                1,
            ),
            (
                TerminalSelectedInstructionKind::ExactAddI64 {
                    obligation: ObligationId::new(1).unwrap(),
                    accepted_fact: fact,
                },
                TerminalMachineAlternativeFamily::ExactAddI64,
                3,
            ),
            (
                TerminalSelectedInstructionKind::ExactAddI64Immediate {
                    immediate: IntegerValue::Unsigned(4095),
                    obligation: ObligationId::new(2).unwrap(),
                    accepted_fact: fact,
                },
                TerminalMachineAlternativeFamily::ExactAddI64Immediate,
                2,
            ),
            (
                TerminalSelectedInstructionKind::ExactSubtractI64 {
                    obligation: ObligationId::new(3).unwrap(),
                    accepted_fact: fact,
                },
                TerminalMachineAlternativeFamily::ExactSubtractI64,
                3,
            ),
            (
                TerminalSelectedInstructionKind::ExactSubtractI64Immediate {
                    immediate: IntegerValue::Unsigned(5),
                    obligation: ObligationId::new(4).unwrap(),
                    accepted_fact: fact,
                },
                TerminalMachineAlternativeFamily::ExactSubtractI64Immediate,
                2,
            ),
        ];
        for (kind, family, count) in cases {
            let encoded = encode_aarch64_terminal_selected_form(
                &physical,
                kind,
                alternative(family),
                &views[..count],
            )
            .unwrap();
            assert_eq!(encoded.bytes().len(), 4);
            if matches!(
                kind,
                TerminalSelectedInstructionKind::ExactSubtractI64Immediate { .. }
            ) {
                assert_eq!(encoded.bytes(), [0x64, 0x14, 0x00, 0xd1]);
                assert!(!encoded.footprint().writes_nzcv);
                assert!(encoded.footprint().encoded.implicit_unit_defs.is_empty());
            }
        }
    }

    #[test]
    fn ret_x30_is_exact_and_separates_abi_result_custody_from_encoded_effects() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let x0 = physical.model().view_named("x0").unwrap().id;
        let x1 = physical.model().view_named("x1").unwrap().id;
        let x30 = physical.model().view_named("x30").unwrap();
        let pc = physical.model().view_named("pc").unwrap();
        let kind = TerminalSelectedInstructionKind::ReturnI64;
        let alternative = alternative(TerminalMachineAlternativeFamily::ReturnI64);
        let encoded =
            encode_aarch64_terminal_selected_form(&physical, kind, alternative, &[x0]).unwrap();

        assert_eq!(encoded.bytes(), [0xc0, 0x03, 0x5f, 0xd6]);
        assert!(encoded.footprint().register_reads.is_empty());
        assert!(encoded.footprint().register_writes.is_empty());
        assert_eq!(encoded.footprint().encoded.external_operand_reads, []);
        assert_eq!(encoded.footprint().encoded.external_operand_writes, []);
        assert_eq!(encoded.footprint().encoded.implicit_unit_uses, x30.units);
        assert_eq!(encoded.footprint().encoded.implicit_unit_defs, pc.units);
        assert_eq!(
            encoded.footprint().encoded.memory,
            TerminalMachineEncodedMemoryEffect::NoneV1
        );
        assert_eq!(
            encoded.footprint().encoded.stack,
            TerminalMachineEncodedStackEffect::UnchangedV1
        );
        assert_eq!(
            encoded.footprint().encoded.trap,
            TerminalMachineEncodedTrapBehavior::MayArchitecturalFaultV1
        );
        assert_eq!(
            encoded.footprint().encoded.control,
            TerminalMachineEncodedControlEffect::ReturnIndirectRegisterV1 { target: x30.id }
        );
        assert!(
            encode_aarch64_terminal_selected_form(&physical, kind, alternative, &[x1]).is_err()
        );
        assert!(
            validate_aarch64_terminal_selected_form_encoding(
                &physical,
                kind,
                alternative,
                &[x0],
                &0xd65f_03a0_u32.to_le_bytes()
            )
            .is_err()
        );
        assert!(
            validate_aarch64_terminal_selected_form_encoding(
                &physical,
                kind,
                alternative,
                &[x0],
                &[0xc0, 0x03, 0x5f, 0xd6, 0xc0, 0x03, 0x5f, 0xd6]
            )
            .is_err()
        );
    }

    #[test]
    fn unit_return_is_a_distinct_zero_operand_ret_x30() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let kind = TerminalSelectedInstructionKind::ReturnUnit;
        let return_alternative = alternative(TerminalMachineAlternativeFamily::ReturnUnit);
        let encoded =
            encode_aarch64_terminal_selected_form(&physical, kind, return_alternative, &[])
                .unwrap();

        assert_eq!(encoded.bytes(), [0xc0, 0x03, 0x5f, 0xd6]);
        assert!(encoded.footprint().register_reads.is_empty());
        assert!(encoded.footprint().register_writes.is_empty());
        assert!(matches!(
            encoded.footprint().encoded.control,
            TerminalMachineEncodedControlEffect::ReturnIndirectRegisterV1 { .. }
        ));
        assert!(
            encode_aarch64_terminal_selected_form(
                &physical,
                kind,
                alternative(TerminalMachineAlternativeFamily::ReturnI64),
                &[]
            )
            .is_err()
        );
    }

    #[test]
    fn nonzero_branch_has_exact_instruction_relative_imm19_and_effects() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let alternative = alternative(TerminalMachineAlternativeFamily::ConditionalBranchNonZero);
        for displacement in [-1_048_576, -4, 0, 4, 1_048_572] {
            let encoded = encode_aarch64_terminal_selected_nonzero_branch_form(
                &physical,
                alternative,
                displacement,
            )
            .unwrap();
            assert_eq!(encoded.bytes().len(), 4);
            assert_eq!(encoded.bytes()[0] & 0x1f, 1);
            assert!(encoded.footprint().register_reads.is_empty());
            assert!(encoded.footprint().register_writes.is_empty());
            assert_eq!(
                encoded.footprint().encoded.control,
                TerminalMachineEncodedControlEffect::ConditionalRelativeBranchV1
            );
        }
        for displacement in [-1_048_580, 1_048_576, 2] {
            assert!(
                encode_aarch64_terminal_selected_nonzero_branch_form(
                    &physical,
                    alternative,
                    displacement
                )
                .is_err()
            );
        }
        assert!(
            validate_aarch64_terminal_selected_nonzero_branch_form(
                &physical,
                alternative,
                0,
                &0x5400_0000_u32.to_le_bytes()
            )
            .is_err()
        );
        assert!(
            validate_aarch64_terminal_selected_nonzero_branch_form(
                &physical,
                alternative,
                0,
                &[1, 0, 0, 0, 0]
            )
            .is_err()
        );
    }

    #[test]
    fn fused_cbnz_is_exact_rejects_nearby_opcodes_and_does_not_read_nzcv() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let x0 = physical.model().view_named("x0").unwrap();
        let x30 = physical.model().view_named("x30").unwrap();
        let pc = physical.model().view_named("pc").unwrap();
        let nzcv = physical.model().view_named("nzcv").unwrap();

        for (source, register) in [(x0.id, 0_u8), (x30.id, 30_u8)] {
            for displacement in [-1_048_576, -4, 0, 4, 1_048_572] {
                let encoded =
                    encode_aarch64_terminal_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                        &physical,
                        source,
                        displacement,
                    )
                    .unwrap();
                assert_eq!(encoded.bytes().len(), 4);
                assert_eq!(encoded.bytes()[0] & 0x1f, register);
                assert_eq!(encoded.footprint().register_reads, [source]);
                assert!(encoded.footprint().register_writes.is_empty());
                assert!(!encoded.footprint().writes_nzcv);
                assert_eq!(encoded.footprint().encoded.external_operand_reads, []);
                assert_eq!(encoded.footprint().encoded.implicit_unit_uses, pc.units);
                assert_eq!(encoded.footprint().encoded.implicit_unit_defs, pc.units);
                assert!(
                    encoded
                        .footprint()
                        .encoded
                        .implicit_unit_uses
                        .iter()
                        .all(|unit| !nzcv.units.contains(unit))
                );
            }
        }

        for displacement in [-1_048_580, 1_048_576, 2] {
            assert!(
                encode_aarch64_terminal_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                    &physical,
                    x0.id,
                    displacement,
                )
                .is_err()
            );
        }
        for word in [
            0xb400_0000_u32, // 64-bit CBZ
            0x3500_0000_u32, // 32-bit CBNZ
            0xb500_001e_u32, // wrong source register
        ] {
            assert!(
                validate_aarch64_terminal_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                    &physical,
                    x0.id,
                    0,
                    &word.to_le_bytes(),
                )
                .is_err()
            );
        }
        assert!(
            validate_aarch64_terminal_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                &physical,
                x0.id,
                0,
                &[0, 0, 0, 0, 0],
            )
            .is_err()
        );
    }
}
