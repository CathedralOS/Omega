//! Request validation, operand families, return homes, register
//! resolution and alias partitions.

use crate::selected_form_encoding::X86_64SelectedFormEncodingError;
use crate::selected_form_encoding::saturating_forms::SaturatingForm;
use crate::selected_form_encoding::trapping_forms::TrappingShape;
use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily as Family, MachineAlternativeKey, SaturatingOperation,
    SelectedInstructionKind as Kind,
};

/// Every encoder entrypoint first rejects a foreign register model — the
/// canonical x86-64 view roster is the only one these encodings can name.
pub(crate) fn guard_canonical(
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), X86_64SelectedFormEncodingError> {
    if physical.identity() != crate::canonical_x86_64_physical_register_model_identity() {
        return Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    Ok(())
}

/// Request registration shared by the byte-loop forms (`CopyBytes`,
/// `ZeroBytes`, `ScanByteSequence`, `CompareByteSequences`): the canonical
/// model, the form's single `family` alternative, the exact operand count,
/// and the early-clobber rule. Every scratch register from `preserved` on is
/// written before the inputs after it are read, so a scratch may not reuse a
/// preserved input or an earlier scratch.
pub(super) fn byte_loop_registers<const N: usize>(
    physical: &ValidatedPhysicalRegisterModel,
    requested: MachineAlternativeKey,
    family: Family,
    operands: &[RegisterViewId],
    preserved: usize,
) -> Result<[u8; N], X86_64SelectedFormEncodingError> {
    guard_canonical(physical)?;
    if requested != (MachineAlternativeKey { family, variant: 0 }) {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let registers: [u8; N] = resolve_registers(physical, operands)?
        .try_into()
        .map_err(|_| X86_64SelectedFormEncodingError::OperandCountMismatch)?;
    for position in preserved..N {
        if registers[..position].contains(&registers[position]) {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
    }
    Ok(registers)
}

pub(crate) fn validate_request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: Kind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<(), X86_64SelectedFormEncodingError> {
    guard_canonical(physical)?;
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
    kind: Kind,
) -> Result<(Family, usize, std::ops::RangeInclusive<u32>), X86_64SelectedFormEncodingError> {
    Ok(match kind {
        Kind::RegisterAccess { operation } => (
            super::register_access::family(operation)?,
            operation.input_carriers().len() + usize::from(operation.produces_value()),
            0..=0,
        ),
        Kind::AccessorScalarAccess { form, .. } => (Family::AccessorScalarAccess(form), 2, 0..=0),
        Kind::FloatResultPolicy { form } => (
            Family::FloatResultPolicy(form),
            form.operand_count() + 1,
            0..=0,
        ),
        Kind::AtomicAccess { form } => (
            Family::AtomicAccess(form),
            usize::from(form.operand_count()),
            0..=0,
        ),
        // Float arithmetic owns its request, encoding and decoding.
        Kind::NearestIeeeFloatArithmetic { form } => (
            Family::NearestIeeeFloatArithmetic(form),
            form.operand_count() + 1,
            0..=0,
        ),
        Kind::PortWrite { .. } => (Family::PortWrite, 0, 0..=0),
        Kind::AtomicFence => (Family::AtomicFence, 0, 0..=0),
        Kind::Crash => (Family::Crash, 0, 0..=0),
        Kind::CompareI64Zero => (Family::CompareI64Zero, 1, 0..=0),
        Kind::CompareI64 => (Family::CompareI64, 2, 0..=0),
        Kind::CompareI64Immediate { .. } => (Family::CompareI64Immediate, 1, 0..=0),
        // Variant 1 is the optimizer-selected `xor r64, r64` zero idiom.
        Kind::MaterializeI64 { .. } => (Family::MaterializeI64, 1, 0..=1),
        Kind::ImageAddress { .. } => (Family::ImageAddress, 1, 0..=0),
        Kind::CopyI64 => (Family::CopyI64, 2, 0..=0),
        Kind::MaterializeBooleanEqual => (Family::MaterializeBooleanEqual, 1, 0..=0),
        Kind::MaterializeBooleanU64LessThan => (Family::MaterializeBooleanU64LessThan, 1, 0..=0),
        Kind::MaterializeBooleanI64LessThan => (Family::MaterializeBooleanI64LessThan, 1, 0..=0),
        Kind::MaterializeBooleanU64LessOrEqual => {
            (Family::MaterializeBooleanU64LessOrEqual, 1, 0..=0)
        }
        Kind::MaterializeBooleanI64LessOrEqual => {
            (Family::MaterializeBooleanI64LessOrEqual, 1, 0..=0)
        }
        Kind::ZeroExtendU8 => (Family::ZeroExtendU8, 2, 0..=0),
        Kind::ZeroExtendU16 => (Family::ZeroExtendU16, 2, 0..=0),
        Kind::SignExtendI8 => (Family::SignExtendI8, 2, 0..=0),
        Kind::SignExtendI16 => (Family::SignExtendI16, 2, 0..=0),
        Kind::SignExtendI32 => (Family::SignExtendI32, 2, 0..=0),
        Kind::ZeroExtendU32 => (Family::ZeroExtendU32, 2, 0..=0),
        Kind::ByteViewAddress => (Family::ByteViewAddress, 3, 0..=0),
        Kind::ExactAddI64 { .. } => (Family::ExactAddI64, 3, 0..=0),
        Kind::WrappingAddI64 => (Family::WrappingAddI64, 3, 0..=0),
        Kind::BitwiseAndI64 => (Family::BitwiseAndI64, 3, 0..=0),
        Kind::BitwiseOrI64 => (Family::BitwiseOrI64, 3, 0..=0),
        Kind::BitwiseNotI64 => (Family::BitwiseNotI64, 2, 0..=0),
        Kind::ExactDivideU64 { .. } => (Family::ExactDivideU64, 4, 0..=0),
        Kind::ExactRemainderU64 { .. } => (Family::ExactRemainderU64, 4, 0..=0),
        Kind::WrappingRemainderI64 { .. } => (Family::WrappingRemainderI64, 4, 0..=0),
        Kind::WrappingDivideI64 { .. } => (Family::WrappingDivideI64, 4, 0..=0),
        Kind::ExactDivideI64 { .. } => (Family::ExactDivideI64, 4, 0..=0),
        Kind::ExactRemainderI64 { .. } => (Family::ExactRemainderI64, 4, 0..=0),
        // The carrier's realization shape fixes the operand count: the u64
        // add and every unsigned subtract are three-operand forms, everything
        // else carries the early-clobber scratch or the fixed RDX input.
        Kind::SaturatingAdd { carrier } => (
            Family::SaturatingAdd(carrier),
            SaturatingForm::of(SaturatingOperation::Add, carrier).operand_count(),
            0..=0,
        ),
        Kind::SaturatingSubtract { carrier } => (
            Family::SaturatingSubtract(carrier),
            SaturatingForm::of(SaturatingOperation::Subtract, carrier).operand_count(),
            0..=0,
        ),
        Kind::SaturatingDivide { carrier, .. } => (
            Family::SaturatingDivide(carrier),
            SaturatingForm::of(SaturatingOperation::Divide, carrier).operand_count(),
            0..=0,
        ),
        Kind::SaturatingRemainder { carrier, .. } => (
            Family::SaturatingRemainder(carrier),
            SaturatingForm::of(SaturatingOperation::Remainder, carrier).operand_count(),
            0..=0,
        ),
        Kind::SaturatingMultiply { carrier } => (
            Family::SaturatingMultiply(carrier),
            SaturatingForm::of(SaturatingOperation::Multiply, carrier).operand_count(),
            0..=0,
        ),
        // A Trapping form's row fixes its operand count: the conversions read
        // one input, every other form two, and each defines a result and a
        // scratch (RDX on the fixed divide row).
        Kind::TrappingInteger { form } => (
            Family::TrappingInteger(form),
            TrappingShape::of(form).operand_count(),
            0..=0,
        ),
        Kind::BitwiseXorI64 => (Family::BitwiseXorI64, 3, 0..=0),
        Kind::ExactSubtractI64 { .. } => (Family::ExactSubtractI64, 3, 0..=3),
        Kind::WrappingSubtractI64 => (Family::WrappingSubtractI64, 3, 0..=3),
        Kind::ExactMultiplyI64 { .. } => (Family::ExactMultiplyI64, 3, 0..=3),
        Kind::WrappingMultiplyI64 => (Family::WrappingMultiplyI64, 3, 0..=3),
        // The pinned-CL realization has a single register form: the
        // early-clobber result is always copied from the value before the
        // variable-count shift executes.
        Kind::WrappingShiftLeftI64 => (Family::WrappingShiftLeftI64, 3, 0..=0),
        Kind::WrappingShiftRightI64 => (Family::WrappingShiftRightI64, 3, 0..=0),
        Kind::WrappingShiftRightU64 => (Family::WrappingShiftRightU64, 3, 0..=0),
        Kind::ExactShiftLeftI64 { .. } => (Family::ExactShiftLeftI64, 3, 0..=0),
        Kind::ExactShiftRightI64 { .. } => (Family::ExactShiftRightI64, 3, 0..=0),
        Kind::ExactShiftRightU64 { .. } => (Family::ExactShiftRightU64, 3, 0..=0),
        Kind::ExactAddI64Immediate { .. } => (Family::ExactAddI64Immediate, 2, 0..=0),
        Kind::ExactSubtractI64Immediate { .. } => (Family::ExactSubtractI64Immediate, 2, 0..=0),
        Kind::ReturnScalar => (Family::ReturnScalar, 1, 0..=0),
        Kind::ReturnAggregate { fragment_count } => {
            if !(1..=2).contains(&fragment_count) {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            (Family::ReturnAggregate, usize::from(fragment_count), 0..=0)
        }
        Kind::ReturnUnit => (Family::ReturnUnit, 0, 0..=0),
        selected_instructions::layout_dependent_selected_transfers!()
        | selected_instructions::dedicated_selected_forms!() => {
            return Err(X86_64SelectedFormEncodingError::LayoutDependentForm);
        }
    })
}

pub(crate) fn validate_return_home(
    kind: Kind,
    registers: &[u8],
) -> Result<(), X86_64SelectedFormEncodingError> {
    if matches!(kind, Kind::ReturnScalar) && registers != [0] {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    if let Kind::ReturnAggregate { fragment_count } = kind
        && (!(1..=2).contains(&fragment_count)
            || registers != &[0, 2][..usize::from(fragment_count)])
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(())
}

pub(crate) fn resolve_scalar_registers(
    physical: &ValidatedPhysicalRegisterModel,
    kind: Kind,
    operands: &[RegisterViewId],
) -> Result<Vec<u8>, X86_64SelectedFormEncodingError> {
    // RET does not encode the returned register. The exact constraint row
    // retains its ABI class; accept only the canonical floating result home.
    if kind == Kind::ReturnScalar
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
    kind: Kind,
    alternative: MachineAlternativeKey,
    registers: &[u8],
) -> Result<(), X86_64SelectedFormEncodingError> {
    if !matches!(
        kind,
        Kind::ExactSubtractI64 { .. }
            | Kind::WrappingSubtractI64
            | Kind::ExactMultiplyI64 { .. }
            | Kind::WrappingMultiplyI64
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
