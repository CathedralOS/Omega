//! Request validation, operand families, return homes, register
//! resolution and alias partitions.

use crate::selected_form_encoding::X86_64SelectedFormEncodingError;
use crate::selected_form_encoding::saturating_forms::SaturatingForm;
use crate::x86_64_physical_register_model;
use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, SaturatingOperation, SelectedInstructionKind,
};

pub(crate) fn validate_request(
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
        SelectedInstructionKind::CompareI64Immediate { .. } => {
            (MachineAlternativeFamily::CompareI64Immediate, 1, 0..=0)
        }
        SelectedInstructionKind::MaterializeI64 { .. } => {
            (MachineAlternativeFamily::MaterializeI64, 1, 0..=0)
        }
        SelectedInstructionKind::CopyI64 => (MachineAlternativeFamily::CopyI64, 2, 0..=0),
        SelectedInstructionKind::MaterializeBooleanEqual => {
            (MachineAlternativeFamily::MaterializeBooleanEqual, 1, 0..=0)
        }
        SelectedInstructionKind::MaterializeBooleanU64LessThan => (
            MachineAlternativeFamily::MaterializeBooleanU64LessThan,
            1,
            0..=0,
        ),
        SelectedInstructionKind::MaterializeBooleanI64LessThan => (
            MachineAlternativeFamily::MaterializeBooleanI64LessThan,
            1,
            0..=0,
        ),
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => (
            MachineAlternativeFamily::MaterializeBooleanU64LessOrEqual,
            1,
            0..=0,
        ),
        SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => (
            MachineAlternativeFamily::MaterializeBooleanI64LessOrEqual,
            1,
            0..=0,
        ),
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
        SelectedInstructionKind::WrappingAddI64 => {
            (MachineAlternativeFamily::WrappingAddI64, 3, 0..=0)
        }
        SelectedInstructionKind::BitwiseAndI64 => {
            (MachineAlternativeFamily::BitwiseAndI64, 3, 0..=0)
        }
        SelectedInstructionKind::BitwiseOrI64 => (MachineAlternativeFamily::BitwiseOrI64, 3, 0..=0),
        SelectedInstructionKind::BitwiseNotI64 => {
            (MachineAlternativeFamily::BitwiseNotI64, 2, 0..=0)
        }
        SelectedInstructionKind::ExactDivideU64 { .. } => {
            (MachineAlternativeFamily::ExactDivideU64, 4, 0..=0)
        }
        SelectedInstructionKind::ExactRemainderU64 { .. } => {
            (MachineAlternativeFamily::ExactRemainderU64, 4, 0..=0)
        }
        SelectedInstructionKind::WrappingRemainderI64 { .. } => {
            (MachineAlternativeFamily::WrappingRemainderI64, 4, 0..=0)
        }
        SelectedInstructionKind::WrappingDivideI64 { .. } => {
            (MachineAlternativeFamily::WrappingDivideI64, 4, 0..=0)
        }
        // The carrier's realization shape fixes the operand count: the u64
        // add and every unsigned subtract are three-operand forms, everything
        // else carries the early-clobber scratch or the fixed RDX input.
        SelectedInstructionKind::SaturatingAdd { carrier } => (
            MachineAlternativeFamily::SaturatingAdd(carrier),
            SaturatingForm::of(SaturatingOperation::Add, carrier).operand_count(),
            0..=0,
        ),
        SelectedInstructionKind::SaturatingSubtract { carrier } => (
            MachineAlternativeFamily::SaturatingSubtract(carrier),
            SaturatingForm::of(SaturatingOperation::Subtract, carrier).operand_count(),
            0..=0,
        ),
        SelectedInstructionKind::SaturatingDivide { carrier, .. } => (
            MachineAlternativeFamily::SaturatingDivide(carrier),
            SaturatingForm::of(SaturatingOperation::Divide, carrier).operand_count(),
            0..=0,
        ),
        SelectedInstructionKind::SaturatingRemainder { carrier, .. } => (
            MachineAlternativeFamily::SaturatingRemainder(carrier),
            SaturatingForm::of(SaturatingOperation::Remainder, carrier).operand_count(),
            0..=0,
        ),
        SelectedInstructionKind::BitwiseXorI64 => {
            (MachineAlternativeFamily::BitwiseXorI64, 3, 0..=0)
        }
        SelectedInstructionKind::ExactSubtractI64 { .. } => {
            (MachineAlternativeFamily::ExactSubtractI64, 3, 0..=3)
        }
        SelectedInstructionKind::WrappingSubtractI64 => {
            (MachineAlternativeFamily::WrappingSubtractI64, 3, 0..=3)
        }
        SelectedInstructionKind::ExactMultiplyI64 { .. } => {
            (MachineAlternativeFamily::ExactMultiplyI64, 3, 0..=3)
        }
        SelectedInstructionKind::WrappingMultiplyI64 => {
            (MachineAlternativeFamily::WrappingMultiplyI64, 3, 0..=3)
        }
        SelectedInstructionKind::ExactAddI64Immediate { .. } => {
            (MachineAlternativeFamily::ExactAddI64Immediate, 2, 0..=0)
        }
        SelectedInstructionKind::ExactSubtractI64Immediate { .. } => (
            MachineAlternativeFamily::ExactSubtractI64Immediate,
            2,
            0..=0,
        ),
        SelectedInstructionKind::ReturnScalar => (MachineAlternativeFamily::ReturnScalar, 1, 0..=0),
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
        | SelectedInstructionKind::CopyBytes
        | SelectedInstructionKind::Load8 { .. }
        | SelectedInstructionKind::Load16 { .. }
        | SelectedInstructionKind::LoadPacked { .. }
        | SelectedInstructionKind::StorePacked { .. }
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
        | SelectedInstructionKind::NormalizedForeignCall { .. }
        | SelectedInstructionKind::CallScalar { .. } => {
            return Err(X86_64SelectedFormEncodingError::LayoutDependentForm);
        }
    })
}

pub(crate) fn validate_return_home(
    kind: SelectedInstructionKind,
    registers: &[u8],
) -> Result<(), X86_64SelectedFormEncodingError> {
    if matches!(kind, SelectedInstructionKind::ReturnScalar) && registers != [0] {
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

pub(crate) fn resolve_scalar_registers(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    operands: &[RegisterViewId],
) -> Result<Vec<u8>, X86_64SelectedFormEncodingError> {
    // RET does not encode the returned register. The exact constraint row
    // retains its ABI class; accept only the canonical floating result home.
    if kind == SelectedInstructionKind::ReturnScalar
        && physical
            .model()
            .view_named("xmm0")
            .is_some_and(|view| operands == [view.id])
    {
        return Ok(vec![0]);
    }
    resolve_registers(physical, operands)
}

pub(crate) fn resolve_registers(
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

pub(crate) fn validate_alias_partition(
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    registers: &[u8],
) -> Result<(), X86_64SelectedFormEncodingError> {
    if !matches!(
        kind,
        SelectedInstructionKind::ExactSubtractI64 { .. }
            | SelectedInstructionKind::WrappingSubtractI64
            | SelectedInstructionKind::ExactMultiplyI64 { .. }
            | SelectedInstructionKind::WrappingMultiplyI64
    ) {
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
