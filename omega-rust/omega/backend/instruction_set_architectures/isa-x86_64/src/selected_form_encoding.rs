#[cfg(test)]
mod byte_view_address_tests;
#[cfg(test)]
mod integer_normalization_tests;

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstructionKind,
};
use semantic_vocabulary::IntegerValue;

use crate::x86_64_physical_register_model;

mod float_bits;
pub(crate) mod hosted_exit_process;
pub(crate) mod hosted_read_byte;
pub(crate) mod hosted_write_byte;
mod jump;
pub use hosted_write_byte::{
    encode_x86_64_selected_hosted_write_byte_form, validate_x86_64_selected_hosted_write_byte_form,
};
mod memory;
mod scalar_call;
pub use jump::*;
pub use memory::*;

pub use scalar_call::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64SelectedFormFootprint {
    pub register_reads: Vec<RegisterViewId>,
    pub register_writes: Vec<RegisterViewId>,
    pub writes_rflags: bool,
    pub encoded: MachineEncodedEffects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64SelectedFormEncoding {
    bytes: Vec<u8>,
    footprint: X86_64SelectedFormFootprint,
}

impl ValidatedX86_64SelectedFormEncoding {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn footprint(&self) -> &X86_64SelectedFormFootprint {
        &self.footprint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86_64SelectedFormEncodingError {
    NonCanonicalPhysicalModel,
    LayoutDependentForm,
    AlternativeMismatch,
    OperandCountMismatch,
    UnknownOrNonGpr64View(RegisterViewId),
    IntegerOutsideI64Bits,
    ImmediateOutsideU12,
    BranchDisplacementOutsideI32,
    MalformedEncoding,
    EncodedFormMismatch,
    BranchDisplacementOutsideI8,
}

impl std::fmt::Display for X86_64SelectedFormEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid x86-64 selected-form encoding: {self:?}")
    }
}

impl std::error::Error for X86_64SelectedFormEncodingError {}

/// Encode the canonical layout-resolved realization of
/// `ConditionalBranchNonZero`. The displacement is measured from the end of
/// this six-byte near branch, as required by x86-64.
pub fn encode_x86_64_selected_nonzero_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchNonZero,
    )?;
    let displacement = i32::try_from(byte_displacement_from_instruction_end)
        .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)?;
    let mut bytes = vec![0x0f, 0x85];
    bytes.extend(displacement.to_le_bytes());
    validate_x86_64_selected_nonzero_branch_form(
        physical,
        alternative,
        byte_displacement_from_instruction_end,
        &bytes,
    )
}

pub fn validate_x86_64_selected_nonzero_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchNonZero,
    )?;
    let expected = i32::try_from(byte_displacement_from_instruction_end)
        .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)?;
    let actual = bytes
        .get(2..6)
        .filter(|_| bytes.len() == 6 && bytes[..2] == [0x0f, 0x85])
        .and_then(|bytes| bytes.try_into().ok())
        .map(i32::from_le_bytes)
        .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    if actual != expected {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(
            SelectedInstructionKind::ConditionalBranchNonZero,
            alternative,
            &[],
        ),
    })
}

/// Encode the canonical short layout-resolved realization of
/// `ConditionalBranchNonZero`. The signed byte displacement is measured from
/// the end of this two-byte instruction, as required by x86-64 `JNE rel8`.
pub fn encode_x86_64_selected_short_nonzero_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchNonZero,
    )?;
    let displacement = i8::try_from(byte_displacement_from_instruction_end)
        .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)?;
    validate_x86_64_selected_short_nonzero_branch_form(
        physical,
        alternative,
        byte_displacement_from_instruction_end,
        &[0x75, displacement as u8],
    )
}

/// Validate exactly one canonical x86-64 `JNE rel8` instruction. Near-branch
/// opcodes, prefixes, suffixes, and trailing bytes are not alternate encodings
/// of this selected short form.
pub fn validate_x86_64_selected_short_nonzero_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    validate_branch_request(
        physical,
        alternative,
        MachineAlternativeFamily::ConditionalBranchNonZero,
    )?;
    let expected = i8::try_from(byte_displacement_from_instruction_end)
        .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)?;
    let actual = bytes
        .get(1)
        .copied()
        .filter(|_| bytes.len() == 2 && bytes[0] == 0x75)
        .map(|displacement| displacement as i8)
        .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    if actual != expected {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(
            SelectedInstructionKind::ConditionalBranchNonZero,
            alternative,
            &[],
        ),
    })
}

/// Encode the selected layout-resolved unsigned-less-than branch. Alternative
/// zero is the six-byte `JB rel32`; alternative one is the two-byte `JB rel8`.
/// In both cases the displacement is measured from the instruction end.
pub fn encode_x86_64_selected_u64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    validate_less_than_branch_request(physical, alternative)?;
    let bytes = match alternative.variant {
        0 => {
            let displacement = i32::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)?;
            let mut bytes = vec![0x0f, 0x82];
            bytes.extend(displacement.to_le_bytes());
            bytes
        }
        1 => {
            let displacement = i8::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)?;
            vec![0x72, displacement as u8]
        }
        _ => unreachable!("less-than branch request validated variant"),
    };
    validate_x86_64_selected_u64_less_than_branch_form(
        physical,
        alternative,
        byte_displacement_from_instruction_end,
        &bytes,
    )
}

/// Independently decode the exact `JB` form named by the alternative.
pub fn validate_x86_64_selected_u64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    validate_less_than_branch_request(physical, alternative)?;
    match alternative.variant {
        0 => {
            let expected = i32::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)?;
            let actual = bytes
                .get(2..6)
                .filter(|_| bytes.len() == 6 && bytes[..2] == [0x0f, 0x82])
                .and_then(|bytes| bytes.try_into().ok())
                .map(i32::from_le_bytes)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
            if actual != expected {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
        }
        1 => {
            let expected = i8::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)?;
            let actual = bytes
                .get(1)
                .copied()
                .filter(|_| bytes.len() == 2 && bytes[0] == 0x72)
                .map(|displacement| displacement as i8)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
            if actual != expected {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
        }
        _ => unreachable!("less-than branch request validated variant"),
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(
            SelectedInstructionKind::ConditionalBranchU64LessThan,
            alternative,
            &[],
        ),
    })
}

/// Encode the selected layout-resolved signed-less-than branch. Alternative
/// zero is the six-byte `JL rel32`; alternative one is the two-byte `JL rel8`.
/// In both cases the displacement is measured from the instruction end.
pub fn encode_x86_64_selected_i64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    validate_i64_less_than_branch_request(physical, alternative)?;
    let bytes = match alternative.variant {
        0 => {
            let displacement = i32::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)?;
            let mut bytes = vec![0x0f, 0x8c];
            bytes.extend(displacement.to_le_bytes());
            bytes
        }
        1 => {
            let displacement = i8::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)?;
            vec![0x7c, displacement as u8]
        }
        _ => unreachable!("signed less-than branch request validated variant"),
    };
    validate_x86_64_selected_i64_less_than_branch_form(
        physical,
        alternative,
        byte_displacement_from_instruction_end,
        &bytes,
    )
}

/// Independently decode the exact `JL` form named by the alternative.
pub fn validate_x86_64_selected_i64_less_than_branch_form(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement_from_instruction_end: i64,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    validate_i64_less_than_branch_request(physical, alternative)?;
    match alternative.variant {
        0 => {
            let expected = i32::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)?;
            let actual = bytes
                .get(2..6)
                .filter(|_| bytes.len() == 6 && bytes[..2] == [0x0f, 0x8c])
                .and_then(|bytes| bytes.try_into().ok())
                .map(i32::from_le_bytes)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
            if actual != expected {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
        }
        1 => {
            let expected = i8::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)?;
            let actual = bytes
                .get(1)
                .copied()
                .filter(|_| bytes.len() == 2 && bytes[0] == 0x7c)
                .map(|displacement| displacement as i8)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
            if actual != expected {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
        }
        _ => unreachable!("signed less-than branch request validated variant"),
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(
            SelectedInstructionKind::ConditionalBranchI64LessThan,
            alternative,
            &[],
        ),
    })
}

fn validate_i64_less_than_branch_request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
) -> Result<(), X86_64SelectedFormEncodingError> {
    if physical.model() != &x86_64_physical_register_model() {
        return Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    if alternative.family != MachineAlternativeFamily::ConditionalBranchI64LessThan
        || alternative.variant > 1
    {
        return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
    }
    Ok(())
}

fn validate_less_than_branch_request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
) -> Result<(), X86_64SelectedFormEncodingError> {
    if physical.model() != &x86_64_physical_register_model() {
        return Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    if alternative.family != MachineAlternativeFamily::ConditionalBranchU64LessThan
        || alternative.variant > 1
    {
        return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
    }
    Ok(())
}

fn validate_branch_request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    family: MachineAlternativeFamily,
) -> Result<(), X86_64SelectedFormEncodingError> {
    if physical.model() != &x86_64_physical_register_model() {
        return Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    if alternative != (MachineAlternativeKey { family, variant: 0 }) {
        return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
    }
    Ok(())
}

pub fn encode_x86_64_selected_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    if float_bits::is_transfer(kind) {
        return float_bits::encode(physical, kind, alternative, operands);
    }
    validate_request(physical, kind, alternative, operands)?;
    let registers = resolve_registers(physical, operands)?;
    validate_return_home(kind, &registers)?;
    validate_alias_partition(kind, alternative, &registers)?;
    let bytes = encode_unchecked(kind, alternative, &registers)?;
    validate_x86_64_selected_form_encoding(physical, kind, alternative, operands, &bytes)
}

pub fn validate_x86_64_selected_form_encoding(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    if float_bits::is_transfer(kind) {
        return float_bits::validate(physical, kind, alternative, operands, bytes);
    }
    validate_request(physical, kind, alternative, operands)?;
    let registers = resolve_registers(physical, operands)?;
    validate_return_home(kind, &registers)?;
    validate_alias_partition(kind, alternative, &registers)?;
    let decoded = decode_all(bytes)?;
    validate_decoded(kind, alternative, &registers, &decoded)?;
    let canonical = encode_unchecked(kind, alternative, &registers)?;
    if bytes != canonical {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(kind, alternative, operands),
    })
}

fn validate_request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<(), X86_64SelectedFormEncodingError> {
    if physical.model() != &x86_64_physical_register_model() {
        return Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    let (family, count, variants) = family_and_operand_count(kind)?;
    if alternative.family != family || !variants.contains(&alternative.variant) {
        return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
    }
    if operands.len() != count {
        return Err(X86_64SelectedFormEncodingError::OperandCountMismatch);
    }
    Ok(())
}

fn family_and_operand_count(
    kind: SelectedInstructionKind,
) -> Result<
    (
        MachineAlternativeFamily,
        usize,
        std::ops::RangeInclusive<u32>,
    ),
    X86_64SelectedFormEncodingError,
> {
    Ok(match kind {
        SelectedInstructionKind::CompareI64Zero => {
            (MachineAlternativeFamily::CompareI64Zero, 1, 0..=0)
        }
        SelectedInstructionKind::CompareI64 => (MachineAlternativeFamily::CompareI64, 2, 0..=0),
        SelectedInstructionKind::MaterializeI64 { .. } => {
            (MachineAlternativeFamily::MaterializeI64, 1, 0..=0)
        }
        SelectedInstructionKind::CopyI64 => (MachineAlternativeFamily::CopyI64, 2, 0..=0),
        SelectedInstructionKind::ZeroExtendU8 => (MachineAlternativeFamily::ZeroExtendU8, 2, 0..=0),
        SelectedInstructionKind::ZeroExtendU16 => {
            (MachineAlternativeFamily::ZeroExtendU16, 2, 0..=0)
        }
        SelectedInstructionKind::SignExtendI8 => (MachineAlternativeFamily::SignExtendI8, 2, 0..=0),
        SelectedInstructionKind::SignExtendI16 => {
            (MachineAlternativeFamily::SignExtendI16, 2, 0..=0)
        }
        SelectedInstructionKind::SignExtendI32 => {
            (MachineAlternativeFamily::SignExtendI32, 2, 0..=0)
        }
        SelectedInstructionKind::ZeroExtendU32 => {
            (MachineAlternativeFamily::ZeroExtendU32, 2, 0..=0)
        }
        SelectedInstructionKind::ByteViewAddress => {
            (MachineAlternativeFamily::ByteViewAddress, 3, 0..=0)
        }
        SelectedInstructionKind::ExactAddI64 { .. } => {
            (MachineAlternativeFamily::ExactAddI64, 3, 0..=0)
        }
        SelectedInstructionKind::ExactSubtractI64 { .. } => {
            (MachineAlternativeFamily::ExactSubtractI64, 3, 0..=3)
        }
        SelectedInstructionKind::ExactAddI64Immediate { .. } => {
            (MachineAlternativeFamily::ExactAddI64Immediate, 2, 0..=0)
        }
        SelectedInstructionKind::ExactSubtractI64Immediate { .. } => (
            MachineAlternativeFamily::ExactSubtractI64Immediate,
            2,
            0..=0,
        ),
        SelectedInstructionKind::ReturnI64 => (MachineAlternativeFamily::ReturnI64, 1, 0..=0),
        SelectedInstructionKind::ReturnAggregate { fragment_count } => {
            if !(1..=2).contains(&fragment_count) {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            (
                MachineAlternativeFamily::ReturnAggregate,
                usize::from(fragment_count),
                0..=0,
            )
        }
        SelectedInstructionKind::ReturnUnit => (MachineAlternativeFamily::ReturnUnit, 0, 0..=0),
        SelectedInstructionKind::ConditionalBranchNonZero => {
            return Err(X86_64SelectedFormEncodingError::LayoutDependentForm);
        }
        SelectedInstructionKind::ConditionalBranchU64LessThan => {
            return Err(X86_64SelectedFormEncodingError::LayoutDependentForm);
        }
        SelectedInstructionKind::ConditionalBranchI64LessThan => {
            return Err(X86_64SelectedFormEncodingError::LayoutDependentForm);
        }
        SelectedInstructionKind::Float32ToBits
        | SelectedInstructionKind::Float64ToBits
        | SelectedInstructionKind::BitsToFloat32
        | SelectedInstructionKind::BitsToFloat64
        | SelectedInstructionKind::Load8Indexed
        | SelectedInstructionKind::Load8 { .. }
        | SelectedInstructionKind::Load16 { .. }
        | SelectedInstructionKind::Load32 { .. }
        | SelectedInstructionKind::Load64 { .. }
        | SelectedInstructionKind::Store { .. }
        | SelectedInstructionKind::AddressOffset { .. }
        | SelectedInstructionKind::Store64 { .. }
        | SelectedInstructionKind::HostedExitProcessI32
        | SelectedInstructionKind::HostedReadByte { .. }
        | SelectedInstructionKind::HostedWriteByteI32 { .. }
        | SelectedInstructionKind::FrameAddress { .. }
        | SelectedInstructionKind::CallUnit { .. }
        | SelectedInstructionKind::Jump
        | SelectedInstructionKind::CallAggregate { .. }
        | SelectedInstructionKind::CallI64 { .. } => {
            return Err(X86_64SelectedFormEncodingError::LayoutDependentForm);
        }
    })
}

fn validate_return_home(
    kind: SelectedInstructionKind,
    registers: &[u8],
) -> Result<(), X86_64SelectedFormEncodingError> {
    if matches!(kind, SelectedInstructionKind::ReturnI64) && registers != [0] {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    if let SelectedInstructionKind::ReturnAggregate { fragment_count } = kind
        && (!(1..=2).contains(&fragment_count)
            || registers != &[0, 2][..usize::from(fragment_count)])
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(())
}

fn resolve_registers(
    physical: &ValidatedPhysicalRegisterModel,
    operands: &[RegisterViewId],
) -> Result<Vec<u8>, X86_64SelectedFormEncodingError> {
    const NAMES: [&str; 16] = [
        "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12",
        "r13", "r14", "r15",
    ];
    operands
        .iter()
        .map(|id| {
            let (code, view) = NAMES
                .iter()
                .enumerate()
                .find_map(|(code, name)| {
                    physical
                        .model()
                        .view_named(name)
                        .filter(|view| view.id == *id)
                        .map(|view| (code as u8, view))
                })
                .ok_or(X86_64SelectedFormEncodingError::UnknownOrNonGpr64View(*id))?;
            if code == 4 || view.bits != 64 || !view.allocatable {
                return Err(X86_64SelectedFormEncodingError::UnknownOrNonGpr64View(*id));
            }
            Ok(code)
        })
        .collect()
}

fn validate_alias_partition(
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    registers: &[u8],
) -> Result<(), X86_64SelectedFormEncodingError> {
    if !matches!(kind, SelectedInstructionKind::ExactSubtractI64 { .. }) {
        return Ok(());
    }
    let [left, right, result] = registers else {
        return Err(X86_64SelectedFormEncodingError::OperandCountMismatch);
    };
    let expected = match (result == left, result == right) {
        (true, true) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (false, false) => 3,
    };
    if alternative.variant != expected {
        return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
    }
    Ok(())
}

fn integer_bits(value: IntegerValue) -> Result<u64, X86_64SelectedFormEncodingError> {
    match value {
        IntegerValue::Signed(value) => i64::try_from(value)
            .map(|value| value as u64)
            .map_err(|_| X86_64SelectedFormEncodingError::IntegerOutsideI64Bits),
        IntegerValue::Unsigned(value) => {
            u64::try_from(value).map_err(|_| X86_64SelectedFormEncodingError::IntegerOutsideI64Bits)
        }
    }
}

fn u12(value: IntegerValue) -> Result<u32, X86_64SelectedFormEncodingError> {
    match value {
        IntegerValue::Unsigned(value) if value <= 4095 => Ok(value as u32),
        _ => Err(X86_64SelectedFormEncodingError::ImmediateOutsideU12),
    }
}

fn rex(register: u8, index: u8, base: u8) -> u8 {
    0x48 | ((register >> 3) << 2) | ((index >> 3) << 1) | (base >> 3)
}

fn modrm(mode: u8, register: u8, rm: u8) -> u8 {
    (mode << 6) | ((register & 7) << 3) | (rm & 7)
}

fn append_register_binary(bytes: &mut Vec<u8>, opcode: u8, source: u8, destination: u8) {
    bytes.extend([
        rex(source, 0, destination),
        opcode,
        modrm(3, source, destination),
    ]);
}

fn append_lea_register(bytes: &mut Vec<u8>, left: u8, right: u8, destination: u8) {
    let (base, index) = if left & 7 == 5 && right & 7 != 5 {
        (right, left)
    } else {
        (left, right)
    };
    let needs_zero_displacement = base & 7 == 5;
    bytes.extend([
        rex(destination, index, base),
        0x8d,
        modrm(u8::from(needs_zero_displacement), destination, 4),
        ((index & 7) << 3) | (base & 7),
    ]);
    if needs_zero_displacement {
        bytes.push(0);
    }
}

fn append_lea_immediate(bytes: &mut Vec<u8>, base: u8, destination: u8, displacement: i32) {
    let use_disp8 = i8::try_from(displacement).is_ok();
    let uses_sib = base & 7 == 4;
    bytes.extend([
        rex(destination, 0, base),
        0x8d,
        modrm(
            if use_disp8 { 1 } else { 2 },
            destination,
            if uses_sib { 4 } else { base },
        ),
    ]);
    if uses_sib {
        bytes.push(0x20 | (base & 7));
    }
    if use_disp8 {
        bytes.push(displacement as i8 as u8);
    } else {
        bytes.extend(displacement.to_le_bytes());
    }
}

fn encode_unchecked(
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    registers: &[u8],
) -> Result<Vec<u8>, X86_64SelectedFormEncodingError> {
    let mut bytes = Vec::new();
    match kind {
        SelectedInstructionKind::MaterializeI64 { value } => {
            bytes.extend([0x48 | (registers[0] >> 3), 0xb8 | (registers[0] & 7)]);
            bytes.extend(integer_bits(value)?.to_le_bytes());
        }
        SelectedInstructionKind::ZeroExtendU8 => {
            bytes.extend([
                0x40 | ((registers[1] >> 3) << 2) | (registers[0] >> 3),
                0x0f,
                0xb6,
                0xc0 | ((registers[1] & 7) << 3) | (registers[0] & 7),
            ]);
        }
        SelectedInstructionKind::ZeroExtendU16 => {
            bytes.extend([
                0x40 | ((registers[1] >> 3) << 2) | (registers[0] >> 3),
                0x0f,
                0xb7,
                0xc0 | ((registers[1] & 7) << 3) | (registers[0] & 7),
            ]);
        }
        SelectedInstructionKind::SignExtendI8 => {
            bytes.extend([
                0x48 | ((registers[1] >> 3) << 2) | (registers[0] >> 3),
                0x0f,
                0xbe,
                0xc0 | ((registers[1] & 7) << 3) | (registers[0] & 7),
            ]);
        }
        SelectedInstructionKind::SignExtendI16 => {
            bytes.extend([
                0x48 | ((registers[1] >> 3) << 2) | (registers[0] >> 3),
                0x0f,
                0xbf,
                0xc0 | ((registers[1] & 7) << 3) | (registers[0] & 7),
            ]);
        }
        SelectedInstructionKind::SignExtendI32 => {
            bytes.extend([
                0x48 | ((registers[1] >> 3) << 2) | (registers[0] >> 3),
                0x63,
                0xc0 | ((registers[1] & 7) << 3) | (registers[0] & 7),
            ]);
        }
        SelectedInstructionKind::ZeroExtendU32 => {
            bytes.extend([
                0x40 | ((registers[0] >> 3) << 2) | (registers[1] >> 3),
                0x89,
                0xc0 | ((registers[0] & 7) << 3) | (registers[1] & 7),
            ]);
        }
        SelectedInstructionKind::CopyI64 => {
            append_register_binary(&mut bytes, 0x89, registers[0], registers[1]);
        }
        SelectedInstructionKind::CompareI64Zero => {
            append_register_binary(&mut bytes, 0x85, registers[0], registers[0]);
        }
        SelectedInstructionKind::CompareI64 => {
            append_register_binary(&mut bytes, 0x39, registers[1], registers[0]);
        }
        SelectedInstructionKind::ByteViewAddress | SelectedInstructionKind::ExactAddI64 { .. } => {
            append_lea_register(&mut bytes, registers[0], registers[1], registers[2]);
        }
        SelectedInstructionKind::ExactAddI64Immediate { immediate, .. } => {
            append_lea_immediate(
                &mut bytes,
                registers[0],
                registers[1],
                i32::try_from(u12(immediate)?).expect("u12 fits i32"),
            );
        }
        SelectedInstructionKind::ExactSubtractI64Immediate { immediate, .. } => {
            append_lea_immediate(
                &mut bytes,
                registers[0],
                registers[1],
                -i32::try_from(u12(immediate)?).expect("u12 fits i32"),
            );
        }
        SelectedInstructionKind::ExactSubtractI64 { .. } => match alternative.variant {
            0 => append_register_binary(&mut bytes, 0x31, registers[2], registers[2]),
            1 => append_register_binary(&mut bytes, 0x29, registers[1], registers[2]),
            2 => {
                bytes.extend([rex(0, 0, registers[2]), 0xf7, modrm(3, 3, registers[2])]);
                append_register_binary(&mut bytes, 0x01, registers[0], registers[2]);
            }
            3 => {
                append_register_binary(&mut bytes, 0x89, registers[0], registers[2]);
                append_register_binary(&mut bytes, 0x29, registers[1], registers[2]);
            }
            _ => return Err(X86_64SelectedFormEncodingError::AlternativeMismatch),
        },
        SelectedInstructionKind::ReturnI64
        | SelectedInstructionKind::ReturnAggregate { .. }
        | SelectedInstructionKind::ReturnUnit => bytes.push(0xc3),
        SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan
        | SelectedInstructionKind::Jump => {
            return Err(X86_64SelectedFormEncodingError::LayoutDependentForm);
        }
        SelectedInstructionKind::Float32ToBits
        | SelectedInstructionKind::Float64ToBits
        | SelectedInstructionKind::BitsToFloat32
        | SelectedInstructionKind::BitsToFloat64
        | SelectedInstructionKind::Load8Indexed
        | SelectedInstructionKind::Load8 { .. }
        | SelectedInstructionKind::Load16 { .. }
        | SelectedInstructionKind::Load32 { .. }
        | SelectedInstructionKind::Load64 { .. }
        | SelectedInstructionKind::Store { .. }
        | SelectedInstructionKind::AddressOffset { .. }
        | SelectedInstructionKind::Store64 { .. }
        | SelectedInstructionKind::HostedExitProcessI32
        | SelectedInstructionKind::HostedReadByte { .. }
        | SelectedInstructionKind::HostedWriteByteI32 { .. }
        | SelectedInstructionKind::FrameAddress { .. }
        | SelectedInstructionKind::CallUnit { .. }
        | SelectedInstructionKind::CallAggregate { .. }
        | SelectedInstructionKind::CallI64 { .. } => {
            return Err(X86_64SelectedFormEncodingError::LayoutDependentForm);
        }
    }
    Ok(bytes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecodedInstruction {
    ZeroExtendU16 {
        source: u8,
        destination: u8,
    },
    SignExtendI8 {
        source: u8,
        destination: u8,
    },
    SignExtendI16 {
        source: u8,
        destination: u8,
    },
    SignExtendI32 {
        source: u8,
        destination: u8,
    },
    ZeroExtendU8 {
        source: u8,
        destination: u8,
    },
    ZeroExtendU32 {
        source: u8,
        destination: u8,
    },
    Materialize {
        destination: u8,
        value: u64,
    },
    Move {
        source: u8,
        destination: u8,
    },
    Test {
        register: u8,
    },
    Compare {
        left: u8,
        right: u8,
    },
    Lea {
        destination: u8,
        base: u8,
        index: Option<u8>,
        displacement: i32,
    },
    Xor {
        source: u8,
        destination: u8,
    },
    Subtract {
        source: u8,
        destination: u8,
    },
    Negate {
        destination: u8,
    },
    Add {
        source: u8,
        destination: u8,
    },
    Return,
}

fn decode_all(bytes: &[u8]) -> Result<Vec<DecodedInstruction>, X86_64SelectedFormEncodingError> {
    if bytes.is_empty() {
        return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
    }
    let mut decoded = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let (instruction, length) = decode_one(&bytes[offset..])?;
        decoded.push(instruction);
        offset = offset
            .checked_add(length)
            .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    }
    Ok(decoded)
}

fn decode_one(
    bytes: &[u8],
) -> Result<(DecodedInstruction, usize), X86_64SelectedFormEncodingError> {
    if let [rex, 0x0f, 0xb6, modrm, ..] = bytes
        && rex & !0x05 == 0x40
        && modrm & 0xc0 == 0xc0
    {
        return Ok((
            DecodedInstruction::ZeroExtendU8 {
                source: (modrm & 7) | ((rex & 1) << 3),
                destination: ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3),
            },
            4,
        ));
    }
    if let [rex, 0x0f, 0xb7, modrm, ..] = bytes
        && rex & !0x05 == 0x40
        && modrm & 0xc0 == 0xc0
    {
        return Ok((
            DecodedInstruction::ZeroExtendU16 {
                source: (modrm & 7) | ((rex & 1) << 3),
                destination: ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3),
            },
            4,
        ));
    }
    if let [rex, 0x0f, 0xbe, modrm, ..] = bytes
        && rex & !0x05 == 0x48
        && modrm & 0xc0 == 0xc0
    {
        return Ok((
            DecodedInstruction::SignExtendI8 {
                source: (modrm & 7) | ((rex & 1) << 3),
                destination: ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3),
            },
            4,
        ));
    }
    if let [rex, 0x0f, 0xbf, modrm, ..] = bytes
        && rex & !0x05 == 0x48
        && modrm & 0xc0 == 0xc0
    {
        return Ok((
            DecodedInstruction::SignExtendI16 {
                source: (modrm & 7) | ((rex & 1) << 3),
                destination: ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3),
            },
            4,
        ));
    }
    if let [rex, 0x63, modrm, ..] = bytes
        && rex & !0x05 == 0x48
        && modrm & 0xc0 == 0xc0
    {
        return Ok((
            DecodedInstruction::SignExtendI32 {
                source: (modrm & 7) | ((rex & 1) << 3),
                destination: ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3),
            },
            3,
        ));
    }
    if let [rex, 0x89, modrm, ..] = bytes
        && rex & !0x05 == 0x40
        && modrm & 0xc0 == 0xc0
    {
        return Ok((
            DecodedInstruction::ZeroExtendU32 {
                source: ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3),
                destination: (modrm & 7) | ((rex & 1) << 3),
            },
            3,
        ));
    }
    if bytes.first() == Some(&0xc3) {
        return Ok((DecodedInstruction::Return, 1));
    }
    let (&rex, rest) = bytes
        .split_first()
        .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    if !(0x48..=0x4f).contains(&rex) {
        return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
    }
    let (&opcode, _) = rest
        .split_first()
        .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    let rex_r = (rex >> 2) & 1;
    let rex_x = (rex >> 1) & 1;
    let rex_b = rex & 1;
    if (0xb8..=0xbf).contains(&opcode) {
        let immediate = bytes
            .get(2..10)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u64::from_le_bytes)
            .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
        return Ok((
            DecodedInstruction::Materialize {
                destination: (opcode & 7) | (rex_b << 3),
                value: immediate,
            },
            10,
        ));
    }
    let modrm = *bytes
        .get(2)
        .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    let mode = modrm >> 6;
    let reg = ((modrm >> 3) & 7) | (rex_r << 3);
    let rm_low = modrm & 7;
    let rm = rm_low | (rex_b << 3);
    if matches!(opcode, 0x89 | 0x85 | 0x39 | 0x31 | 0x29 | 0x01 | 0xf7) {
        if mode != 3 || bytes.len() < 3 {
            return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
        }
        let decoded = match opcode {
            0x89 => DecodedInstruction::Move {
                source: reg,
                destination: rm,
            },
            0x85 if reg == rm => DecodedInstruction::Test { register: reg },
            0x39 => DecodedInstruction::Compare {
                left: rm,
                right: reg,
            },
            0x31 => DecodedInstruction::Xor {
                source: reg,
                destination: rm,
            },
            0x29 => DecodedInstruction::Subtract {
                source: reg,
                destination: rm,
            },
            0x01 => DecodedInstruction::Add {
                source: reg,
                destination: rm,
            },
            0xf7 if (modrm >> 3) & 7 == 3 => DecodedInstruction::Negate { destination: rm },
            _ => return Err(X86_64SelectedFormEncodingError::MalformedEncoding),
        };
        return Ok((decoded, 3));
    }
    if opcode != 0x8d || mode == 3 {
        return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
    }
    let mut length = 3;
    let (base, index) = if rm_low == 4 {
        let sib = *bytes
            .get(length)
            .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
        length += 1;
        if sib >> 6 != 0 {
            return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
        }
        let index_low = (sib >> 3) & 7;
        let base_low = sib & 7;
        if mode == 0 && base_low == 5 {
            return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
        }
        (
            base_low | (rex_b << 3),
            if index_low == 4 && rex_x == 0 {
                None
            } else {
                Some(index_low | (rex_x << 3))
            },
        )
    } else {
        if mode == 0 && rm_low == 5 {
            return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
        }
        (rm, None)
    };
    let displacement = match mode {
        0 => 0,
        1 => {
            let value = *bytes
                .get(length)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
            length += 1;
            i32::from(value as i8)
        }
        2 => {
            let value = bytes
                .get(length..length + 4)
                .and_then(|bytes| bytes.try_into().ok())
                .map(i32::from_le_bytes)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
            length += 4;
            value
        }
        _ => return Err(X86_64SelectedFormEncodingError::MalformedEncoding),
    };
    Ok((
        DecodedInstruction::Lea {
            destination: reg,
            base,
            index,
            displacement,
        },
        length,
    ))
}

fn validate_decoded(
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    registers: &[u8],
    decoded: &[DecodedInstruction],
) -> Result<(), X86_64SelectedFormEncodingError> {
    let valid = match kind {
        SelectedInstructionKind::MaterializeI64 { value } => {
            decoded
                == [DecodedInstruction::Materialize {
                    destination: registers[0],
                    value: integer_bits(value)?,
                }]
        }
        SelectedInstructionKind::ZeroExtendU8 => {
            decoded
                == [DecodedInstruction::ZeroExtendU8 {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::ZeroExtendU16 => {
            decoded
                == [DecodedInstruction::ZeroExtendU16 {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::SignExtendI8 => {
            decoded
                == [DecodedInstruction::SignExtendI8 {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::SignExtendI16 => {
            decoded
                == [DecodedInstruction::SignExtendI16 {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::SignExtendI32 => {
            decoded
                == [DecodedInstruction::SignExtendI32 {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::ZeroExtendU32 => {
            decoded
                == [DecodedInstruction::ZeroExtendU32 {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::CopyI64 => {
            decoded
                == [DecodedInstruction::Move {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::CompareI64Zero => {
            decoded
                == [DecodedInstruction::Test {
                    register: registers[0],
                }]
        }
        SelectedInstructionKind::CompareI64 => {
            decoded
                == [DecodedInstruction::Compare {
                    left: registers[0],
                    right: registers[1],
                }]
        }
        SelectedInstructionKind::ByteViewAddress | SelectedInstructionKind::ExactAddI64 { .. } => {
            matches!(decoded, [DecodedInstruction::Lea { destination, base, index: Some(index), displacement: 0 }]
                if *destination == registers[2]
                    && ((*base == registers[0] && *index == registers[1])
                        || (*base == registers[1] && *index == registers[0])))
        }
        SelectedInstructionKind::ExactAddI64Immediate { immediate, .. } => {
            decoded
                == [DecodedInstruction::Lea {
                    destination: registers[1],
                    base: registers[0],
                    index: None,
                    displacement: i32::try_from(u12(immediate)?).expect("u12 fits i32"),
                }]
        }
        SelectedInstructionKind::ExactSubtractI64Immediate { immediate, .. } => {
            decoded
                == [DecodedInstruction::Lea {
                    destination: registers[1],
                    base: registers[0],
                    index: None,
                    displacement: -i32::try_from(u12(immediate)?).expect("u12 fits i32"),
                }]
        }
        SelectedInstructionKind::ExactSubtractI64 { .. } => match alternative.variant {
            0 => {
                decoded
                    == [DecodedInstruction::Xor {
                        source: registers[2],
                        destination: registers[2],
                    }]
            }
            1 => {
                decoded
                    == [DecodedInstruction::Subtract {
                        source: registers[1],
                        destination: registers[2],
                    }]
            }
            2 => {
                decoded
                    == [
                        DecodedInstruction::Negate {
                            destination: registers[2],
                        },
                        DecodedInstruction::Add {
                            source: registers[0],
                            destination: registers[2],
                        },
                    ]
            }
            3 => {
                decoded
                    == [
                        DecodedInstruction::Move {
                            source: registers[0],
                            destination: registers[2],
                        },
                        DecodedInstruction::Subtract {
                            source: registers[1],
                            destination: registers[2],
                        },
                    ]
            }
            _ => false,
        },
        SelectedInstructionKind::ReturnI64
        | SelectedInstructionKind::ReturnAggregate { .. }
        | SelectedInstructionKind::ReturnUnit => decoded == [DecodedInstruction::Return],
        SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan
        | SelectedInstructionKind::Jump
        | SelectedInstructionKind::Float32ToBits
        | SelectedInstructionKind::Float64ToBits
        | SelectedInstructionKind::BitsToFloat32
        | SelectedInstructionKind::BitsToFloat64
        | SelectedInstructionKind::Load8Indexed
        | SelectedInstructionKind::Load8 { .. }
        | SelectedInstructionKind::Load16 { .. }
        | SelectedInstructionKind::Load32 { .. }
        | SelectedInstructionKind::Load64 { .. }
        | SelectedInstructionKind::Store { .. }
        | SelectedInstructionKind::AddressOffset { .. }
        | SelectedInstructionKind::Store64 { .. }
        | SelectedInstructionKind::HostedExitProcessI32
        | SelectedInstructionKind::HostedReadByte { .. }
        | SelectedInstructionKind::HostedWriteByteI32 { .. }
        | SelectedInstructionKind::FrameAddress { .. }
        | SelectedInstructionKind::CallUnit { .. }
        | SelectedInstructionKind::CallAggregate { .. }
        | SelectedInstructionKind::CallI64 { .. } => false,
    };
    if valid {
        Ok(())
    } else {
        Err(X86_64SelectedFormEncodingError::EncodedFormMismatch)
    }
}

fn footprint(
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> X86_64SelectedFormFootprint {
    let (reads, writes, writes_rflags) = match kind {
        SelectedInstructionKind::MaterializeI64 { .. } => (vec![], vec![operands[0]], false),
        SelectedInstructionKind::CopyI64
        | SelectedInstructionKind::ZeroExtendU8
        | SelectedInstructionKind::ZeroExtendU16
        | SelectedInstructionKind::SignExtendI8
        | SelectedInstructionKind::SignExtendI16
        | SelectedInstructionKind::SignExtendI32
        | SelectedInstructionKind::ZeroExtendU32 => (vec![operands[0]], vec![operands[1]], false),
        SelectedInstructionKind::CompareI64Zero => (vec![operands[0]], vec![], true),
        SelectedInstructionKind::CompareI64 => (vec![operands[0], operands[1]], vec![], true),
        SelectedInstructionKind::ByteViewAddress | SelectedInstructionKind::ExactAddI64 { .. } => {
            (vec![operands[0], operands[1]], vec![operands[2]], false)
        }
        SelectedInstructionKind::ExactAddI64Immediate { .. }
        | SelectedInstructionKind::ExactSubtractI64Immediate { .. } => {
            (vec![operands[0]], vec![operands[1]], false)
        }
        SelectedInstructionKind::ExactSubtractI64 { .. } if alternative.variant == 0 => {
            (vec![], vec![operands[2]], true)
        }
        SelectedInstructionKind::ExactSubtractI64 { .. } => {
            (vec![operands[0], operands[1]], vec![operands[2]], true)
        }
        SelectedInstructionKind::ReturnI64
        | SelectedInstructionKind::ReturnAggregate { .. }
        | SelectedInstructionKind::ReturnUnit => (vec![], vec![], false),
        SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan
        | SelectedInstructionKind::Jump
        | SelectedInstructionKind::Float32ToBits
        | SelectedInstructionKind::Float64ToBits
        | SelectedInstructionKind::BitsToFloat32
        | SelectedInstructionKind::BitsToFloat64
        | SelectedInstructionKind::Load8Indexed
        | SelectedInstructionKind::Load8 { .. }
        | SelectedInstructionKind::Load16 { .. }
        | SelectedInstructionKind::Load32 { .. }
        | SelectedInstructionKind::Load64 { .. }
        | SelectedInstructionKind::Store { .. }
        | SelectedInstructionKind::AddressOffset { .. }
        | SelectedInstructionKind::Store64 { .. }
        | SelectedInstructionKind::HostedExitProcessI32
        | SelectedInstructionKind::HostedReadByte { .. }
        | SelectedInstructionKind::HostedWriteByteI32 { .. }
        | SelectedInstructionKind::FrameAddress { .. }
        | SelectedInstructionKind::CallUnit { .. }
        | SelectedInstructionKind::CallAggregate { .. }
        | SelectedInstructionKind::CallI64 { .. } => (vec![], vec![], false),
    };
    let physical = x86_64_physical_register_model();
    let units = |name: &str| physical.view_named(name).unwrap().units.clone();
    let encoded = if matches!(
        kind,
        SelectedInstructionKind::ReturnI64
            | SelectedInstructionKind::ReturnAggregate { .. }
            | SelectedInstructionKind::ReturnUnit
    ) {
        let stack_pointer = physical.view_named("rsp").unwrap().id;
        let mut defs = units("rsp");
        defs.extend(units("rip"));
        defs.sort_unstable();
        defs.dedup();
        MachineEncodedEffects {
            external_operand_reads: vec![],
            external_operand_writes: vec![],
            implicit_unit_uses: units("rsp"),
            implicit_unit_defs: defs,
            implicit_unit_clobbers: vec![],
            memory: MachineEncodedMemoryEffect::ReadActivationStackV1 {
                stack_pointer,
                byte_count: 8,
            },
            stack: MachineEncodedStackEffect::PopBytesV1 {
                stack_pointer,
                byte_count: 8,
            },
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: MachineEncodedControlEffect::ReturnFromActivationStackV1,
        }
    } else if matches!(
        kind,
        SelectedInstructionKind::ConditionalBranchNonZero
            | SelectedInstructionKind::ConditionalBranchU64LessThan
            | SelectedInstructionKind::ConditionalBranchI64LessThan
    ) {
        let mut uses = units("rflags");
        uses.extend(units("rip"));
        uses.sort_unstable();
        uses.dedup();
        MachineEncodedEffects {
            external_operand_reads: vec![],
            external_operand_writes: vec![],
            implicit_unit_uses: uses,
            implicit_unit_defs: units("rip"),
            implicit_unit_clobbers: vec![],
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: MachineEncodedControlEffect::ConditionalRelativeBranchV1,
        }
    } else {
        let mut effects = MachineEncodedEffects::fallthrough_v1(
            match kind {
                SelectedInstructionKind::MaterializeI64 { .. } => vec![],
                SelectedInstructionKind::CopyI64
                | SelectedInstructionKind::ZeroExtendU8
                | SelectedInstructionKind::ZeroExtendU16
                | SelectedInstructionKind::SignExtendI8
                | SelectedInstructionKind::SignExtendI16
                | SelectedInstructionKind::SignExtendI32
                | SelectedInstructionKind::ZeroExtendU32
                | SelectedInstructionKind::CompareI64Zero
                | SelectedInstructionKind::ExactAddI64Immediate { .. }
                | SelectedInstructionKind::ExactSubtractI64Immediate { .. } => vec![0],
                SelectedInstructionKind::CompareI64 => vec![0, 1],
                SelectedInstructionKind::ByteViewAddress
                | SelectedInstructionKind::ExactAddI64 { .. } => vec![0, 1],
                SelectedInstructionKind::ExactSubtractI64 { .. } if alternative.variant == 0 => {
                    vec![]
                }
                SelectedInstructionKind::ExactSubtractI64 { .. } => vec![0, 1],
                _ => unreachable!("control forms handled separately"),
            },
            match kind {
                SelectedInstructionKind::MaterializeI64 { .. } => vec![0],
                SelectedInstructionKind::CopyI64
                | SelectedInstructionKind::ZeroExtendU8
                | SelectedInstructionKind::ZeroExtendU16
                | SelectedInstructionKind::SignExtendI8
                | SelectedInstructionKind::SignExtendI16
                | SelectedInstructionKind::SignExtendI32
                | SelectedInstructionKind::ZeroExtendU32
                | SelectedInstructionKind::ExactAddI64Immediate { .. }
                | SelectedInstructionKind::ExactSubtractI64Immediate { .. } => vec![1],
                SelectedInstructionKind::ByteViewAddress
                | SelectedInstructionKind::ExactAddI64 { .. }
                | SelectedInstructionKind::ExactSubtractI64 { .. } => vec![2],
                SelectedInstructionKind::CompareI64Zero => vec![],
                SelectedInstructionKind::CompareI64 => vec![],
                _ => unreachable!("control forms handled separately"),
            },
        );
        if matches!(
            kind,
            SelectedInstructionKind::CompareI64Zero | SelectedInstructionKind::CompareI64
        ) {
            effects.implicit_unit_defs = units("rflags");
        }
        if matches!(kind, SelectedInstructionKind::ExactSubtractI64 { .. }) {
            effects.implicit_unit_clobbers = units("rflags");
        }
        effects
    };
    X86_64SelectedFormFootprint {
        register_reads: reads,
        register_writes: writes,
        writes_rflags,
        encoded,
    }
}

#[cfg(test)]
mod tests {
    use optimization_core::AcceptedObligationFactIdentity;
    use register_model::validate_physical_register_model;
    use semantic_vocabulary::{MachineId, ObligationId};

    use super::*;

    fn alternative(family: MachineAlternativeFamily, variant: u32) -> MachineAlternativeKey {
        MachineAlternativeKey { family, variant }
    }

    fn exact_add() -> SelectedInstructionKind {
        SelectedInstructionKind::ExactAddI64 {
            obligation: ObligationId::new(1).unwrap(),
            accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
        }
    }

    #[test]
    fn scalar_call_is_explicitly_refused_before_encoding() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        assert_eq!(
            encode_x86_64_selected_form(
                &physical,
                SelectedInstructionKind::CallI64 {
                    callee: MachineId::new(1).unwrap(),
                },
                alternative(MachineAlternativeFamily::ReturnUnit, 0),
                &[],
            ),
            Err(X86_64SelectedFormEncodingError::LayoutDependentForm),
        );
    }

    #[test]
    fn zero_extend_u8_binds_width_registers_and_is_not_a_copy() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        for source in ["rax", "rcx", "rsi", "rdi", "r8", "r15"] {
            for destination in ["rax", "rcx", "rsi", "rdi", "r8", "r15"] {
                let operands = [
                    physical.model().view_named(source).unwrap().id,
                    physical.model().view_named(destination).unwrap().id,
                ];
                let kind = SelectedInstructionKind::ZeroExtendU8;
                let key = alternative(MachineAlternativeFamily::ZeroExtendU8, 0);
                let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
                assert_eq!(encoded.bytes().len(), 4);
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &operands,
                    encoded.bytes(),
                )
                .unwrap();
                let copy = encode_x86_64_selected_form(
                    &physical,
                    SelectedInstructionKind::CopyI64,
                    alternative(MachineAlternativeFamily::CopyI64, 0),
                    &operands,
                )
                .unwrap();
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        key,
                        &operands,
                        copy.bytes()
                    )
                    .is_err()
                );
                for byte in 0..4 {
                    let mut changed = encoded.bytes().to_vec();
                    changed[byte] ^= 1;
                    assert!(
                        validate_x86_64_selected_form_encoding(
                            &physical, kind, key, &operands, &changed
                        )
                        .is_err()
                    );
                }
            }
        }
    }

    #[test]
    fn zero_extend_u32_binds_width_registers_and_is_not_a_copy() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        for source in ["rax", "rcx", "rsi", "rdi", "r8", "r15"] {
            for destination in ["rax", "rcx", "rsi", "rdi", "r8", "r15"] {
                let operands = [
                    physical.model().view_named(source).unwrap().id,
                    physical.model().view_named(destination).unwrap().id,
                ];
                let kind = SelectedInstructionKind::ZeroExtendU32;
                let key = alternative(MachineAlternativeFamily::ZeroExtendU32, 0);
                let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
                assert_eq!(encoded.bytes().len(), 3);
                // A 32-bit MOV writes the full parent register with zero upper bits.
                assert_eq!(encoded.bytes()[0] & 0xf8, 0x40);
                assert_eq!(encoded.bytes()[1], 0x89);
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &operands,
                    encoded.bytes(),
                )
                .unwrap();
                let narrow = encode_x86_64_selected_form(
                    &physical,
                    SelectedInstructionKind::ZeroExtendU8,
                    alternative(MachineAlternativeFamily::ZeroExtendU8, 0),
                    &operands,
                )
                .unwrap();
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        key,
                        &operands,
                        narrow.bytes()
                    )
                    .is_err()
                );
                let copy = encode_x86_64_selected_form(
                    &physical,
                    SelectedInstructionKind::CopyI64,
                    alternative(MachineAlternativeFamily::CopyI64, 0),
                    &operands,
                )
                .unwrap();
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        key,
                        &operands,
                        copy.bytes()
                    )
                    .is_err()
                );
                for byte in 0..3 {
                    let mut changed = encoded.bytes().to_vec();
                    changed[byte] ^= 1;
                    assert!(
                        validate_x86_64_selected_form_encoding(
                            &physical, kind, key, &operands, &changed
                        )
                        .is_err()
                    );
                }
            }
        }
    }

    #[test]
    fn r12_is_a_valid_rex_extended_sib_index() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let r12 = physical.model().view_named("r12").unwrap().id;
        let rax = physical.model().view_named("rax").unwrap().id;
        let encoded = encode_x86_64_selected_form(
            &physical,
            exact_add(),
            alternative(MachineAlternativeFamily::ExactAddI64, 0),
            &[r12, r12, rax],
        )
        .unwrap();
        assert_eq!(encoded.bytes(), [0x4b, 0x8d, 0x04, 0x24]);
    }

    #[test]
    fn compare_i64_is_canonical_register_cmp_with_exact_footprint() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let rax = physical.model().view_named("rax").unwrap().id;
        let rbx = physical.model().view_named("rbx").unwrap().id;
        let r8 = physical.model().view_named("r8").unwrap().id;
        let r9 = physical.model().view_named("r9").unwrap().id;
        let alternative = alternative(MachineAlternativeFamily::CompareI64, 0);
        for (operands, expected) in [
            ([rax, rbx], [0x48, 0x39, 0xd8]),
            ([r8, r9], [0x4d, 0x39, 0xc8]),
            ([rax, rax], [0x48, 0x39, 0xc0]),
        ] {
            let encoded = encode_x86_64_selected_form(
                &physical,
                SelectedInstructionKind::CompareI64,
                alternative,
                &operands,
            )
            .unwrap();
            assert_eq!(encoded.bytes(), expected);
            assert_eq!(encoded.footprint().register_reads, operands);
            assert!(encoded.footprint().register_writes.is_empty());
            assert!(encoded.footprint().writes_rflags);
            assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
        }
    }

    #[test]
    fn scalar_sizes_and_subtraction_alias_partitions_are_exact() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let views = ["rax", "rbx", "rcx"].map(|name| physical.model().view_named(name).unwrap().id);
        let materialize = encode_x86_64_selected_form(
            &physical,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Signed(-1),
            },
            alternative(MachineAlternativeFamily::MaterializeI64, 0),
            &views[..1],
        )
        .unwrap();
        assert_eq!(materialize.bytes().len(), 10);
        for (homes, variant, size) in [
            ([views[0], views[0], views[0]], 0, 3),
            ([views[0], views[1], views[0]], 1, 3),
            ([views[0], views[1], views[1]], 2, 6),
            ([views[0], views[1], views[2]], 3, 6),
        ] {
            let kind = SelectedInstructionKind::ExactSubtractI64 {
                obligation: ObligationId::new(2).unwrap(),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([4; 32]),
            };
            let encoded = encode_x86_64_selected_form(
                &physical,
                kind,
                alternative(MachineAlternativeFamily::ExactSubtractI64, variant),
                &homes,
            )
            .unwrap();
            assert_eq!(encoded.bytes().len(), size);
            let mut corrupted = encoded.bytes().to_vec();
            corrupted[1] ^= 1;
            assert!(
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    alternative(MachineAlternativeFamily::ExactSubtractI64, variant),
                    &homes,
                    &corrupted,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn immediate_lea_uses_declared_size_extremes() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let rax = physical.model().view_named("rax").unwrap().id;
        let r12 = physical.model().view_named("r12").unwrap().id;
        let fact = AcceptedObligationFactIdentity::from_bytes([5; 32]);
        for (base, immediate, size) in [(rax, 0, 4), (rax, 4095, 7), (r12, 0, 5), (r12, 4095, 8)] {
            let kind = SelectedInstructionKind::ExactAddI64Immediate {
                immediate: IntegerValue::Unsigned(immediate),
                obligation: ObligationId::new(3).unwrap(),
                accepted_fact: fact,
            };
            let encoded = encode_x86_64_selected_form(
                &physical,
                kind,
                alternative(MachineAlternativeFamily::ExactAddI64Immediate, 0),
                &[base, rax],
            )
            .unwrap();
            assert_eq!(encoded.bytes().len(), size);
        }
    }

    #[test]
    fn subtract_immediate_lea_is_negative_canonical_and_flag_transparent() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let rax = physical.model().view_named("rax").unwrap().id;
        let r12 = physical.model().view_named("r12").unwrap().id;
        let fact = AcceptedObligationFactIdentity::from_bytes([6; 32]);
        for (base, immediate, expected) in [
            (rax, 0, vec![0x48, 0x8d, 0x40, 0x00]),
            (rax, 128, vec![0x48, 0x8d, 0x40, 0x80]),
            (rax, 129, vec![0x48, 0x8d, 0x80, 0x7f, 0xff, 0xff, 0xff]),
            (
                r12,
                4095,
                vec![0x49, 0x8d, 0x84, 0x24, 0x01, 0xf0, 0xff, 0xff],
            ),
        ] {
            let kind = SelectedInstructionKind::ExactSubtractI64Immediate {
                immediate: IntegerValue::Unsigned(immediate),
                obligation: ObligationId::new(4).unwrap(),
                accepted_fact: fact,
            };
            let alternative = alternative(MachineAlternativeFamily::ExactSubtractI64Immediate, 0);
            let encoded =
                encode_x86_64_selected_form(&physical, kind, alternative, &[base, rax]).unwrap();
            assert_eq!(encoded.bytes(), expected);
            assert!(!encoded.footprint().writes_rflags);
            assert!(
                encoded
                    .footprint()
                    .encoded
                    .implicit_unit_clobbers
                    .is_empty()
            );
            let mut wrong_sign = expected;
            *wrong_sign.last_mut().unwrap() ^= 0x80;
            assert!(
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    alternative,
                    &[base, rax],
                    &wrong_sign,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn near_return_is_exact_and_separates_abi_result_custody_from_encoded_effects() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let rax = physical.model().view_named("rax").unwrap().id;
        let rbx = physical.model().view_named("rbx").unwrap().id;
        let rsp = physical.model().view_named("rsp").unwrap();
        let rip = physical.model().view_named("rip").unwrap();
        let kind = SelectedInstructionKind::ReturnI64;
        let alternative = alternative(MachineAlternativeFamily::ReturnI64, 0);
        let encoded = encode_x86_64_selected_form(&physical, kind, alternative, &[rax]).unwrap();

        assert_eq!(encoded.bytes(), [0xc3]);
        assert!(encoded.footprint().register_reads.is_empty());
        assert!(encoded.footprint().register_writes.is_empty());
        assert_eq!(encoded.footprint().encoded.external_operand_reads, []);
        assert_eq!(encoded.footprint().encoded.external_operand_writes, []);
        assert_eq!(encoded.footprint().encoded.implicit_unit_uses, rsp.units);
        let mut expected_defs = rsp.units.clone();
        expected_defs.extend(&rip.units);
        expected_defs.sort_unstable();
        expected_defs.dedup();
        assert_eq!(
            encoded.footprint().encoded.implicit_unit_defs,
            expected_defs
        );
        assert_eq!(
            encoded.footprint().encoded.memory,
            MachineEncodedMemoryEffect::ReadActivationStackV1 {
                stack_pointer: rsp.id,
                byte_count: 8,
            }
        );
        assert_eq!(
            encoded.footprint().encoded.stack,
            MachineEncodedStackEffect::PopBytesV1 {
                stack_pointer: rsp.id,
                byte_count: 8,
            }
        );
        assert_eq!(
            encoded.footprint().encoded.trap,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        );
        assert_eq!(
            encoded.footprint().encoded.control,
            MachineEncodedControlEffect::ReturnFromActivationStackV1
        );
        assert!(encode_x86_64_selected_form(&physical, kind, alternative, &[rbx]).is_err());
        assert!(
            validate_x86_64_selected_form_encoding(
                &physical,
                kind,
                alternative,
                &[rax],
                &[0xc2, 0, 0]
            )
            .is_err()
        );
        assert!(
            validate_x86_64_selected_form_encoding(
                &physical,
                kind,
                alternative,
                &[rax],
                &[0xc3, 0xc3]
            )
            .is_err()
        );
    }

    #[test]
    fn unit_return_is_a_distinct_zero_operand_near_return() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let kind = SelectedInstructionKind::ReturnUnit;
        let return_alternative = alternative(MachineAlternativeFamily::ReturnUnit, 0);
        let encoded =
            encode_x86_64_selected_form(&physical, kind, return_alternative, &[]).unwrap();

        assert_eq!(encoded.bytes(), [0xc3]);
        assert!(encoded.footprint().register_reads.is_empty());
        assert!(encoded.footprint().register_writes.is_empty());
        assert_eq!(
            encoded.footprint().encoded.control,
            MachineEncodedControlEffect::ReturnFromActivationStackV1
        );
        assert!(
            encode_x86_64_selected_form(
                &physical,
                kind,
                alternative(MachineAlternativeFamily::ReturnI64, 0),
                &[]
            )
            .is_err()
        );
    }

    #[test]
    fn near_nonzero_branch_has_exact_end_relative_displacement_and_effects() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let alternative = alternative(MachineAlternativeFamily::ConditionalBranchNonZero, 0);
        for displacement in [i64::from(i32::MIN), -6, 0, 6, i64::from(i32::MAX)] {
            let encoded =
                encode_x86_64_selected_nonzero_branch_form(&physical, alternative, displacement)
                    .unwrap();
            assert_eq!(&encoded.bytes()[..2], [0x0f, 0x85]);
            assert_eq!(
                i32::from_le_bytes(encoded.bytes()[2..].try_into().unwrap()),
                displacement as i32
            );
            assert!(encoded.footprint().register_reads.is_empty());
            assert!(encoded.footprint().register_writes.is_empty());
            assert_eq!(
                encoded.footprint().encoded.control,
                MachineEncodedControlEffect::ConditionalRelativeBranchV1
            );
        }
        assert!(
            encode_x86_64_selected_nonzero_branch_form(
                &physical,
                alternative,
                i64::from(i32::MAX) + 1
            )
            .is_err()
        );
        assert!(
            validate_x86_64_selected_nonzero_branch_form(
                &physical,
                alternative,
                0,
                &[0x0f, 0x84, 0, 0, 0, 0]
            )
            .is_err()
        );
        assert!(
            validate_x86_64_selected_nonzero_branch_form(
                &physical,
                alternative,
                0,
                &[0x0f, 0x85, 0, 0, 0, 0, 0]
            )
            .is_err()
        );
    }

    #[test]
    fn short_nonzero_branch_has_exact_signed_rel8_bounds_and_near_footprint() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let alternative = alternative(MachineAlternativeFamily::ConditionalBranchNonZero, 0);
        let near = encode_x86_64_selected_nonzero_branch_form(&physical, alternative, 0).unwrap();

        for (displacement, encoded_displacement) in [(-128, 0x80), (127, 0x7f)] {
            let encoded = encode_x86_64_selected_short_nonzero_branch_form(
                &physical,
                alternative,
                displacement,
            )
            .unwrap();
            assert_eq!(encoded.bytes(), [0x75, encoded_displacement]);
            assert_eq!(encoded.footprint(), near.footprint());
        }

        for displacement in [-129, 128] {
            assert_eq!(
                encode_x86_64_selected_short_nonzero_branch_form(
                    &physical,
                    alternative,
                    displacement,
                ),
                Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)
            );
            assert_eq!(
                validate_x86_64_selected_short_nonzero_branch_form(
                    &physical,
                    alternative,
                    displacement,
                    &[0x75, 0],
                ),
                Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)
            );
        }
    }

    #[test]
    fn short_nonzero_branch_validation_rejects_every_noncanonical_form() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let canonical_alternative =
            alternative(MachineAlternativeFamily::ConditionalBranchNonZero, 0);

        for bytes in [&[0x74, 0][..], &[0x75][..], &[0x75, 0, 0][..]] {
            assert_eq!(
                validate_x86_64_selected_short_nonzero_branch_form(
                    &physical,
                    canonical_alternative,
                    0,
                    bytes,
                ),
                Err(X86_64SelectedFormEncodingError::MalformedEncoding)
            );
        }
        assert_eq!(
            validate_x86_64_selected_short_nonzero_branch_form(
                &physical,
                canonical_alternative,
                0,
                &[0x75, 1],
            ),
            Err(X86_64SelectedFormEncodingError::EncodedFormMismatch)
        );

        let wrong_alternative = alternative(MachineAlternativeFamily::ConditionalBranchNonZero, 1);
        assert_eq!(
            encode_x86_64_selected_short_nonzero_branch_form(&physical, wrong_alternative, 0,),
            Err(X86_64SelectedFormEncodingError::AlternativeMismatch)
        );
        assert_eq!(
            validate_x86_64_selected_short_nonzero_branch_form(
                &physical,
                alternative(MachineAlternativeFamily::ReturnI64, 0),
                0,
                &[0x75, 0],
            ),
            Err(X86_64SelectedFormEncodingError::AlternativeMismatch)
        );

        let mut forged = x86_64_physical_register_model();
        forged.views[0].name = "forged.rax".into();
        let forged = validate_physical_register_model(forged).unwrap();
        assert_eq!(
            encode_x86_64_selected_short_nonzero_branch_form(&forged, canonical_alternative, 0,),
            Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel)
        );
    }

    #[test]
    fn near_u64_less_than_branch_is_exact_jb_rel32_with_flag_control_effects() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let rflags = physical.model().view_named("rflags").unwrap();
        let rip = physical.model().view_named("rip").unwrap();
        let alternative = alternative(MachineAlternativeFamily::ConditionalBranchU64LessThan, 0);
        for displacement in [i64::from(i32::MIN), -6, 0, 6, i64::from(i32::MAX)] {
            let encoded = encode_x86_64_selected_u64_less_than_branch_form(
                &physical,
                alternative,
                displacement,
            )
            .unwrap();
            assert_eq!(&encoded.bytes()[..2], [0x0f, 0x82]);
            assert_eq!(
                i32::from_le_bytes(encoded.bytes()[2..].try_into().unwrap()),
                displacement as i32
            );
            assert!(encoded.footprint().register_reads.is_empty());
            assert!(encoded.footprint().register_writes.is_empty());
            assert_eq!(
                encoded.footprint().encoded.control,
                MachineEncodedControlEffect::ConditionalRelativeBranchV1
            );
            assert!(rflags.units.iter().all(|unit| {
                encoded
                    .footprint()
                    .encoded
                    .implicit_unit_uses
                    .contains(unit)
            }));
            assert!(rip.units.iter().all(|unit| {
                encoded
                    .footprint()
                    .encoded
                    .implicit_unit_uses
                    .contains(unit)
            }));
            assert_eq!(encoded.footprint().encoded.implicit_unit_defs, rip.units);
        }
        assert_eq!(
            encode_x86_64_selected_u64_less_than_branch_form(
                &physical,
                alternative,
                i64::from(i32::MAX) + 1,
            ),
            Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)
        );
        for bytes in [
            &[0x0f, 0x83, 0, 0, 0, 0][..],
            &[0x0f, 0x82, 0, 0, 0][..],
            &[0x0f, 0x82, 0, 0, 0, 0, 0][..],
        ] {
            assert_eq!(
                validate_x86_64_selected_u64_less_than_branch_form(
                    &physical,
                    alternative,
                    0,
                    bytes,
                ),
                Err(X86_64SelectedFormEncodingError::MalformedEncoding)
            );
        }
    }

    #[test]
    fn short_u64_less_than_branch_is_exact_jb_rel8_and_rejects_near_forms() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let near_alternative =
            alternative(MachineAlternativeFamily::ConditionalBranchU64LessThan, 0);
        let short_alternative =
            alternative(MachineAlternativeFamily::ConditionalBranchU64LessThan, 1);
        let near = encode_x86_64_selected_u64_less_than_branch_form(&physical, near_alternative, 0)
            .unwrap();
        for (displacement, encoded_displacement) in [(-128, 0x80), (127, 0x7f)] {
            let encoded = encode_x86_64_selected_u64_less_than_branch_form(
                &physical,
                short_alternative,
                displacement,
            )
            .unwrap();
            assert_eq!(encoded.bytes(), [0x72, encoded_displacement]);
            assert_eq!(encoded.footprint(), near.footprint());
        }
        for displacement in [-129, 128] {
            assert_eq!(
                encode_x86_64_selected_u64_less_than_branch_form(
                    &physical,
                    short_alternative,
                    displacement,
                ),
                Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)
            );
        }
        for bytes in [
            &[0x73, 0][..],
            &[0x72][..],
            &[0x72, 0, 0][..],
            &[0x0f, 0x82, 0, 0, 0, 0][..],
        ] {
            assert_eq!(
                validate_x86_64_selected_u64_less_than_branch_form(
                    &physical,
                    short_alternative,
                    0,
                    bytes,
                ),
                Err(X86_64SelectedFormEncodingError::MalformedEncoding)
            );
        }
        assert_eq!(
            validate_x86_64_selected_u64_less_than_branch_form(
                &physical,
                short_alternative,
                0,
                &[0x72, 1],
            ),
            Err(X86_64SelectedFormEncodingError::EncodedFormMismatch)
        );
    }

    #[test]
    fn signed_less_than_branch_uses_exact_near_and_short_jl_forms() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let near = alternative(MachineAlternativeFamily::ConditionalBranchI64LessThan, 0);
        let short = alternative(MachineAlternativeFamily::ConditionalBranchI64LessThan, 1);

        let encoded =
            encode_x86_64_selected_i64_less_than_branch_form(&physical, near, -6).unwrap();
        assert_eq!(encoded.bytes(), [0x0f, 0x8c, 0xfa, 0xff, 0xff, 0xff]);
        assert_eq!(
            encoded.footprint().encoded.control,
            MachineEncodedControlEffect::ConditionalRelativeBranchV1
        );
        let encoded =
            encode_x86_64_selected_i64_less_than_branch_form(&physical, short, -2).unwrap();
        assert_eq!(encoded.bytes(), [0x7c, 0xfe]);
        assert_eq!(
            validate_x86_64_selected_i64_less_than_branch_form(&physical, short, 0, &[0x72, 0],),
            Err(X86_64SelectedFormEncodingError::MalformedEncoding)
        );
        assert_eq!(
            encode_x86_64_selected_i64_less_than_branch_form(&physical, short, 128),
            Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)
        );
    }
}
