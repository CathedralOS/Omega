mod atomic;
mod byte_loops;
mod float_forms;
mod memory;
mod normalized_foreign_call;
mod packed_integer;
pub(crate) mod register_access;
mod scalar_call;

use byte_loops::{
    compare_byte_sequences, copy_bytes, encoded_scalar, scan_byte_sequence, zero_bytes,
};
use float_forms::{float_bits, float_result_policy, floating_control, ieee_float_arithmetic};

pub use branch_forms::{
    encode_x86_64_selected_i64_less_than_branch_form, encode_x86_64_selected_nonzero_branch_form,
    encode_x86_64_selected_short_nonzero_branch_form,
    encode_x86_64_selected_u64_less_than_branch_form,
    validate_x86_64_selected_i64_less_than_branch_form,
    validate_x86_64_selected_nonzero_branch_form,
    validate_x86_64_selected_short_nonzero_branch_form,
    validate_x86_64_selected_u64_less_than_branch_form,
};
pub use emit::X86_64_BYTE_LITERAL_ADDRESS_FIELD_OFFSET;
pub use hosted_write_byte::{
    encode_x86_64_selected_hosted_write_byte_form, validate_x86_64_selected_hosted_write_byte_form,
};
pub use jump::{encode_x86_64_selected_jump_form, validate_x86_64_selected_jump_form};
pub use memory::frame::{
    encode as encode_x86_64_selected_frame_form, validate as validate_x86_64_selected_frame_form,
};
pub use memory::{encode_x86_64_selected_memory_form, validate_x86_64_selected_memory_form};
pub use normalized_foreign_call::{
    X86_64CallbackAddressMaterialization, X86_64CallbackMaterializationDestination,
    X86_64NormalizedForeignCallFixup, X86_64NormalizedForeignCallFixupKind,
    X86_64NormalizedForeignCallFixupState, X86_64NormalizedForeignCallTemplateError,
    encode_x86_64_selected_normalized_foreign_call_template,
    validate_x86_64_selected_normalized_foreign_call_template,
};
pub use scalar_call::{
    X86_64ScalarCallFixup, X86_64ScalarCallFixupKind, X86_64ScalarCallFixupState,
    X86_64ScalarCallTemplateError, encode_x86_64_selected_dynamic_call_template,
    encode_x86_64_selected_scalar_call_template, validate_x86_64_selected_scalar_call_template,
};
pub use table_call::{
    encode_x86_64_selected_table_call_form, validate_x86_64_selected_table_call_form,
};

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstructionKind, is_float_bit_transfer,
};

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
    InstalledTable(Box<crate::X86InstalledTableEncodingError>),
    UnsupportedAtomicForm,
    NonCanonicalPhysicalModel,
    LayoutDependentForm,
    AlternativeMismatch,
    OperandCountMismatch,
    UnknownOrNonGpr64View(RegisterViewId),
    IntegerOutsideI64Bits,
    ImmediateOutsideU12,
    MemoryDisplacementOutsideI32,
    BranchDisplacementOutsideI32,
    MalformedEncoding,
    EncodedFormMismatch,
    BranchDisplacementOutsideI8,
}

wire::invalid_error_display!(
    X86_64SelectedFormEncodingError,
    "x86-64 selected-form encoding"
);

pub(super) fn canonical_effects_encoding(
    physical: &ValidatedPhysicalRegisterModel,
    operands: &[RegisterViewId],
    bytes: &[u8],
    read_count: usize,
    memory: MachineEncodedMemoryEffect,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let flags = physical
        .model()
        .view_named("condition_flags")
        .ok_or(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel)?;
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: X86_64SelectedFormFootprint {
            register_reads: operands[..read_count].to_vec(),
            register_writes: operands[read_count..].to_vec(),
            writes_rflags: true,
            encoded: MachineEncodedEffects {
                external_operand_reads: (0..read_count as u16).collect(),
                external_operand_writes: (read_count as u16..operands.len() as u16).collect(),
                implicit_unit_uses: Vec::new(),
                implicit_unit_defs: Vec::new(),
                implicit_unit_clobbers: flags.units.clone(),
                memory,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::FallThroughV1,
            },
        },
    })
}

pub(crate) mod emit {
    use super::X86_64SelectedFormEncodingError as Error;

    pub const X86_64_BYTE_LITERAL_ADDRESS_FIELD_OFFSET: u16 = 3;

    pub(crate) const fn rex(w: bool, r: u8, x: u8, b: u8) -> u8 {
        0x40 | (if w { 8 } else { 0 }) | ((r >> 3) << 2) | ((x >> 3) << 1) | (b >> 3)
    }

    pub(crate) const fn rex64(r: u8, x: u8, b: u8) -> u8 {
        rex(true, r, x, b)
    }

    pub(crate) const fn modrm(mode: u8, reg: u8, rm: u8) -> u8 {
        (mode << 6) | ((reg & 7) << 3) | (rm & 7)
    }

    pub(crate) fn rr64(bytes: &mut Vec<u8>, opcode: u8, reg: u8, rm: u8) {
        bytes.extend([rex64(reg, 0, rm), opcode, modrm(3, reg, rm)]);
    }

    pub(crate) fn append_self_clear(bytes: &mut Vec<u8>, reg: u8) {
        rr64(bytes, 0x31, reg, reg);
    }

    pub(crate) fn append_inc(bytes: &mut Vec<u8>, reg: u8) {
        bytes.extend([rex64(0, 0, reg), 0xff, modrm(3, 0, reg)]);
    }

    pub(crate) fn append_self_test(bytes: &mut Vec<u8>, reg: u8) {
        rr64(bytes, 0x85, reg, reg);
    }

    pub(crate) fn append_indexed_byte_load(
        bytes: &mut Vec<u8>,
        value: u8,
        base: u8,
        index: u8,
        offset: u32,
    ) {
        bytes.extend([
            rex64(value, index, base),
            0x0f,
            0xb6,
            modrm(2, value, 4),
            ((index & 7) << 3) | (base & 7),
        ]);
        bytes.extend(offset.to_le_bytes());
    }

    pub(crate) fn append_boolean(bytes: &mut Vec<u8>, dst: u8, condition: u8) {
        bytes.extend([
            0x40 | (dst >> 3),
            0x0f,
            0x90 | condition,
            0xc0 | (dst & 7),
            0x40 | ((dst >> 3) << 2) | (dst >> 3),
            0x0f,
            0xb6,
            0xc0 | ((dst & 7) << 3) | (dst & 7),
        ]);
    }

    pub(crate) fn append_jcc_rel32<L>(
        bytes: &mut Vec<u8>,
        cc: u8,
        target: L,
        patches: &mut Vec<(usize, L)>,
    ) {
        bytes.extend([0x0f, cc]);
        patches.push((bytes.len(), target));
        bytes.extend([0; 4]);
    }

    pub(crate) fn append_jmp_rel32<L>(
        bytes: &mut Vec<u8>,
        target: L,
        patches: &mut Vec<(usize, L)>,
    ) {
        bytes.push(0xe9);
        patches.push((bytes.len(), target));
        bytes.extend([0; 4]);
    }

    pub(crate) fn assemble_labeled_stream<L: Copy + Eq, I: Copy>(
        stream: &[(Option<L>, I)],
        emit: impl Fn(&mut Vec<u8>, I, &mut Vec<(usize, L)>),
    ) -> Result<Vec<u8>, Error> {
        let mut bytes = Vec::new();
        let mut labels: Vec<(L, usize)> = Vec::new();
        let mut patches = Vec::new();
        for &(label, insn) in stream {
            if let Some(label) = label {
                labels.push((label, bytes.len()));
            }
            emit(&mut bytes, insn, &mut patches);
        }
        for &(offset, target) in &patches {
            let Some(&(_, position)) = labels.iter().find(|&&(label, _)| label == target) else {
                return Err(Error::EncodedFormMismatch);
            };
            let rel = i32::try_from(position as i64 - (offset as i64 + 4))
                .map_err(|_| Error::MalformedEncoding)?;
            crate::codec_bytes::patch_encoded(&mut bytes, offset, rel.to_le_bytes());
        }
        Ok(bytes)
    }

    pub(crate) const fn byte_literal_address_bytes(reg: u8) -> [u8; 7] {
        [
            0x48 | ((reg >> 3) << 2),
            0x8d,
            0x05 | ((reg & 7) << 3),
            0,
            0,
            0,
            0,
        ]
    }
}

pub(crate) mod request_validation {
    use super::X86_64SelectedFormEncodingError as Error;
    use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        MachineAlternativeFamily as Family, MachineAlternativeKey, SaturatingOperation,
        SelectedInstructionKind as Kind,
    };
    use target_x86_64::form_effects::saturating_forms::SaturatingForm;
    use target_x86_64::form_effects::trapping_forms::TrappingShape;

    pub(crate) fn guard_canonical(physical: &ValidatedPhysicalRegisterModel) -> Result<(), Error> {
        if physical.identity() != crate::canonical_x86_64_physical_register_model_identity() {
            return Err(Error::NonCanonicalPhysicalModel);
        }
        Ok(())
    }

    pub(super) fn byte_loop_registers<const N: usize>(
        physical: &ValidatedPhysicalRegisterModel,
        requested: MachineAlternativeKey,
        family: Family,
        operands: &[RegisterViewId],
        preserved: usize,
    ) -> Result<[u8; N], Error> {
        guard_canonical(physical)?;
        if requested != (MachineAlternativeKey { family, variant: 0 }) {
            return Err(Error::EncodedFormMismatch);
        }
        let regs: [u8; N] = resolve_registers(physical, operands)?
            .try_into()
            .map_err(|_| Error::OperandCountMismatch)?;
        for pos in preserved..N {
            if regs[..pos].contains(&regs[pos]) {
                return Err(Error::EncodedFormMismatch);
            }
        }
        Ok(regs)
    }

    pub(crate) fn validate_request(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<(), Error> {
        guard_canonical(physical)?;
        let (family, count, max_variant) = form_shape(kind)?;
        if alternative.family != family || alternative.variant > max_variant {
            return Err(Error::AlternativeMismatch);
        }
        if operands.len() != count {
            return Err(Error::OperandCountMismatch);
        }
        Ok(())
    }

    fn form_shape(kind: Kind) -> Result<(Family, usize, u32), Error> {
        Ok(match kind {
            Kind::InstalledInterruptCpuCheck { .. } => (Family::InstalledInterruptCpuCheck, 4, 0),
            Kind::InstalledInterruptTaskCommit { .. } => {
                (Family::InstalledInterruptTaskCommit, 3, 0)
            }
            Kind::InstalledInterruptTablePublication { .. } => {
                (Family::InstalledInterruptTablePublication, 2, 0)
            }
            Kind::RegisterAccess { operation } => (
                super::register_access::family(operation)?,
                operation.input_carriers().len() + usize::from(operation.produces_value()),
                0,
            ),
            Kind::AccessorScalarAccess { form, .. } => (Family::AccessorScalarAccess(form), 2, 0),
            Kind::FloatResultPolicy { form } => {
                (Family::FloatResultPolicy(form), form.operand_count() + 1, 0)
            }
            Kind::AtomicAccess { form } => (
                Family::AtomicAccess(form),
                usize::from(form.operand_count()),
                0,
            ),
            Kind::NearestIeeeFloatArithmetic { form } => (
                Family::NearestIeeeFloatArithmetic(form),
                form.operand_count() + 1,
                0,
            ),
            Kind::PortWrite { .. } => (Family::PortWrite, 0, 0),
            Kind::AtomicFence => (Family::AtomicFence, 0, 0),
            Kind::Crash => (Family::Crash, 0, 0),
            Kind::PrivilegedCrash => (Family::PrivilegedCrash, 0, 0),
            Kind::CompareI64Zero => (Family::CompareI64Zero, 1, 0),
            Kind::CompareI64 => (Family::CompareI64, 2, 0),
            Kind::CompareI64Immediate { .. } => (Family::CompareI64Immediate, 1, 0),
            Kind::MaterializeI64 { .. } => (Family::MaterializeI64, 1, 1),
            Kind::ImageAddress { .. } => (Family::ImageAddress, 1, 0),
            Kind::CopyI64 => (Family::CopyI64, 2, 0),
            Kind::MaterializeBooleanEqual => (Family::MaterializeBooleanEqual, 1, 0),
            Kind::MaterializeBooleanU64LessThan => (Family::MaterializeBooleanU64LessThan, 1, 0),
            Kind::MaterializeBooleanI64LessThan => (Family::MaterializeBooleanI64LessThan, 1, 0),
            Kind::MaterializeBooleanU64LessOrEqual => {
                (Family::MaterializeBooleanU64LessOrEqual, 1, 0)
            }
            Kind::MaterializeBooleanI64LessOrEqual => {
                (Family::MaterializeBooleanI64LessOrEqual, 1, 0)
            }
            Kind::ZeroExtendU8 => (Family::ZeroExtendU8, 2, 0),
            Kind::ZeroExtendU16 => (Family::ZeroExtendU16, 2, 0),
            Kind::SignExtendI8 => (Family::SignExtendI8, 2, 0),
            Kind::SignExtendI16 => (Family::SignExtendI16, 2, 0),
            Kind::SignExtendI32 => (Family::SignExtendI32, 2, 0),
            Kind::ZeroExtendU32 => (Family::ZeroExtendU32, 2, 0),
            Kind::ByteViewAddress => (Family::ByteViewAddress, 3, 0),
            Kind::ExactAddI64 { .. } => (Family::ExactAddI64, 3, 0),
            Kind::WrappingAddI64 => (Family::WrappingAddI64, 3, 0),
            Kind::BitwiseAndI64 => (Family::BitwiseAndI64, 3, 0),
            Kind::BitwiseOrI64 => (Family::BitwiseOrI64, 3, 0),
            Kind::BitwiseXorI64 => (Family::BitwiseXorI64, 3, 0),
            Kind::BitwiseNotI64 => (Family::BitwiseNotI64, 2, 0),
            Kind::ExactDivideU64 { .. } => (Family::ExactDivideU64, 4, 0),
            Kind::ExactRemainderU64 { .. } => (Family::ExactRemainderU64, 4, 0),
            Kind::WrappingRemainderI64 { .. } => (Family::WrappingRemainderI64, 4, 0),
            Kind::WrappingDivideI64 { .. } => (Family::WrappingDivideI64, 4, 0),
            Kind::ExactDivideI64 { .. } => (Family::ExactDivideI64, 4, 0),
            Kind::ExactRemainderI64 { .. } => (Family::ExactRemainderI64, 4, 0),
            Kind::SaturatingAdd { carrier } => (
                Family::SaturatingAdd(carrier),
                SaturatingForm::of(SaturatingOperation::Add, carrier).operand_count(),
                0,
            ),
            Kind::SaturatingSubtract { carrier } => (
                Family::SaturatingSubtract(carrier),
                SaturatingForm::of(SaturatingOperation::Subtract, carrier).operand_count(),
                0,
            ),
            Kind::SaturatingDivide { carrier, .. } => (
                Family::SaturatingDivide(carrier),
                SaturatingForm::of(SaturatingOperation::Divide, carrier).operand_count(),
                0,
            ),
            Kind::SaturatingRemainder { carrier, .. } => (
                Family::SaturatingRemainder(carrier),
                SaturatingForm::of(SaturatingOperation::Remainder, carrier).operand_count(),
                0,
            ),
            Kind::SaturatingMultiply { carrier } => (
                Family::SaturatingMultiply(carrier),
                SaturatingForm::of(SaturatingOperation::Multiply, carrier).operand_count(),
                0,
            ),
            Kind::TrappingInteger { form } => (
                Family::TrappingInteger(form),
                TrappingShape::of(form).operand_count(),
                0,
            ),
            Kind::ExactSubtractI64 { .. } => (Family::ExactSubtractI64, 3, 3),
            Kind::WrappingSubtractI64 => (Family::WrappingSubtractI64, 3, 3),
            Kind::ExactMultiplyI64 { .. } => (Family::ExactMultiplyI64, 3, 3),
            Kind::WrappingMultiplyI64 => (Family::WrappingMultiplyI64, 3, 3),
            Kind::WrappingShiftLeftI64 => (Family::WrappingShiftLeftI64, 3, 0),
            Kind::WrappingShiftRightI64 => (Family::WrappingShiftRightI64, 3, 0),
            Kind::WrappingShiftRightU64 => (Family::WrappingShiftRightU64, 3, 0),
            Kind::ExactShiftLeftI64 { .. } => (Family::ExactShiftLeftI64, 3, 0),
            Kind::ExactShiftRightI64 { .. } => (Family::ExactShiftRightI64, 3, 0),
            Kind::ExactShiftRightU64 { .. } => (Family::ExactShiftRightU64, 3, 0),
            Kind::ExactAddI64Immediate { .. } => (Family::ExactAddI64Immediate, 2, 0),
            Kind::ExactSubtractI64Immediate { .. } => (Family::ExactSubtractI64Immediate, 2, 0),
            Kind::ReturnScalar => (Family::ReturnScalar, 1, 0),
            Kind::ReturnAggregate { fragment_count } => {
                if !(1..=2).contains(&fragment_count) {
                    return Err(Error::EncodedFormMismatch);
                }
                (Family::ReturnAggregate, usize::from(fragment_count), 0)
            }
            Kind::ReturnUnit => (Family::ReturnUnit, 0, 0),
            selected_instructions::layout_dependent_selected_transfers!()
            | selected_instructions::dedicated_selected_forms!() => {
                return Err(Error::LayoutDependentForm);
            }
        })
    }

    pub(crate) fn validate_return_home(kind: Kind, regs: &[u8]) -> Result<(), Error> {
        if matches!(kind, Kind::ReturnScalar) && regs != [0] {
            return Err(Error::EncodedFormMismatch);
        }
        if let Kind::ReturnAggregate { fragment_count } = kind
            && (!(1..=2).contains(&fragment_count)
                || regs != &[0, 2][..usize::from(fragment_count)])
        {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(())
    }

    pub(crate) fn resolve_scalar_registers(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        operands: &[RegisterViewId],
    ) -> Result<Vec<u8>, Error> {
        if kind == Kind::ReturnScalar
            && physical
                .model()
                .view_named("xmm0")
                .is_some_and(|v| operands == [v.id])
        {
            return Ok(vec![0]);
        }
        resolve_registers(physical, operands)
    }

    pub(crate) fn resolve_registers(
        physical: &ValidatedPhysicalRegisterModel,
        operands: &[RegisterViewId],
    ) -> Result<Vec<u8>, Error> {
        const NAMES: [&str; 16] = [
            "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10", "r11",
            "r12", "r13", "r14", "r15",
        ];
        operands
            .iter()
            .map(|&id| {
                let (code, view) = NAMES
                    .iter()
                    .enumerate()
                    .find_map(|(code, name)| {
                        physical
                            .model()
                            .view_named(name)
                            .filter(|v| v.id == id)
                            .map(|v| (code as u8, v))
                    })
                    .ok_or(Error::UnknownOrNonGpr64View(id))?;
                if code == 4 || view.bits != 64 || !view.allocatable {
                    return Err(Error::UnknownOrNonGpr64View(id));
                }
                Ok(code)
            })
            .collect()
    }

    pub(crate) fn validate_alias_partition(
        kind: Kind,
        alternative: MachineAlternativeKey,
        regs: &[u8],
    ) -> Result<(), Error> {
        if !matches!(
            kind,
            Kind::ExactSubtractI64 { .. }
                | Kind::WrappingSubtractI64
                | Kind::ExactMultiplyI64 { .. }
                | Kind::WrappingMultiplyI64
        ) {
            return Ok(());
        }
        let &[l, r, dst] = regs else {
            return Err(Error::OperandCountMismatch);
        };
        let expected = match (dst == l, dst == r) {
            (true, true) => 0,
            (true, false) => 1,
            (false, true) => 2,
            (false, false) => 3,
        };
        if alternative.variant != expected {
            return Err(Error::AlternativeMismatch);
        }
        Ok(())
    }
}

mod scalar_forms {
    use super::emit::{
        append_boolean, append_self_clear, append_self_test, byte_literal_address_bytes, modrm,
        rex, rex64, rr64,
    };
    use super::{X86_64SelectedFormEncodingError as Error, X86_64SelectedFormFootprint};
    use crate::register_model::units_of;
    use register_model::RegisterViewId;
    use selected_instructions::{
        MachineAlternativeKey, MachineEncodedControlEffect, MachineEncodedEffects,
        MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
        SaturatingCarrier, SaturatingOperation, SelectedInstructionKind as Kind, TrappingForm,
        dedicated_selected_forms, exact_add_or_sub_pair_pattern, exact_shift_i64_pair_pattern,
        exact_wrapping_divrem_set_pattern, layout_dependent_selected_transfers,
        saturating_add_or_subtract_pattern,
    };
    use semantic_vocabulary::IntegerValue;
    use target_x86_64::form_effects::saturating_forms::SaturatingForm;
    use target_x86_64::form_effects::trapping_forms::{
        DividendGuard, RangeCheck, TrappingArithmetic, TrappingShape,
    };

    const CMOVG: u8 = 0x4f;
    const CMOVL: u8 = 0x4c;
    const CMOVA: u8 = 0x47;
    const JNO: u8 = 0x71;
    const JB: u8 = 0x72;
    const JAE: u8 = 0x73;
    const JE: u8 = 0x74;
    const JNE: u8 = 0x75;

    fn integer_bits(value: IntegerValue) -> Result<u64, Error> {
        match value {
            IntegerValue::Signed(v) => i64::try_from(v)
                .map(|v| v as u64)
                .map_err(|_| Error::IntegerOutsideI64Bits),
            IntegerValue::Unsigned(v) => u64::try_from(v).map_err(|_| Error::IntegerOutsideI64Bits),
        }
    }

    fn u12(value: IntegerValue) -> Result<u32, Error> {
        match value {
            IntegerValue::Unsigned(v) if v <= 4095 => Ok(v as u32),
            _ => Err(Error::ImmediateOutsideU12),
        }
    }

    fn sat_op(kind: Kind) -> Option<(SaturatingOperation, SaturatingCarrier)> {
        Some(match kind {
            Kind::SaturatingAdd { carrier } => (SaturatingOperation::Add, carrier),
            Kind::SaturatingSubtract { carrier } => (SaturatingOperation::Subtract, carrier),
            Kind::SaturatingDivide { carrier, .. } => (SaturatingOperation::Divide, carrier),
            Kind::SaturatingRemainder { carrier, .. } => (SaturatingOperation::Remainder, carrier),
            Kind::SaturatingMultiply { carrier } => (SaturatingOperation::Multiply, carrier),
            _ => return None,
        })
    }

    fn append_clamp(bytes: &mut Vec<u8>, val: u8, scratch: u8, bound: u64, cc: u8) {
        bytes.extend([0x48 | (scratch >> 3), 0xb8 | (scratch & 7)]);
        bytes.extend(bound.to_le_bytes());
        rr64(bytes, 0x39, scratch, val);
        bytes.extend([rex64(val, 0, scratch), 0x0f, cc, modrm(3, val, scratch)]);
    }

    fn append_imul(bytes: &mut Vec<u8>, src: u8, dst: u8) {
        bytes.extend([rex64(dst, 0, src), 0x0f, 0xaf, modrm(3, dst, src)]);
    }

    fn append_cmovo(bytes: &mut Vec<u8>, src: u8, dst: u8) {
        bytes.extend([rex64(dst, 0, src), 0x0f, 0x40, modrm(3, dst, src)]);
    }

    fn append_idiv(bytes: &mut Vec<u8>, divisor: u8) {
        bytes.extend([0x48, 0x99, rex64(0, 0, divisor), 0xf7, modrm(3, 7, divisor)]);
    }

    fn append_trap_unless(bytes: &mut Vec<u8>, cc: u8) {
        bytes.extend([cc, 2, 0x0f, 0x0b]);
    }

    fn append_lea_reg(bytes: &mut Vec<u8>, left: u8, right: u8, dst: u8) {
        let (base, index) = if left & 7 == 5 && right & 7 != 5 {
            (right, left)
        } else {
            (left, right)
        };
        let disp0 = base & 7 == 5;
        bytes.extend([
            rex64(dst, index, base),
            0x8d,
            modrm(u8::from(disp0), dst, 4),
            ((index & 7) << 3) | (base & 7),
        ]);
        if disp0 {
            bytes.push(0);
        }
    }

    fn append_lea_imm(bytes: &mut Vec<u8>, base: u8, dst: u8, disp: i32) {
        let d8 = i8::try_from(disp).is_ok();
        let sib = base & 7 == 4;
        bytes.extend([
            rex64(dst, 0, base),
            0x8d,
            modrm(if d8 { 1 } else { 2 }, dst, if sib { 4 } else { base }),
        ]);
        if sib {
            bytes.push(0x20 | (base & 7));
        }
        if d8 {
            bytes.push(disp as i8 as u8);
        } else {
            bytes.extend(disp.to_le_bytes());
        }
    }

    fn append_range_check(bytes: &mut Vec<u8>, check: RangeCheck, val: u8, scratch: u8) {
        match check {
            RangeCheck::SignExtension { bits } => {
                bytes.push(rex64(scratch, 0, val));
                match bits {
                    8 => bytes.extend([0x0f, 0xbe]),
                    16 => bytes.extend([0x0f, 0xbf]),
                    _ => bytes.push(0x63),
                }
                bytes.push(modrm(3, scratch, val));
                rr64(bytes, 0x39, val, scratch);
            }
            RangeCheck::HighBits { shift } => {
                rr64(bytes, 0x89, val, scratch);
                bytes.extend([rex64(0, 0, scratch), 0xc1, modrm(3, 5, scratch), shift]);
            }
        }
        append_trap_unless(bytes, JE);
    }

    fn append_saturating(
        bytes: &mut Vec<u8>,
        op: SaturatingOperation,
        carrier: SaturatingCarrier,
        regs: &[u8],
    ) -> Result<(), Error> {
        let form = SaturatingForm::of(op, carrier);
        if !form.accepts_registers(regs) {
            return Err(Error::EncodedFormMismatch);
        }
        let (l, r, res) = (regs[0], regs[1], regs[2]);
        let alu = if op == SaturatingOperation::Add {
            0x01
        } else {
            0x29
        };
        match form {
            SaturatingForm::SubtractUnsigned => {
                rr64(bytes, 0x39, r, l);
                rr64(bytes, 0x89, l, res);
                bytes.extend([rex64(res, 0, r), 0x0f, 0x42, modrm(3, res, r)]);
                rr64(bytes, 0x29, r, res);
            }
            SaturatingForm::AddU64 => {
                rr64(bytes, 0x89, l, res);
                bytes.extend([rex64(0, 0, res), 0xf7, modrm(3, 2, res)]);
                rr64(bytes, 0x39, r, res);
                bytes.extend([rex64(res, 0, r), 0x0f, 0x42, modrm(3, res, r)]);
                rr64(bytes, 0x29, r, res);
                bytes.extend([rex64(0, 0, res), 0xf7, modrm(3, 2, res)]);
            }
            SaturatingForm::ClampSignedNarrow | SaturatingForm::ClampUnsignedNarrow => {
                let s = regs[3];
                rr64(bytes, 0x89, l, res);
                rr64(bytes, alu, r, res);
                append_clamp(bytes, res, s, carrier.maximum_bits(), CMOVG);
                if form == SaturatingForm::ClampSignedNarrow {
                    append_clamp(bytes, res, s, carrier.minimum_bits(), CMOVL);
                }
            }
            SaturatingForm::OverflowSelectI64 => {
                let s = regs[3];
                rr64(bytes, 0x89, l, s);
                bytes.extend([
                    rex64(0, 0, s),
                    0xf7,
                    modrm(3, 2, s),
                    rex64(0, 0, s),
                    0xc1,
                    modrm(3, 7, s),
                    63,
                    rex64(0, 0, s),
                    0x0f,
                    0xba,
                    modrm(3, 7, s),
                    63,
                ]);
                rr64(bytes, 0x89, l, res);
                rr64(bytes, alu, r, res);
                append_cmovo(bytes, s, res);
            }
            SaturatingForm::DivideUnsigned => {
                bytes.extend([rex64(0, 0, r), 0xf7, modrm(3, 6, r)]);
            }
            SaturatingForm::DivideSignedNarrow => {
                append_idiv(bytes, r);
                append_clamp(bytes, 0, 2, carrier.maximum_bits(), CMOVG);
            }
            SaturatingForm::DivideI64 => {
                bytes.extend([rex64(0, 0, r), 0x83, modrm(3, 7, r), 0xff]);
                rr64(bytes, 0x19, 2, 2);
                rr64(bytes, 0x09, 0, 2);
                bytes.extend([rex64(0, 0, 2), 0xf7, modrm(3, 3, 2)]);
                append_lea_imm(bytes, 0, 2, 1);
                append_cmovo(bytes, 2, 0);
                append_idiv(bytes, r);
            }
            SaturatingForm::RemainderUnsigned => {
                rr64(bytes, 0x31, regs[3], regs[3]);
                bytes.extend([rex64(0, 0, r), 0xf7, modrm(3, 6, r)]);
                rr64(bytes, 0x89, regs[3], res);
            }
            SaturatingForm::RemainderSigned => {
                rr64(bytes, 0x31, regs[3], regs[3]);
                bytes.extend([
                    rex64(0, 0, r),
                    0x83,
                    modrm(3, 7, r),
                    0xff,
                    0x74,
                    5,
                    0x48,
                    0x99,
                    rex64(0, 0, r),
                    0xf7,
                    modrm(3, 7, r),
                ]);
                rr64(bytes, 0x89, regs[3], res);
            }
            SaturatingForm::MultiplySignedNarrow | SaturatingForm::MultiplyUnsignedNarrow => {
                let s = regs[3];
                rr64(bytes, 0x89, l, res);
                append_imul(bytes, r, res);
                if form == SaturatingForm::MultiplySignedNarrow {
                    append_clamp(bytes, res, s, carrier.maximum_bits(), CMOVG);
                    append_clamp(bytes, res, s, carrier.minimum_bits(), CMOVL);
                } else {
                    append_clamp(bytes, res, s, carrier.maximum_bits(), CMOVA);
                }
            }
            SaturatingForm::MultiplyI64 => {
                let s = regs[3];
                rr64(bytes, 0x89, l, s);
                rr64(bytes, 0x31, r, s);
                bytes.extend([
                    rex64(0, 0, s),
                    0xf7,
                    modrm(3, 2, s),
                    rex64(0, 0, s),
                    0xc1,
                    modrm(3, 7, s),
                    63,
                    rex64(0, 0, s),
                    0x0f,
                    0xba,
                    modrm(3, 7, s),
                    63,
                ]);
                rr64(bytes, 0x89, l, res);
                append_imul(bytes, r, res);
                append_cmovo(bytes, s, res);
            }
            SaturatingForm::MultiplyU64 => {
                bytes.extend([rex64(0, 0, r), 0xf7, modrm(3, 4, r)]);
                rr64(bytes, 0x19, 2, 2);
                rr64(bytes, 0x09, 2, 0);
            }
        }
        Ok(())
    }

    fn append_trapping(bytes: &mut Vec<u8>, form: TrappingForm, regs: &[u8]) -> Result<(), Error> {
        let shape = TrappingShape::of(form);
        if !shape.accepts_registers(regs) {
            return Err(Error::EncodedFormMismatch);
        }
        let arith = |bytes: &mut Vec<u8>, a: TrappingArithmetic| {
            rr64(bytes, 0x89, regs[0], regs[2]);
            match a {
                TrappingArithmetic::Add => rr64(bytes, 0x01, regs[1], regs[2]),
                TrappingArithmetic::Subtract => rr64(bytes, 0x29, regs[1], regs[2]),
                TrappingArithmetic::Multiply => append_imul(bytes, regs[1], regs[2]),
            }
        };
        let sh_check = |bytes: &mut Vec<u8>, width: u8| {
            bytes.extend([rex64(0, 0, regs[1]), 0x83, modrm(3, 7, regs[1]), width]);
            append_trap_unless(bytes, JB);
            rr64(bytes, 0x89, regs[0], regs[2]);
        };
        let sh_cl = |bytes: &mut Vec<u8>, digit: u8, dst: u8| {
            bytes.extend([rex64(0, 0, dst), 0xd3, modrm(3, digit, dst)]);
        };
        match shape {
            TrappingShape::OverflowFlag(a) => {
                arith(bytes, a);
                append_trap_unless(bytes, JNO);
            }
            TrappingShape::CarryFlag(a) => {
                arith(bytes, a);
                append_trap_unless(bytes, JAE);
            }
            TrappingShape::NarrowRange(a, check) => {
                arith(bytes, a);
                append_range_check(bytes, check, regs[2], regs[3]);
            }
            TrappingShape::MultiplyHighHalf => {
                bytes.extend([rex64(0, 0, regs[1]), 0xf7, modrm(3, 4, regs[1])]);
                append_trap_unless(bytes, JNO);
            }
            TrappingShape::Division { guard, remainder } => {
                let div = regs[1];
                append_self_test(bytes, div);
                append_trap_unless(bytes, JNE);
                if let Some(guard) = guard {
                    bytes.extend([
                        rex64(0, 0, div),
                        0x83,
                        modrm(3, 7, div),
                        0xff,
                        JNE,
                        guard.byte_count() as u8,
                    ]);
                    match guard {
                        DividendGuard::Minimum8(m) => {
                            bytes.extend([0x48, 0x83, 0xf8, m as u8]);
                            append_trap_unless(bytes, JNE);
                        }
                        DividendGuard::Minimum32(m) => {
                            bytes.extend([0x48, 0x3d]);
                            bytes.extend(m.to_le_bytes());
                            append_trap_unless(bytes, JNE);
                        }
                        DividendGuard::DecrementOverflows => {
                            bytes.extend([0x48, 0x83, 0xf8, 1]);
                            append_trap_unless(bytes, JNO);
                        }
                    }
                    append_idiv(bytes, div);
                } else {
                    rr64(bytes, 0x31, 2, 2);
                    bytes.extend([rex64(0, 0, div), 0xf7, modrm(3, 6, div)]);
                }
                if remainder {
                    rr64(bytes, 0x89, 2, 0);
                }
            }
            TrappingShape::ShiftLeftRoundTrip { signed } => {
                sh_check(bytes, 64);
                sh_cl(bytes, 4, regs[2]);
                rr64(bytes, 0x89, regs[2], regs[3]);
                sh_cl(bytes, if signed { 7 } else { 5 }, regs[3]);
                rr64(bytes, 0x39, regs[0], regs[3]);
                append_trap_unless(bytes, JE);
            }
            TrappingShape::ShiftLeftRange { width, check } => {
                sh_check(bytes, width);
                sh_cl(bytes, 4, regs[2]);
                append_range_check(bytes, check, regs[2], regs[3]);
            }
            TrappingShape::ShiftRight { width, signed } => {
                sh_check(bytes, width);
                sh_cl(bytes, if signed { 7 } else { 5 }, regs[2]);
            }
            TrappingShape::Convert(check) => {
                if let Some(check) = check {
                    append_range_check(bytes, check, regs[0], regs[2]);
                }
                rr64(bytes, 0x89, regs[0], regs[1]);
            }
        }
        Ok(())
    }

    fn require_div_layout(regs: &[u8]) -> Result<(), Error> {
        if regs[0] != 0 || regs[2] != 0 || regs[3] != 2 || regs[1] == 2 {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(())
    }

    pub(crate) fn encode_unchecked(
        kind: Kind,
        alternative: MachineAlternativeKey,
        regs: &[u8],
    ) -> Result<Vec<u8>, Error> {
        if let Some((op, carrier)) = sat_op(kind) {
            let mut bytes = Vec::new();
            append_saturating(&mut bytes, op, carrier, regs)?;
            return Ok(bytes);
        }
        let mut bytes = Vec::new();
        match kind {
            Kind::InstalledInterruptCpuCheck { .. }
            | Kind::InstalledInterruptTaskCommit { .. }
            | Kind::InstalledInterruptTablePublication { .. }
            | Kind::AccessorScalarAccess { .. }
            | Kind::NearestIeeeFloatArithmetic { .. }
            | Kind::FloatResultPolicy { .. } => return Err(Error::EncodedFormMismatch),
            Kind::RegisterAccess { operation } => {
                return super::register_access::encode(operation, regs);
            }
            Kind::AtomicAccess { form } => return super::atomic::encode(form, regs),
            Kind::PortWrite { port, value, .. } => {
                bytes.extend_from_slice(&x86_64_encoding::encode_immediate_port_write(port, value));
            }
            Kind::AtomicFence => bytes.extend([0x0f, 0xae, 0xf0]),
            Kind::Crash => bytes.extend([0x0f, 0x0b]),
            Kind::PrivilegedCrash => bytes.extend(crate::PRIVILEGED_STOP_BYTES),
            Kind::ReturnScalar | Kind::ReturnAggregate { .. } | Kind::ReturnUnit => {
                bytes.extend(crate::ORDINARY_RETURN_BYTES);
            }
            Kind::MaterializeI64 { value } if alternative.variant == 1 => {
                if integer_bits(value)? != 0 {
                    return Err(Error::AlternativeMismatch);
                }
                append_self_clear(&mut bytes, regs[0]);
            }
            Kind::MaterializeI64 { value } => {
                bytes.extend([0x48 | (regs[0] >> 3), 0xb8 | (regs[0] & 7)]);
                bytes.extend(integer_bits(value)?.to_le_bytes());
            }
            Kind::ImageAddress { .. } => bytes.extend(byte_literal_address_bytes(regs[0])),
            Kind::MaterializeBooleanEqual
            | Kind::MaterializeBooleanU64LessThan
            | Kind::MaterializeBooleanI64LessThan
            | Kind::MaterializeBooleanU64LessOrEqual
            | Kind::MaterializeBooleanI64LessOrEqual => {
                let cc = match kind {
                    Kind::MaterializeBooleanEqual => 4,
                    Kind::MaterializeBooleanU64LessThan => 2,
                    Kind::MaterializeBooleanI64LessThan => 12,
                    Kind::MaterializeBooleanU64LessOrEqual => 6,
                    _ => 14,
                };
                append_boolean(&mut bytes, regs[0], cc);
            }
            Kind::ZeroExtendU8
            | Kind::ZeroExtendU16
            | Kind::SignExtendI8
            | Kind::SignExtendI16
            | Kind::SignExtendI32
            | Kind::ZeroExtendU32 => {
                let (wide, op1, op2, swap): (bool, Option<u8>, u8, bool) = match kind {
                    Kind::ZeroExtendU8 => (false, Some(0x0f), 0xb6, false),
                    Kind::ZeroExtendU16 => (false, Some(0x0f), 0xb7, false),
                    Kind::SignExtendI8 => (true, Some(0x0f), 0xbe, false),
                    Kind::SignExtendI16 => (true, Some(0x0f), 0xbf, false),
                    Kind::SignExtendI32 => (true, None, 0x63, false),
                    _ => (false, None, 0x89, true),
                };
                let (reg, rm) = if swap {
                    (regs[0], regs[1])
                } else {
                    (regs[1], regs[0])
                };
                bytes.push(rex(wide, reg, 0, rm));
                if let Some(p) = op1 {
                    bytes.push(p);
                }
                bytes.extend([op2, modrm(3, reg, rm)]);
            }
            // A copy whose source and destination got the same register is
            // already complete; `mov r, r` would only spend a slot.
            Kind::CopyI64 if regs[0] == regs[1] => {}
            Kind::CopyI64 => rr64(&mut bytes, 0x89, regs[0], regs[1]),
            Kind::CompareI64Zero => append_self_test(&mut bytes, regs[0]),
            Kind::CompareI64 => rr64(&mut bytes, 0x39, regs[1], regs[0]),
            Kind::CompareI64Immediate { immediate } => {
                bytes.extend([rex64(0, 0, regs[0]), 0x81, modrm(3, 7, regs[0])]);
                bytes.extend(u12(immediate)?.to_le_bytes());
            }
            Kind::ByteViewAddress | Kind::WrappingAddI64 | Kind::ExactAddI64 { .. } => {
                append_lea_reg(&mut bytes, regs[0], regs[1], regs[2]);
            }
            Kind::ExactDivideU64 { .. } => {
                require_div_layout(regs)?;
                bytes.extend([rex64(0, 0, regs[1]), 0xf7, modrm(3, 6, regs[1])]);
            }
            Kind::ExactRemainderU64 { .. } => {
                require_div_layout(regs)?;
                rr64(&mut bytes, 0x31, regs[3], regs[3]);
                bytes.extend([rex64(0, 0, regs[1]), 0xf7, modrm(3, 6, regs[1])]);
                rr64(&mut bytes, 0x89, regs[3], regs[2]);
            }
            Kind::WrappingDivideI64 { .. } => {
                require_div_layout(regs)?;
                bytes.extend([
                    rex64(0, 0, regs[1]),
                    0x83,
                    modrm(3, 7, regs[1]),
                    0xff,
                    0x75,
                    5,
                    rex64(0, 0, regs[2]),
                    0xf7,
                    modrm(3, 3, regs[2]),
                    0xeb,
                    5,
                ]);
                append_idiv(&mut bytes, regs[1]);
            }
            Kind::ExactDivideI64 { .. } => {
                require_div_layout(regs)?;
                append_idiv(&mut bytes, regs[1]);
            }
            Kind::ExactRemainderI64 { .. } => {
                require_div_layout(regs)?;
                append_idiv(&mut bytes, regs[1]);
                rr64(&mut bytes, 0x89, regs[3], regs[2]);
            }
            Kind::WrappingRemainderI64 { .. } => {
                require_div_layout(regs)?;
                rr64(&mut bytes, 0x31, regs[3], regs[3]);
                bytes.extend([
                    rex64(0, 0, regs[1]),
                    0x83,
                    modrm(3, 7, regs[1]),
                    0xff,
                    0x74,
                    5,
                    0x48,
                    0x99,
                    rex64(0, 0, regs[1]),
                    0xf7,
                    modrm(3, 7, regs[1]),
                ]);
                rr64(&mut bytes, 0x89, regs[3], regs[2]);
            }
            Kind::TrappingInteger { form } => append_trapping(&mut bytes, form, regs)?,
            Kind::BitwiseAndI64 | Kind::BitwiseOrI64 | Kind::BitwiseXorI64 => {
                let op = match kind {
                    Kind::BitwiseXorI64 => 0x31,
                    Kind::BitwiseOrI64 => 0x09,
                    _ => 0x21,
                };
                if regs[2] == regs[0] {
                    rr64(&mut bytes, op, regs[1], regs[2]);
                } else if regs[2] == regs[1] {
                    rr64(&mut bytes, op, regs[0], regs[2]);
                } else {
                    rr64(&mut bytes, 0x89, regs[0], regs[2]);
                    rr64(&mut bytes, op, regs[1], regs[2]);
                }
            }
            Kind::BitwiseNotI64 => {
                if regs[1] != regs[0] {
                    rr64(&mut bytes, 0x89, regs[0], regs[1]);
                }
                bytes.extend([rex64(0, 0, regs[1]), 0xf7, modrm(3, 2, regs[1])]);
            }
            Kind::ExactAddI64Immediate { immediate, .. } => {
                append_lea_imm(&mut bytes, regs[0], regs[1], u12(immediate)? as i32);
            }
            Kind::ExactSubtractI64Immediate { immediate, .. } => {
                append_lea_imm(&mut bytes, regs[0], regs[1], -(u12(immediate)? as i32));
            }
            Kind::ExactSubtractI64 { .. } | Kind::WrappingSubtractI64 => {
                match alternative.variant {
                    0 => append_self_clear(&mut bytes, regs[2]),
                    1 => rr64(&mut bytes, 0x29, regs[1], regs[2]),
                    2 => {
                        bytes.extend([rex64(0, 0, regs[2]), 0xf7, modrm(3, 3, regs[2])]);
                        rr64(&mut bytes, 0x01, regs[0], regs[2]);
                    }
                    3 => {
                        rr64(&mut bytes, 0x89, regs[0], regs[2]);
                        rr64(&mut bytes, 0x29, regs[1], regs[2]);
                    }
                    _ => return Err(Error::AlternativeMismatch),
                }
            }
            Kind::ExactMultiplyI64 { .. } | Kind::WrappingMultiplyI64 => {
                match alternative.variant {
                    0 => append_imul(&mut bytes, regs[2], regs[2]),
                    1 => append_imul(&mut bytes, regs[1], regs[2]),
                    2 => append_imul(&mut bytes, regs[0], regs[2]),
                    3 => {
                        rr64(&mut bytes, 0x89, regs[0], regs[2]);
                        append_imul(&mut bytes, regs[1], regs[2]);
                    }
                    _ => return Err(Error::AlternativeMismatch),
                }
            }
            Kind::WrappingShiftLeftI64
            | Kind::WrappingShiftRightI64
            | Kind::WrappingShiftRightU64
            | exact_shift_i64_pair_pattern!()
            | Kind::ExactShiftRightU64 { .. } => {
                if regs[1] != 1 {
                    return Err(Error::EncodedFormMismatch);
                }
                let digit = match kind {
                    Kind::WrappingShiftLeftI64 | Kind::ExactShiftLeftI64 { .. } => 4,
                    Kind::WrappingShiftRightU64 | Kind::ExactShiftRightU64 { .. } => 5,
                    _ => 7,
                };
                rr64(&mut bytes, 0x89, regs[0], regs[2]);
                bytes.extend([rex64(0, 0, regs[2]), 0xd3, modrm(3, digit, regs[2])]);
            }
            layout_dependent_selected_transfers!() | dedicated_selected_forms!() => {
                return Err(Error::LayoutDependentForm);
            }
            saturating_add_or_subtract_pattern!()
            | Kind::SaturatingDivide { .. }
            | Kind::SaturatingRemainder { .. }
            | Kind::SaturatingMultiply { .. } => unreachable!(),
        }
        Ok(bytes)
    }

    fn views(positions: &[u16], operands: &[RegisterViewId]) -> Vec<RegisterViewId> {
        positions
            .iter()
            .map(|&p| operands[usize::from(p)])
            .collect()
    }

    pub(crate) fn footprint(
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> X86_64SelectedFormFootprint {
        let physical = crate::x86_64_physical_register_model_ref();
        let units = |name: &str| physical.view_named(name).unwrap().units.clone();

        if let Some((op, carrier)) = sat_op(kind) {
            let form = SaturatingForm::of(op, carrier);
            let (reads, writes) = form.operand_reads_and_writes();
            let mut encoded = MachineEncodedEffects::fallthrough_v1(reads.clone(), writes.clone());
            encoded.implicit_unit_clobbers = units("condition_flags");
            if form.is_division() {
                if !matches!(
                    form,
                    SaturatingForm::RemainderUnsigned | SaturatingForm::RemainderSigned
                ) {
                    encoded.implicit_unit_clobbers.extend(units("rdx"));
                    encoded.implicit_unit_clobbers.sort_unstable();
                    encoded.implicit_unit_clobbers.dedup();
                }
                encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
            }
            return X86_64SelectedFormFootprint {
                register_reads: views(&reads, operands),
                register_writes: views(&writes, operands),
                writes_rflags: true,
                encoded,
            };
        }
        if let Kind::TrappingInteger { form } = kind {
            let shape = TrappingShape::of(form);
            let (reads, writes) = shape.operand_reads_and_writes();
            return X86_64SelectedFormFootprint {
                register_reads: views(&reads, operands),
                register_writes: views(&writes, operands),
                writes_rflags: true,
                encoded: shape.encoded_effects(),
            };
        }
        if let Kind::NearestIeeeFloatArithmetic { form } = kind {
            return super::ieee_float_arithmetic::footprint(form, operands);
        }
        if let Kind::AtomicAccess { form } = kind {
            return super::atomic::footprint(form, operands);
        }
        if let Kind::RegisterAccess { operation } = kind {
            return super::register_access::footprint(operation, operands);
        }
        if let Kind::FloatResultPolicy { form } = kind {
            return super::float_result_policy::footprint(form, operands);
        }
        if let Some(form) = super::installed_interrupt::Form::from_kind(kind) {
            return super::installed_interrupt::footprint(form, operands);
        }

        let (reads, writes, writes_rflags): (Vec<u16>, Vec<u16>, bool) = match kind {
            Kind::AccessorScalarAccess { form, .. } => {
                if form.write {
                    (vec![0, 1], vec![], false)
                } else {
                    (vec![0], vec![1], false)
                }
            }
            Kind::PortWrite { .. } | Kind::AtomicFence | Kind::Crash => (vec![], vec![], false),
            Kind::PrivilegedCrash => (vec![], vec![], true),
            Kind::MaterializeBooleanEqual
            | Kind::MaterializeBooleanU64LessThan
            | Kind::MaterializeBooleanI64LessThan
            | Kind::MaterializeBooleanU64LessOrEqual
            | Kind::MaterializeBooleanI64LessOrEqual
            | Kind::ImageAddress { .. } => (vec![], vec![0], false),
            Kind::MaterializeI64 { .. } => (vec![], vec![0], alternative.variant == 1),
            Kind::CopyI64
            | Kind::ZeroExtendU8
            | Kind::ZeroExtendU16
            | Kind::SignExtendI8
            | Kind::SignExtendI16
            | Kind::SignExtendI32
            | Kind::ZeroExtendU32
            | Kind::BitwiseNotI64
            | exact_add_or_sub_pair_pattern!(..) => (vec![0], vec![1], false),
            Kind::CompareI64Zero | Kind::CompareI64Immediate { .. } => (vec![0], vec![], true),
            Kind::CompareI64 => (vec![0, 1], vec![], true),
            Kind::ExactDivideU64 { .. } => (vec![0, 1, 3], vec![2], true),
            exact_wrapping_divrem_set_pattern!(..) => (vec![0, 1], vec![2, 3], true),
            Kind::ByteViewAddress | Kind::WrappingAddI64 | Kind::ExactAddI64 { .. } => {
                (vec![0, 1], vec![2], false)
            }
            Kind::ExactSubtractI64 { .. } | Kind::WrappingSubtractI64
                if alternative.variant == 0 =>
            {
                (vec![], vec![2], true)
            }
            Kind::ExactSubtractI64 { .. }
            | Kind::WrappingSubtractI64
            | Kind::ExactMultiplyI64 { .. }
            | Kind::WrappingMultiplyI64
            | Kind::WrappingShiftLeftI64
            | Kind::WrappingShiftRightI64
            | Kind::WrappingShiftRightU64
            | exact_shift_i64_pair_pattern!()
            | Kind::ExactShiftRightU64 { .. }
            | Kind::BitwiseAndI64
            | Kind::BitwiseOrI64
            | Kind::BitwiseXorI64 => (vec![0, 1], vec![2], true),
            _ => (vec![], vec![], false),
        };
        let register_reads = views(&reads, operands);
        let register_writes = views(&writes, operands);
        let encoded = match kind {
            Kind::PortWrite { .. } => super::port_write::effects(),
            Kind::AtomicFence => MachineEncodedEffects::atomic_fence_v1(),
            Kind::PrivilegedCrash => {
                MachineEncodedEffects::privileged_stop_v1(&units("rip"), &units("control_flags"))
            }
            Kind::Crash => MachineEncodedEffects::crash_v1(&units("rip")),
            Kind::ReturnScalar | Kind::ReturnAggregate { .. } | Kind::ReturnUnit => {
                let rsp = physical.view_named("rsp").unwrap().id;
                MachineEncodedEffects {
                    external_operand_reads: vec![],
                    external_operand_writes: vec![],
                    implicit_unit_uses: units("rsp"),
                    implicit_unit_defs: units_of(&["rsp", "rip"]),
                    implicit_unit_clobbers: vec![],
                    memory: MachineEncodedMemoryEffect::ReadActivationStackV1 {
                        stack_pointer: rsp,
                        byte_count: 8,
                    },
                    stack: MachineEncodedStackEffect::PopBytesV1 {
                        stack_pointer: rsp,
                        byte_count: 8,
                    },
                    trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    control: MachineEncodedControlEffect::ReturnFromActivationStackV1,
                }
            }
            Kind::ConditionalBranchNonZero
            | Kind::ConditionalBranchU64LessThan
            | Kind::ConditionalBranchI64LessThan => MachineEncodedEffects {
                external_operand_reads: vec![],
                external_operand_writes: vec![],
                implicit_unit_uses: units_of(&["condition_flags", "rip"]),
                implicit_unit_defs: units("rip"),
                implicit_unit_clobbers: vec![],
                memory: MachineEncodedMemoryEffect::NoneV1,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::ConditionalRelativeBranchV1,
            },
            _ => {
                let mut e = MachineEncodedEffects::fallthrough_v1(reads, writes);
                if matches!(
                    kind,
                    Kind::CompareI64Zero | Kind::CompareI64 | Kind::CompareI64Immediate { .. }
                ) {
                    e.implicit_unit_defs = units("condition_flags");
                }
                if matches!(
                    kind,
                    Kind::ExactSubtractI64 { .. }
                        | Kind::WrappingSubtractI64
                        | Kind::ExactMultiplyI64 { .. }
                        | Kind::WrappingMultiplyI64
                        | Kind::WrappingShiftLeftI64
                        | Kind::WrappingShiftRightI64
                        | Kind::WrappingShiftRightU64
                        | exact_shift_i64_pair_pattern!()
                        | Kind::ExactShiftRightU64 { .. }
                        | Kind::BitwiseAndI64
                        | Kind::BitwiseOrI64
                        | Kind::BitwiseXorI64
                ) || (writes_rflags && matches!(kind, Kind::MaterializeI64 { .. }))
                {
                    e.implicit_unit_clobbers = units("condition_flags");
                }
                if matches!(
                    kind,
                    Kind::MaterializeBooleanEqual
                        | Kind::MaterializeBooleanU64LessThan
                        | Kind::MaterializeBooleanI64LessThan
                        | Kind::MaterializeBooleanU64LessOrEqual
                        | Kind::MaterializeBooleanI64LessOrEqual
                ) {
                    e.implicit_unit_uses = units("condition_flags");
                }
                if matches!(kind, Kind::ExactDivideU64 { .. }) {
                    e.implicit_unit_clobbers = units_of(&["rdx", "condition_flags"]);
                    e.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
                }
                if matches!(kind, exact_wrapping_divrem_set_pattern!(..)) {
                    e.implicit_unit_clobbers = units("condition_flags");
                    e.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
                }
                e
            }
        };
        X86_64SelectedFormFootprint {
            register_reads,
            register_writes,
            writes_rflags,
            encoded,
        }
    }
}

pub(crate) mod memory_fence {
    use selected_instructions::MemoryFenceKind;

    pub(crate) fn encode(kind: MemoryFenceKind) -> Option<Vec<u8>> {
        let opcode = match kind {
            MemoryFenceKind::Load => 0xe8,
            MemoryFenceKind::Store => 0xf8,
            MemoryFenceKind::Full => 0xf0,
            _ => return None,
        };
        Some(vec![0x0f, 0xae, opcode])
    }
}

pub(crate) mod port_write {
    use super::{
        MachineAlternativeKey, RegisterViewId, SelectedInstructionKind,
        ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
        X86_64SelectedFormEncodingError as Error,
    };
    pub(crate) use target_x86_64::form_effects::port_write::effects;

    pub(super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let SelectedInstructionKind::PortWrite { port, value, .. } = kind else {
            return Err(Error::AlternativeMismatch);
        };
        super::request_validation::validate_request(physical, kind, alternative, operands)?;
        if bytes != x86_64_encoding::encode_immediate_port_write(port, value) {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: super::scalar_forms::footprint(kind, alternative, operands),
        })
    }
}

mod branch_forms {
    use super::scalar_forms::footprint;
    use super::{
        MachineAlternativeFamily as Family, MachineAlternativeKey, SelectedInstructionKind as Kind,
        ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
        X86_64SelectedFormEncodingError as Error,
    };

    #[derive(Clone, Copy)]
    struct BranchSpec {
        family: Family,
        kind: Kind,
        cc: u8,
        variants: u32,
    }

    const NONZERO: BranchSpec = BranchSpec {
        family: Family::ConditionalBranchNonZero,
        kind: Kind::ConditionalBranchNonZero,
        cc: 0x5,
        variants: 1,
    };
    const U64_LT: BranchSpec = BranchSpec {
        family: Family::ConditionalBranchU64LessThan,
        kind: Kind::ConditionalBranchU64LessThan,
        cc: 0x2,
        variants: 2,
    };
    const I64_LT: BranchSpec = BranchSpec {
        family: Family::ConditionalBranchI64LessThan,
        kind: Kind::ConditionalBranchI64LessThan,
        cc: 0xc,
        variants: 2,
    };

    #[derive(Clone, Copy)]
    enum Width {
        Near,
        Short,
        Variant,
    }

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        spec: BranchSpec,
        width: Width,
        disp: i64,
    ) -> Result<Vec<u8>, Error> {
        super::request_validation::guard_canonical(physical)?;
        if alternative.family != spec.family || alternative.variant >= spec.variants {
            return Err(Error::AlternativeMismatch);
        }
        let near = match width {
            Width::Near => true,
            Width::Short => false,
            Width::Variant => alternative.variant == 0,
        };
        if near {
            let d = i32::try_from(disp).map_err(|_| Error::BranchDisplacementOutsideI32)?;
            let mut b = vec![0x0f, 0x80 | spec.cc];
            b.extend(d.to_le_bytes());
            Ok(b)
        } else {
            let d = i8::try_from(disp).map_err(|_| Error::BranchDisplacementOutsideI8)?;
            Ok(vec![0x70 | spec.cc, d as u8])
        }
    }

    fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        spec: BranchSpec,
        width: Width,
        disp: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let bytes = request(physical, alternative, spec, width, disp)?;
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes,
            footprint: footprint(spec.kind, alternative, &[]),
        })
    }

    fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        spec: BranchSpec,
        width: Width,
        disp: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let expected = request(physical, alternative, spec, width, disp)?;
        let prefix_len = expected.len() - if expected.len() == 6 { 4 } else { 1 };
        if bytes.len() != expected.len() || bytes[..prefix_len] != expected[..prefix_len] {
            return Err(Error::MalformedEncoding);
        }
        if bytes != expected {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: footprint(spec.kind, alternative, &[]),
        })
    }

    pub fn encode_x86_64_selected_nonzero_branch_form(
        p: &ValidatedPhysicalRegisterModel,
        a: MachineAlternativeKey,
        disp: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        encode(p, a, NONZERO, Width::Near, disp)
    }

    pub fn validate_x86_64_selected_nonzero_branch_form(
        p: &ValidatedPhysicalRegisterModel,
        a: MachineAlternativeKey,
        disp: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        validate(p, a, NONZERO, Width::Near, disp, bytes)
    }

    pub fn encode_x86_64_selected_short_nonzero_branch_form(
        p: &ValidatedPhysicalRegisterModel,
        a: MachineAlternativeKey,
        disp: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        encode(p, a, NONZERO, Width::Short, disp)
    }

    pub fn validate_x86_64_selected_short_nonzero_branch_form(
        p: &ValidatedPhysicalRegisterModel,
        a: MachineAlternativeKey,
        disp: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        validate(p, a, NONZERO, Width::Short, disp, bytes)
    }

    pub fn encode_x86_64_selected_u64_less_than_branch_form(
        p: &ValidatedPhysicalRegisterModel,
        a: MachineAlternativeKey,
        disp: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        encode(p, a, U64_LT, Width::Variant, disp)
    }

    pub fn validate_x86_64_selected_u64_less_than_branch_form(
        p: &ValidatedPhysicalRegisterModel,
        a: MachineAlternativeKey,
        disp: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        validate(p, a, U64_LT, Width::Variant, disp, bytes)
    }

    pub fn encode_x86_64_selected_i64_less_than_branch_form(
        p: &ValidatedPhysicalRegisterModel,
        a: MachineAlternativeKey,
        disp: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        encode(p, a, I64_LT, Width::Variant, disp)
    }

    pub fn validate_x86_64_selected_i64_less_than_branch_form(
        p: &ValidatedPhysicalRegisterModel,
        a: MachineAlternativeKey,
        disp: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        validate(p, a, I64_LT, Width::Variant, disp, bytes)
    }
}

mod jump {
    use super::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
        MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
        MachineEncodedTrapBehavior, ValidatedPhysicalRegisterModel,
        ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError as Error,
        X86_64SelectedFormFootprint,
    };

    fn jump_footprint(physical: &ValidatedPhysicalRegisterModel) -> X86_64SelectedFormFootprint {
        let pc = physical
            .model()
            .view_named("rip")
            .expect("canonical RIP")
            .units
            .clone();
        X86_64SelectedFormFootprint {
            register_reads: vec![],
            register_writes: vec![],
            writes_rflags: false,
            encoded: MachineEncodedEffects {
                external_operand_reads: vec![],
                external_operand_writes: vec![],
                implicit_unit_uses: pc.clone(),
                implicit_unit_defs: pc,
                implicit_unit_clobbers: vec![],
                memory: MachineEncodedMemoryEffect::NoneV1,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
            },
        }
    }

    pub fn encode_x86_64_selected_jump_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        displacement: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        super::request_validation::guard_canonical(physical)?;
        if alternative
            != (MachineAlternativeKey {
                family: MachineAlternativeFamily::Jump,
                variant: 0,
            })
        {
            return Err(Error::AlternativeMismatch);
        }
        let d = i32::try_from(displacement).map_err(|_| Error::EncodedFormMismatch)?;
        let mut bytes = vec![0xe9];
        bytes.extend(d.to_le_bytes());
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes,
            footprint: jump_footprint(physical),
        })
    }

    pub fn validate_x86_64_selected_jump_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        displacement: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let expected = encode_x86_64_selected_jump_form(physical, alternative, displacement)?;
        if bytes != expected.bytes() {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(expected)
    }
}

pub(crate) mod hosted_exit_process {
    use super::request_validation::resolve_registers;
    use super::{
        MachineAlternativeFamily, MachineAlternativeKey, RegisterViewId, SelectedInstructionKind,
        ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
        X86_64SelectedFormEncodingError as Error, X86_64SelectedFormFootprint,
    };
    use target::NativeTarget;
    pub use target_x86_64::inspection::decode_x86_64_selected_hosted_exit_process_i32;

    fn request(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<(u8, u32), Error> {
        let syscall =
            crate::hosted_exit_syscall_number(target).ok_or(Error::EncodedFormMismatch)?;
        if physical.identity() != crate::canonical_x86_64_physical_register_model_identity()
            || kind != SelectedInstructionKind::HostedExitProcessI32
            || alternative
                != (MachineAlternativeKey {
                    family: MachineAlternativeFamily::HostedExitProcessI32,
                    variant: 0,
                })
        {
            return Err(Error::EncodedFormMismatch);
        }
        let regs = resolve_registers(physical, operands)?;
        let &[reg] = regs.as_slice() else {
            return Err(Error::EncodedFormMismatch);
        };
        Ok((reg, syscall))
    }

    fn finish(
        target: NativeTarget,
        operands: &[RegisterViewId],
        bytes: Vec<u8>,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let encoded = target_x86_64::form_effects::hosted_exit_process::effects(target)
            .map_err(|_| Error::EncodedFormMismatch)?;
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes,
            footprint: X86_64SelectedFormFootprint {
                register_reads: operands.to_vec(),
                register_writes: Vec::new(),
                writes_rflags: true,
                encoded,
            },
        })
    }

    pub fn encode_x86_64_selected_hosted_exit_process_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let (reg, syscall) = request(target, physical, kind, alternative, operands)?;
        let mut bytes = vec![
            0x40 | ((reg >> 3) << 2),
            0x89,
            0xc7 | ((reg & 7) << 3),
            0xb8,
        ];
        bytes.extend(syscall.to_le_bytes());
        bytes.extend([0x0f, 0x05, 0x0f, 0x0b]);
        finish(target, operands, bytes)
    }

    pub fn validate_x86_64_selected_hosted_exit_process_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let (reg, _) = request(target, physical, kind, alternative, operands)?;
        if target_x86_64::inspection::decode_hosted_exit_process_register(target, bytes)
            != Some(reg)
        {
            return Err(Error::EncodedFormMismatch);
        }
        finish(target, operands, bytes.to_vec())
    }
}

pub(crate) mod hosted_read_byte {
    use super::{
        MachineAlternativeFamily, MachineAlternativeKey, RegisterViewId, SelectedInstructionKind,
        ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
        X86_64SelectedFormEncodingError as Error, X86_64SelectedFormFootprint,
    };
    use selected_instructions::LocalStorageSlotId;
    use target::NativeTarget;
    pub use target_x86_64::inspection::decode_x86_64_selected_hosted_read_byte;

    fn request(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<u32, Error> {
        if !crate::hosted_syscall_target(target)
            || physical.identity() != crate::canonical_x86_64_physical_register_model_identity()
            || !matches!(
                kind,
                SelectedInstructionKind::HostedReadByte {
                    slot: LocalStorageSlotId::Structural { .. }
                }
            )
            || alternative
                != (MachineAlternativeKey {
                    family: MachineAlternativeFamily::HostedReadByte,
                    variant: 0,
                })
            || !operands.is_empty()
            || displacement < 0
            || displacement % 4 != 0
            || displacement > 2_147_483_640
        {
            return Err(Error::EncodedFormMismatch);
        }
        u32::try_from(displacement).map_err(|_| Error::EncodedFormMismatch)
    }

    fn finish(bytes: Vec<u8>) -> ValidatedX86_64SelectedFormEncoding {
        ValidatedX86_64SelectedFormEncoding {
            bytes,
            footprint: X86_64SelectedFormFootprint {
                register_reads: Vec::new(),
                register_writes: Vec::new(),
                writes_rflags: true,
                encoded: target_x86_64::form_effects::hosted_read_byte::effects(),
            },
        }
    }

    pub fn encode_x86_64_selected_hosted_read_byte_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let disp = request(target, physical, kind, alternative, operands, displacement)?;
        let bytes = if target == NativeTarget::macos_x64() {
            crate::encode_darwin_read_byte_to_stack(disp, disp + 4)
        } else {
            crate::encode_linux_read_byte_to_stack(disp, disp + 4)
        }
        .map_err(|_| Error::EncodedFormMismatch)?;
        Ok(finish(bytes))
    }

    pub fn validate_x86_64_selected_hosted_read_byte_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let disp = request(target, physical, kind, alternative, operands, displacement)?;
        if decode_x86_64_selected_hosted_read_byte(target, bytes) != Some(disp) {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(finish(bytes.to_vec()))
    }
}

pub(crate) mod hosted_write_byte {
    use super::request_validation::resolve_registers;
    use super::{
        MachineAlternativeFamily, MachineAlternativeKey, RegisterViewId, SelectedInstructionKind,
        ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
        X86_64SelectedFormEncodingError as Error, X86_64SelectedFormFootprint,
    };
    use selected_instructions::LocalStorageSlotId;
    use target::NativeTarget;
    pub use target_x86_64::inspection::decode_x86_64_selected_hosted_write_byte_i32;

    fn request(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<(u8, i32), Error> {
        if !crate::hosted_syscall_target(target)
            || physical.identity() != crate::canonical_x86_64_physical_register_model_identity()
            || !matches!(
                kind,
                SelectedInstructionKind::HostedWriteByteI32 {
                    slot: LocalStorageSlotId::Boundary { .. }
                }
            )
            || alternative
                != (MachineAlternativeKey {
                    family: MachineAlternativeFamily::HostedWriteByteI32,
                    variant: 0,
                })
            || !(0..=i64::from(i32::MAX)).contains(&displacement)
        {
            return Err(Error::EncodedFormMismatch);
        }
        let regs = resolve_registers(physical, operands)?;
        let &[reg] = regs.as_slice() else {
            return Err(Error::EncodedFormMismatch);
        };
        let disp = i32::try_from(displacement).map_err(|_| Error::EncodedFormMismatch)?;
        Ok((reg, disp))
    }

    fn finish(operands: &[RegisterViewId], bytes: Vec<u8>) -> ValidatedX86_64SelectedFormEncoding {
        ValidatedX86_64SelectedFormEncoding {
            bytes,
            footprint: X86_64SelectedFormFootprint {
                register_reads: operands.to_vec(),
                register_writes: Vec::new(),
                writes_rflags: true,
                encoded: target_x86_64::form_effects::hosted_write_byte::effects(),
            },
        }
    }

    pub fn encode_x86_64_selected_hosted_write_byte_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let (reg, disp) = request(target, physical, kind, alternative, operands, displacement)?;
        let kernel =
            crate::hosted_kernel::hosted_kernel(target).ok_or(Error::EncodedFormMismatch)?;
        let mut bytes = vec![
            0x40 | ((reg >> 3) << 2),
            0x88,
            0x84 | ((reg & 7) << 3),
            0x24,
        ];
        bytes.extend(disp.to_le_bytes());
        bytes.extend([0xbf, 1, 0, 0, 0, 0x48, 0x8d, 0xb4, 0x24]);
        bytes.extend(disp.to_le_bytes());
        bytes.extend([0xba, 1, 0, 0, 0, 0xb8]);
        bytes.extend(kernel.write.to_le_bytes());
        bytes.extend([0x0f, 0x05]);
        if kernel.carry_reports_failure {
            bytes.extend([0x72, 0x05]);
        }
        bytes.extend([0x48, 0x85, 0xc0, 0x7f, 0x02, 0x0f, 0x0b]);
        Ok(finish(operands, bytes))
    }

    pub fn validate_x86_64_selected_hosted_write_byte_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let (reg, disp) = request(target, physical, kind, alternative, operands, displacement)?;
        if target_x86_64::inspection::decode_hosted_write_byte_register(target, bytes)
            != Some((reg, disp as u32))
        {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(finish(operands, bytes.to_vec()))
    }
}

pub(crate) mod installed_interrupt {
    use super::{
        MachineAlternativeKey, RegisterViewId, SelectedInstructionKind as Kind,
        ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
        X86_64SelectedFormEncodingError as Error, X86_64SelectedFormFootprint,
    };
    pub(crate) use target_x86_64::form_effects::installed_interrupt::{Form, effects};

    impl From<crate::X86InstalledTableEncodingError> for Error {
        fn from(e: crate::X86InstalledTableEncodingError) -> Self {
            Self::InstalledTable(Box::new(e))
        }
    }

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<Form, Error> {
        let form = Form::from_kind(kind).ok_or(Error::AlternativeMismatch)?;
        super::request_validation::validate_request(physical, kind, alternative, operands)?;
        let regs = super::request_validation::resolve_scalar_registers(physical, kind, operands)?;
        if regs != form.registers() {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(form)
    }

    pub(crate) fn footprint(
        form: Form,
        operands: &[RegisterViewId],
    ) -> X86_64SelectedFormFootprint {
        X86_64SelectedFormFootprint {
            register_reads: operands[..form.input_count()].to_vec(),
            register_writes: if form == Form::CpuCheck {
                vec![operands[3]]
            } else {
                Vec::new()
            },
            writes_rflags: form != Form::TablePublication,
            encoded: effects(form),
        }
    }

    pub(crate) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let form = request(physical, kind, alternative, operands)?;
        let bytes = match kind {
            Kind::InstalledInterruptCpuCheck { gdt_bytes } => {
                crate::encode_x86_64_installed_cpu_state_check(
                    crate::X86InstalledCpuStateRegisters::FIXED,
                    crate::X86InstalledCpuStateGeometry { gdt_bytes },
                )?
            }
            Kind::InstalledInterruptTaskCommit { gdt_bytes } => {
                crate::encode_x86_64_installed_task_commit(
                    crate::X86InstalledTaskCommitRegisters::FIXED,
                    crate::X86InstalledCpuStateGeometry { gdt_bytes },
                )?
            }
            Kind::InstalledInterruptTablePublication { table_bytes } => {
                crate::encode_x86_64_installed_table_publication(
                    crate::X86InstalledTablePublicationRegisters::FIXED,
                    table_bytes,
                )?
            }
            _ => return Err(Error::AlternativeMismatch),
        };
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes,
            footprint: footprint(form, operands),
        })
    }

    pub(crate) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let expected = encode(physical, kind, alternative, operands)?;
        if bytes != expected.bytes() {
            return Err(Error::InstalledTable(Box::new(
                crate::X86InstalledTableEncodingError::ReplayMismatch,
            )));
        }
        Ok(expected)
    }
}

pub(crate) mod system_call {
    use super::{
        MachineAlternativeKey, SelectedInstructionKind, ValidatedPhysicalRegisterModel,
        ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError as Error,
        X86_64SelectedFormFootprint,
    };
    use register_model::{RegisterOperandAccess, RegisterViewId, views_with_access};
    use target::NativeTarget;

    pub fn encode_x86_64_selected_system_call_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        operand_accesses: &[RegisterOperandAccess],
        number: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let mut bytes = Vec::with_capacity(12);
        bytes.extend([0x48, 0xb8]);
        bytes.extend(number.to_le_bytes());
        bytes.extend([0x0f, 0x05]);
        validate_x86_64_selected_system_call_form(
            target,
            physical,
            kind,
            alternative,
            operand_views,
            operand_accesses,
            number,
            &bytes,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_x86_64_selected_system_call_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        operand_accesses: &[RegisterOperandAccess],
        number: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let encoded = target_x86_64::inspection::selected_system_call_effects(
            target,
            physical,
            kind,
            alternative,
            operand_views,
            operand_accesses,
            number,
            bytes,
        )
        .ok_or(Error::EncodedFormMismatch)?;
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: X86_64SelectedFormFootprint {
                register_reads: views_with_access(
                    operand_views,
                    operand_accesses,
                    RegisterOperandAccess::Use,
                ),
                register_writes: views_with_access(
                    operand_views,
                    operand_accesses,
                    RegisterOperandAccess::Def,
                ),
                writes_rflags: true,
                encoded,
            },
        })
    }
}

pub(crate) mod table_call {
    use super::{
        MachineAlternativeKey, ValidatedX86_64SelectedFormEncoding,
        X86_64SelectedFormEncodingError as Error, X86_64SelectedFormFootprint,
    };
    use register_model::{RegisterOperandAccess, RegisterViewId, views_with_access};
    use selected_instructions::SelectedNormalizedForeignCall;

    pub fn encode_x86_64_selected_table_call_form(
        record: &SelectedNormalizedForeignCall,
        alternative: MachineAlternativeKey,
        views: &[RegisterViewId],
        accesses: &[RegisterOperandAccess],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let (reg, offset) = target_x86_64::form_effects::table_call::table_address(record)
            .ok_or(Error::EncodedFormMismatch)?;
        let mut bytes = vec![0x4c | (reg >> 3), 0x8b, 0x98 | (reg & 7)];
        bytes.extend(offset.to_le_bytes());
        bytes.extend([0x41, 0xff, 0xd3]);
        validate_x86_64_selected_table_call_form(record, alternative, views, accesses, &bytes)
    }

    pub fn validate_x86_64_selected_table_call_form(
        record: &SelectedNormalizedForeignCall,
        alternative: MachineAlternativeKey,
        views: &[RegisterViewId],
        accesses: &[RegisterOperandAccess],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let encoded = target_x86_64::inspection::selected_table_call_effects(
            record,
            alternative,
            views,
            accesses,
            bytes,
        )
        .ok_or(Error::EncodedFormMismatch)?;
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: X86_64SelectedFormFootprint {
                register_reads: views_with_access(views, accesses, RegisterOperandAccess::Use),
                register_writes: views_with_access(views, accesses, RegisterOperandAccess::Def),
                writes_rflags: true,
                encoded,
            },
        })
    }
}

pub(crate) mod materialization {
    pub(crate) mod mov_r32_imm32 {
        use register_model::RegisterViewId;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum X86_64MovR32Imm32I64MaterializationError {
            NonCanonicalPhysicalModel,
            UnknownOrNonGpr64View(RegisterViewId),
            IntegerOutsideZeroExtendedU32Bits,
            MalformedEncoding,
            EncodedFormMismatch,
        }

        wire::invalid_error_display!(
            X86_64MovR32Imm32I64MaterializationError,
            "x86-64 MOV-r32-imm32 i64 materialization"
        );
    }
    pub(crate) mod mov_r64_imm32_sign_extended {
        use register_model::RegisterViewId;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum X86_64MovR64Imm32SignExtendedI64MaterializationError {
            NonCanonicalPhysicalModel,
            UnknownOrNonGpr64View(RegisterViewId),
            IntegerOutsideSignExtendedI32Bits,
            MalformedEncoding,
            EncodedFormMismatch,
        }

        wire::invalid_error_display!(
            X86_64MovR64Imm32SignExtendedI64MaterializationError,
            "x86-64 MOV-r64-imm32 sign-extended i64 materialization"
        );
    }
}

const fn is_packed_simd(kind: SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::PackedAddI16x8
            | SelectedInstructionKind::PackedSubI16x8
            | SelectedInstructionKind::PackedShuffleI16x8 { .. }
            | SelectedInstructionKind::PackedShuffleGeneralI16x8 { .. }
            | SelectedInstructionKind::PackedShiftRightArithmeticI16x8 { .. }
            | SelectedInstructionKind::PackedWidenU8x8ToI16x8
            | SelectedInstructionKind::PackedNarrowI16x8ToU8x8Saturating
            | SelectedInstructionKind::PackedLoadI16x4ZeroExtend
            | SelectedInstructionKind::PackedWidenU8x4ToI16x8
            | SelectedInstructionKind::PackedNarrowI16x8ToU8x4Saturating
            | SelectedInstructionKind::PackedLoad128 { .. }
            | SelectedInstructionKind::PackedStore128 { .. }
            | SelectedInstructionKind::PackedLoad64ZeroExtend { .. }
            | SelectedInstructionKind::PackedStore64 { .. }
            | SelectedInstructionKind::PackedLoad32ZeroExtend { .. }
            | SelectedInstructionKind::PackedStore32 { .. }
            | SelectedInstructionKind::PackedMaterializeZero
            | SelectedInstructionKind::PackedFromGpr64ZeroExtend
            | SelectedInstructionKind::PackedFromGpr64Pair
            | SelectedInstructionKind::PackedAddI16x8Reg
            | SelectedInstructionKind::PackedSubI16x8Reg
            | SelectedInstructionKind::PackedShiftRightArithmeticI16x8Reg { .. }
            | SelectedInstructionKind::PackedShuffleI16x8Reg { .. }
            | SelectedInstructionKind::PackedWidenU8x8ToI16x8Reg
            | SelectedInstructionKind::PackedNarrowI16x8ToU8x8SaturatingReg
            | SelectedInstructionKind::PackedAddU8x16
            | SelectedInstructionKind::PackedMinI16x8
            | SelectedInstructionKind::PackedCmpGtI16x8
            | SelectedInstructionKind::PackedBitAndI16x8
            | SelectedInstructionKind::PackedBitAndNotI16x8
            | SelectedInstructionKind::PackedAbsI16x8
            | SelectedInstructionKind::PackedAddU8x16Reg
            | SelectedInstructionKind::PackedMinI16x8Reg
            | SelectedInstructionKind::PackedCmpGtI16x8Reg
            | SelectedInstructionKind::PackedBitAndI16x8Reg
            | SelectedInstructionKind::PackedBitAndNotI16x8Reg
            | SelectedInstructionKind::PackedAbsI16x8Reg
    )
}

pub fn encode_x86_64_selected_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    if installed_interrupt::Form::from_kind(kind).is_some() {
        return installed_interrupt::encode(physical, kind, alternative, operands);
    }
    if kind == SelectedInstructionKind::CopyBytes {
        return copy_bytes::encode(physical, alternative, operands, 0);
    }
    if is_packed_simd(kind) {
        return packed_integer::encode(physical, kind, alternative, operands);
    }
    if kind == SelectedInstructionKind::ZeroBytes {
        return zero_bytes::encode(physical, alternative, operands);
    }
    if matches!(
        kind,
        SelectedInstructionKind::LoadEncodedScalar | SelectedInstructionKind::StoreEncodedScalar
    ) {
        return encoded_scalar::encode(physical, kind, alternative, operands);
    }
    if let SelectedInstructionKind::ScanByteSequence { predicate } = kind {
        return scan_byte_sequence::encode(physical, predicate, alternative, operands, 0);
    }
    if kind == SelectedInstructionKind::CompareByteSequences {
        return compare_byte_sequences::encode(physical, alternative, operands, 0);
    }
    if is_float_bit_transfer(kind) {
        return float_bits::encode(physical, kind, alternative, operands);
    }
    if let SelectedInstructionKind::NearestIeeeFloatArithmetic { form } = kind {
        return ieee_float_arithmetic::encode(physical, form, alternative, operands);
    }
    if let SelectedInstructionKind::FloatResultPolicy { form } = kind {
        return float_result_policy::encode(physical, form, alternative, operands);
    }
    request_validation::validate_request(physical, kind, alternative, operands)?;
    let regs = request_validation::resolve_scalar_registers(physical, kind, operands)?;
    request_validation::validate_return_home(kind, &regs)?;
    request_validation::validate_alias_partition(kind, alternative, &regs)?;
    let bytes = scalar_forms::encode_unchecked(kind, alternative, &regs)?;
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes,
        footprint: scalar_forms::footprint(kind, alternative, operands),
    })
}

pub fn validate_x86_64_selected_form_encoding(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    if installed_interrupt::Form::from_kind(kind).is_some() {
        return installed_interrupt::validate(physical, kind, alternative, operands, bytes);
    }
    if let SelectedInstructionKind::RegisterAccess { operation } = kind {
        return register_access::validate(physical, operation, alternative, operands, bytes);
    }
    if let SelectedInstructionKind::PortWrite { .. } = kind {
        return port_write::validate(physical, kind, alternative, operands, bytes);
    }
    if kind == SelectedInstructionKind::CopyBytes {
        return copy_bytes::validate(physical, alternative, operands, 0, bytes);
    }
    if is_packed_simd(kind) {
        return packed_integer::validate(physical, kind, alternative, operands, bytes);
    }
    if kind == SelectedInstructionKind::ZeroBytes {
        return zero_bytes::validate(physical, alternative, operands, bytes);
    }
    if matches!(
        kind,
        SelectedInstructionKind::LoadEncodedScalar | SelectedInstructionKind::StoreEncodedScalar
    ) {
        return encoded_scalar::validate(physical, kind, alternative, operands, bytes);
    }
    if let SelectedInstructionKind::ScanByteSequence { predicate } = kind {
        return scan_byte_sequence::validate(physical, predicate, alternative, operands, 0, bytes);
    }
    if kind == SelectedInstructionKind::CompareByteSequences {
        return compare_byte_sequences::validate(physical, alternative, operands, 0, bytes);
    }
    if is_float_bit_transfer(kind) {
        return float_bits::validate(physical, kind, alternative, operands, bytes);
    }
    if let SelectedInstructionKind::NearestIeeeFloatArithmetic { form } = kind {
        return ieee_float_arithmetic::validate(physical, form, alternative, operands, bytes);
    }
    if let SelectedInstructionKind::FloatResultPolicy { form } = kind {
        return float_result_policy::validate(physical, form, alternative, operands, bytes);
    }
    request_validation::validate_request(physical, kind, alternative, operands)?;
    let regs = request_validation::resolve_scalar_registers(physical, kind, operands)?;
    request_validation::validate_return_home(kind, &regs)?;
    request_validation::validate_alias_partition(kind, alternative, &regs)?;
    let canonical = scalar_forms::encode_unchecked(kind, alternative, &regs)?;
    // Only a same-register copy encodes to no bytes.
    if bytes.is_empty() && !canonical.is_empty() {
        return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
    }
    if bytes != canonical {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: scalar_forms::footprint(kind, alternative, operands),
    })
}

#[cfg(test)]
pub(super) fn assert_byte_fragility(encoded: &[u8], rejects: impl Fn(&[u8]) -> bool) {
    for length in 0..encoded.len() {
        assert!(rejects(&encoded[..length]));
    }
    let mut trailing = encoded.to_vec();
    trailing.push(0);
    assert!(rejects(&trailing));
}

#[cfg(test)]
pub(crate) fn assert_every_bit_flip_rejected(encoded: &[u8], rejects: impl Fn(&[u8]) -> bool) {
    for bit in 0..encoded.len() * 8 {
        let mut changed = encoded.to_vec();
        changed[bit / 8] ^= 1 << (bit % 8);
        assert!(rejects(&changed), "changed encoding bit {bit}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use optimization_core::AcceptedObligationFactIdentity;
    use register_model::validate_physical_register_model;
    use selected_instructions::{
        AccessorScalarForm, AtomicForm, AtomicOperation, ByteSequencePredicate, ControlRegister,
        FloatResultPolicyForm, FrameStorageSlotId, GlobalDescriptorTableOperation,
        IeeeFloatArithmeticForm, InterruptControlKind, LocalStorageSlotId,
        MachineAlternativeFamily as Family, MachineAlternativeKey as Key, MemoryFenceKind,
        PackedByteWidth, RegisterOperation, SaturatingCarrier, SaturatingOperation,
        SegmentRegister, SegmentReload, SelectedImageAddressReference,
        SelectedInstructionKind as Kind, SystemRegister, TrappingForm,
    };
    use semantic_vocabulary::{IntegerValue, ObligationId, OperationId, PlaceId, ServiceId};
    use target::NativeTarget;

    fn phys() -> ValidatedPhysicalRegisterModel {
        validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap()
    }

    fn views(p: &ValidatedPhysicalRegisterModel, names: &[&str]) -> Vec<RegisterViewId> {
        names
            .iter()
            .map(|n| p.model().view_named(n).unwrap().id)
            .collect()
    }

    #[test]
    fn scalar_and_control_golden_vectors_round_trip_and_reject_mutations() {
        let p = phys();
        let op = OperationId::new(1).unwrap();
        let place = PlaceId::new(2).unwrap();
        let ob = ObligationId::new(1).unwrap();
        let fact = AcceptedObligationFactIdentity::from_bytes([1; 32]);
        let svc = ServiceId::new(1).unwrap();
        let cases: &[(Kind, Family, u32, &[&str], &[u8])] = &[
            (
                Kind::AtomicFence,
                Family::AtomicFence,
                0,
                &[],
                &[0x0f, 0xae, 0xf0],
            ),
            (Kind::Crash, Family::Crash, 0, &[], &[0x0f, 0x0b]),
            (
                Kind::PrivilegedCrash,
                Family::PrivilegedCrash,
                0,
                &[],
                &[0xfa, 0xf4, 0xeb, 0xfd],
            ),
            (Kind::ReturnUnit, Family::ReturnUnit, 0, &[], &[0xc3]),
            (
                Kind::ReturnScalar,
                Family::ReturnScalar,
                0,
                &["rax"],
                &[0xc3],
            ),
            (
                Kind::ReturnScalar,
                Family::ReturnScalar,
                0,
                &["xmm0"],
                &[0xc3],
            ),
            (
                Kind::ReturnAggregate { fragment_count: 2 },
                Family::ReturnAggregate,
                0,
                &["rax", "rdx"],
                &[0xc3],
            ),
            (
                Kind::PortWrite {
                    service: svc,
                    port: 0x20,
                    value: 0x20,
                },
                Family::PortWrite,
                0,
                &[],
                &[
                    0x49, 0xba, 0x20, 0, 0, 0, 0, 0, 0, 0, 0x44, 0x89, 0xd2, 0x49, 0xbb, 0x20, 0,
                    0, 0, 0, 0, 0, 0, 0x44, 0x89, 0xd8, 0xee,
                ],
            ),
            (
                Kind::MaterializeI64 {
                    value: IntegerValue::Signed(0),
                },
                Family::MaterializeI64,
                1,
                &["rax"],
                &[0x48, 0x31, 0xc0],
            ),
            (
                Kind::MaterializeI64 {
                    value: IntegerValue::Signed(-1),
                },
                Family::MaterializeI64,
                0,
                &["r12"],
                &[0x49, 0xbc, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
            ),
            (
                Kind::ImageAddress {
                    reference: SelectedImageAddressReference::ByteLiteral {
                        operation: op,
                        place,
                    },
                },
                Family::ImageAddress,
                0,
                &["r9"],
                &[0x4c, 0x8d, 0x0d, 0, 0, 0, 0],
            ),
            (
                Kind::MaterializeBooleanEqual,
                Family::MaterializeBooleanEqual,
                0,
                &["rax"],
                &[0x40, 0x0f, 0x94, 0xc0, 0x40, 0x0f, 0xb6, 0xc0],
            ),
            (
                Kind::ZeroExtendU8,
                Family::ZeroExtendU8,
                0,
                &["rax", "rcx"],
                &[0x40, 0x0f, 0xb6, 0xc8],
            ),
            (
                Kind::ZeroExtendU16,
                Family::ZeroExtendU16,
                0,
                &["r8", "r15"],
                &[0x45, 0x0f, 0xb7, 0xf8],
            ),
            (
                Kind::ZeroExtendU32,
                Family::ZeroExtendU32,
                0,
                &["rax", "rcx"],
                &[0x40, 0x89, 0xc1],
            ),
            (
                Kind::SignExtendI8,
                Family::SignExtendI8,
                0,
                &["rax", "rcx"],
                &[0x48, 0x0f, 0xbe, 0xc8],
            ),
            (
                Kind::SignExtendI16,
                Family::SignExtendI16,
                0,
                &["rax", "rcx"],
                &[0x48, 0x0f, 0xbf, 0xc8],
            ),
            (
                Kind::SignExtendI32,
                Family::SignExtendI32,
                0,
                &["rax", "rcx"],
                &[0x48, 0x63, 0xc8],
            ),
            (
                Kind::CopyI64,
                Family::CopyI64,
                0,
                &["r8", "r15"],
                &[0x4d, 0x89, 0xc7],
            ),
            (
                Kind::CompareI64Zero,
                Family::CompareI64Zero,
                0,
                &["r12"],
                &[0x4d, 0x85, 0xe4],
            ),
            (
                Kind::CompareI64,
                Family::CompareI64,
                0,
                &["rax", "rcx"],
                &[0x48, 0x39, 0xc8],
            ),
            (
                Kind::CompareI64Immediate {
                    immediate: IntegerValue::Unsigned(4095),
                },
                Family::CompareI64Immediate,
                0,
                &["rax"],
                &[0x48, 0x81, 0xf8, 0xff, 0x0f, 0, 0],
            ),
            (
                Kind::WrappingAddI64,
                Family::WrappingAddI64,
                0,
                &["rbp", "r13", "r12"],
                &[0x4e, 0x8d, 0x64, 0x2d, 0],
            ),
            (
                Kind::ExactAddI64Immediate {
                    immediate: IntegerValue::Unsigned(128),
                    obligation: ob,
                    accepted_fact: fact,
                },
                Family::ExactAddI64Immediate,
                0,
                &["r12", "r15"],
                &[0x4d, 0x8d, 0xbc, 0x24, 0x80, 0, 0, 0],
            ),
            (
                Kind::ExactSubtractI64Immediate {
                    immediate: IntegerValue::Unsigned(128),
                    obligation: ob,
                    accepted_fact: fact,
                },
                Family::ExactSubtractI64Immediate,
                0,
                &["rax", "rcx"],
                &[0x48, 0x8d, 0x48, 0x80],
            ),
            (
                Kind::WrappingSubtractI64,
                Family::WrappingSubtractI64,
                0,
                &["rax", "rax", "rax"],
                &[0x48, 0x31, 0xc0],
            ),
            (
                Kind::WrappingSubtractI64,
                Family::WrappingSubtractI64,
                1,
                &["r8", "r9", "r8"],
                &[0x4d, 0x29, 0xc8],
            ),
            (
                Kind::WrappingSubtractI64,
                Family::WrappingSubtractI64,
                2,
                &["r8", "r9", "r9"],
                &[0x49, 0xf7, 0xd9, 0x4d, 0x01, 0xc1],
            ),
            (
                Kind::WrappingSubtractI64,
                Family::WrappingSubtractI64,
                3,
                &["r8", "r9", "r10"],
                &[0x4d, 0x89, 0xc2, 0x4d, 0x29, 0xca],
            ),
            (
                Kind::WrappingMultiplyI64,
                Family::WrappingMultiplyI64,
                3,
                &["r8", "r9", "r10"],
                &[0x4d, 0x89, 0xc2, 0x4d, 0x0f, 0xaf, 0xd1],
            ),
            (
                Kind::WrappingShiftLeftI64,
                Family::WrappingShiftLeftI64,
                0,
                &["rax", "rcx", "r9"],
                &[0x49, 0x89, 0xc1, 0x49, 0xd3, 0xe1],
            ),
            (
                Kind::BitwiseAndI64,
                Family::BitwiseAndI64,
                0,
                &["r12", "r15", "r12"],
                &[0x4d, 0x21, 0xfc],
            ),
            (
                Kind::BitwiseNotI64,
                Family::BitwiseNotI64,
                0,
                &["r12", "r12"],
                &[0x49, 0xf7, 0xd4],
            ),
            (
                Kind::ZeroBytes,
                Family::ZeroBytes,
                0,
                &["rdi", "rcx", "rax"],
                &[
                    0x48, 0x31, 0xc0, 0x48, 0x85, 0xc9, 0x74, 0x11, 0x40, 0xc6, 0x84, 0x07, 0, 0,
                    0, 0, 0, 0x48, 0xff, 0xc0, 0x48, 0x39, 0xc8, 0x72, 0xef,
                ],
            ),
            (
                Kind::Float32ToBits,
                Family::Float32ToBits,
                0,
                &["xmm0", "rax"],
                &[0x66, 0x0f, 0x7e, 0xc0],
            ),
            (
                Kind::BitsToFloat64,
                Family::BitsToFloat64,
                0,
                &["r15", "xmm15"],
                &[0x66, 0x4d, 0x0f, 0x6e, 0xff],
            ),
        ];
        for &(kind, family, variant, names, expected) in cases {
            let ops = views(&p, names);
            let key = Key { family, variant };
            let enc = encode_x86_64_selected_form(&p, kind, key, &ops).unwrap();
            assert_eq!(enc.bytes(), expected, "{kind:?} v{variant}");
            assert_eq!(
                validate_x86_64_selected_form_encoding(&p, kind, key, &ops, expected).unwrap(),
                enc
            );
            assert_every_bit_flip_rejected(expected, |b| {
                validate_x86_64_selected_form_encoding(&p, kind, key, &ops, b).is_err()
            });
            assert_byte_fragility(expected, |b| {
                validate_x86_64_selected_form_encoding(&p, kind, key, &ops, b).is_err()
            });
        }
        assert_eq!(
            encode_x86_64_selected_form(
                &p,
                Kind::ReturnScalar,
                Key {
                    family: Family::ReturnScalar,
                    variant: 0
                },
                &views(&p, &["rcx"])
            ),
            Err(X86_64SelectedFormEncodingError::EncodedFormMismatch)
        );
        assert_eq!(
            encode_x86_64_selected_form(
                &p,
                Kind::CompareI64Immediate {
                    immediate: IntegerValue::Unsigned(4096)
                },
                Key {
                    family: Family::CompareI64Immediate,
                    variant: 0
                },
                &views(&p, &["rax"])
            ),
            Err(X86_64SelectedFormEncodingError::ImmediateOutsideU12)
        );
    }

    #[test]
    fn a_same_register_copy_encodes_to_nothing_and_admits_no_bytes() {
        let p = phys();
        let key = Key {
            family: Family::CopyI64,
            variant: 0,
        };
        let same = views(&p, &["r9", "r9"]);
        let enc = encode_x86_64_selected_form(&p, Kind::CopyI64, key, &same).unwrap();
        assert!(enc.bytes().is_empty());
        assert_eq!(
            validate_x86_64_selected_form_encoding(&p, Kind::CopyI64, key, &same, &[]).unwrap(),
            enc
        );
        // `mov r9, r9` is still the wrong bytes for this row.
        assert!(
            validate_x86_64_selected_form_encoding(
                &p,
                Kind::CopyI64,
                key,
                &same,
                &[0x4d, 0x89, 0xc9]
            )
            .is_err()
        );
        // A copy between different registers may not claim the empty encoding.
        let different = views(&p, &["r8", "r15"]);
        assert_eq!(
            validate_x86_64_selected_form_encoding(&p, Kind::CopyI64, key, &different, &[]),
            Err(X86_64SelectedFormEncodingError::MalformedEncoding)
        );
    }

    #[test]
    fn exhaustive_families_encode_validate_and_reject_corruptions() {
        let p = phys();
        let ob = ObligationId::new(1).unwrap();
        let fact = AcceptedObligationFactIdentity::from_bytes([1; 32]);

        for op in [
            SaturatingOperation::Add,
            SaturatingOperation::Subtract,
            SaturatingOperation::Divide,
            SaturatingOperation::Remainder,
            SaturatingOperation::Multiply,
        ] {
            for carrier in SaturatingCarrier::ALL {
                let (kind, family) = match op {
                    SaturatingOperation::Add => (
                        Kind::SaturatingAdd { carrier },
                        Family::SaturatingAdd(carrier),
                    ),
                    SaturatingOperation::Subtract => (
                        Kind::SaturatingSubtract { carrier },
                        Family::SaturatingSubtract(carrier),
                    ),
                    SaturatingOperation::Divide => (
                        Kind::SaturatingDivide {
                            carrier,
                            obligation: ob,
                            accepted_fact: fact,
                        },
                        Family::SaturatingDivide(carrier),
                    ),
                    SaturatingOperation::Remainder => (
                        Kind::SaturatingRemainder {
                            carrier,
                            obligation: ob,
                            accepted_fact: fact,
                        },
                        Family::SaturatingRemainder(carrier),
                    ),
                    SaturatingOperation::Multiply => (
                        Kind::SaturatingMultiply { carrier },
                        Family::SaturatingMultiply(carrier),
                    ),
                };
                let names: &[&str] = match op {
                    SaturatingOperation::Divide | SaturatingOperation::Remainder => {
                        &["rax", "r11", "rax", "rdx"]
                    }
                    SaturatingOperation::Multiply if carrier == SaturatingCarrier::U64 => {
                        &["rax", "r11", "rax", "rdx"]
                    }
                    SaturatingOperation::Add if carrier == SaturatingCarrier::U64 => {
                        &["r12", "r13", "r15"]
                    }
                    SaturatingOperation::Subtract if !carrier.is_signed() => &["r12", "r13", "r15"],
                    _ => &["r12", "r13", "r14", "r15"],
                };
                let ops = views(&p, names);
                let key = Key { family, variant: 0 };
                let enc = encode_x86_64_selected_form(&p, kind, key, &ops).unwrap();
                assert_eq!(
                    validate_x86_64_selected_form_encoding(&p, kind, key, &ops, enc.bytes())
                        .unwrap(),
                    enc
                );
            }
        }

        for form in TrappingForm::ALL {
            let kind = Kind::TrappingInteger { form };
            let key = Key {
                family: Family::TrappingInteger(form),
                variant: 0,
            };
            let found = [
                &["rdi", "r8", "r9"][..],
                &["rdi", "rsi", "r8"],
                &["rdi", "rsi", "r8", "r9"],
                &["rdi", "rcx", "r8"],
                &["rdi", "rcx", "r8", "r9"],
                &["rax", "r11", "rax", "rdx"],
            ]
            .into_iter()
            .any(|names| {
                let ops = views(&p, names);
                encode_x86_64_selected_form(&p, kind, key, &ops).is_ok_and(|enc| {
                    validate_x86_64_selected_form_encoding(&p, kind, key, &ops, enc.bytes())
                        .unwrap()
                        == enc
                })
            });
            assert!(found, "missing trapping form {form:?}");
        }

        for form in AtomicForm::ALL {
            let kind = Kind::AtomicAccess { form };
            let key = Key {
                family: Family::AtomicAccess(form),
                variant: 0,
            };
            let names: &[&str] = match form.operation {
                AtomicOperation::Load | AtomicOperation::Store => &["r12", "r15"],
                AtomicOperation::CompareExchange => &["r12", "rax", "r13", "r15"],
                AtomicOperation::CompareExchangeOnce => &["r12", "rax", "r13", "r14", "r15"],
                _ => &["r12", "r13", "r15"],
            };
            let ops = views(&p, names);
            let enc = encode_x86_64_selected_form(&p, kind, key, &ops).unwrap();
            assert_eq!(
                validate_x86_64_selected_form_encoding(&p, kind, key, &ops, enc.bytes()).unwrap(),
                enc
            );
        }

        for form in IeeeFloatArithmeticForm::ALL {
            let kind = Kind::NearestIeeeFloatArithmetic { form };
            let key = Key {
                family: Family::NearestIeeeFloatArithmetic(form),
                variant: 0,
            };
            let ops = views(
                &p,
                &["r12", "r13", "r14", "r15"][..form.operand_count() + 1],
            );
            let enc = encode_x86_64_selected_form(&p, kind, key, &ops).unwrap();
            assert_eq!(
                validate_x86_64_selected_form_encoding(&p, kind, key, &ops, enc.bytes()).unwrap(),
                enc
            );
        }

        for form in FloatResultPolicyForm::ALL {
            let kind = Kind::FloatResultPolicy { form };
            let key = Key {
                family: Family::FloatResultPolicy(form),
                variant: 0,
            };
            let ops = views(&p, &["r12", "r13", "r15"][..form.operand_count() + 1]);
            let enc = encode_x86_64_selected_form(&p, kind, key, &ops).unwrap();
            assert_eq!(
                validate_x86_64_selected_form_encoding(&p, kind, key, &ops, enc.bytes()).unwrap(),
                enc
            );
        }

        for (op, names) in [
            (
                RegisterOperation::MemoryFence(MemoryFenceKind::Full),
                &[][..],
            ),
            (
                RegisterOperation::GlobalDescriptorTable(GlobalDescriptorTableOperation::Load),
                &["rdi"],
            ),
            (
                RegisterOperation::SystemRead(SystemRegister::Control(ControlRegister::Cr3)),
                &["rax"],
            ),
            (
                RegisterOperation::SystemWrite(SystemRegister::Control(ControlRegister::Cr4)),
                &["r12"],
            ),
            (
                RegisterOperation::InterruptControl(InterruptControlKind::Disable),
                &[],
            ),
            (RegisterOperation::ProcessorHalt, &[]),
            (
                RegisterOperation::SystemRead(SystemRegister::Rflags),
                &["rax"],
            ),
            (
                RegisterOperation::SystemWrite(SystemRegister::Rflags),
                &["r15"],
            ),
            (RegisterOperation::PortRead, &["rdi", "rax"]),
            (RegisterOperation::PortWrite, &["r11", "r10"]),
            (RegisterOperation::ModelSpecificRead, &["rdi", "rax"]),
            (RegisterOperation::ModelSpecificWrite, &["r11", "r10"]),
            (
                RegisterOperation::TaskRegisterActivate,
                &["rdi", "rsi", "rdx", "rcx", "r8", "rax"],
            ),
            (RegisterOperation::TaskRegisterRead, &["r15"]),
            (
                RegisterOperation::SegmentReload(SegmentReload::Code),
                &["rdi", "rdx", "rcx", "r8", "rax"],
            ),
            (
                RegisterOperation::SegmentReload(SegmentReload::Data),
                &["rdi", "rdx", "rcx", "r8", "rax"],
            ),
            (
                RegisterOperation::SegmentRead(SegmentRegister::Gs),
                &["r15"],
            ),
        ] {
            let family = register_access::family(op).unwrap();
            let kind = Kind::RegisterAccess { operation: op };
            let key = Key { family, variant: 0 };
            let ops = views(&p, names);
            let enc = encode_x86_64_selected_form(&p, kind, key, &ops).unwrap();
            assert_eq!(
                validate_x86_64_selected_form_encoding(&p, kind, key, &ops, enc.bytes()).unwrap(),
                enc
            );
        }

        for pred in [
            ByteSequencePredicate::NonEmpty,
            ByteSequencePredicate::NoNul,
            ByteSequencePredicate::AsciiOnly,
            ByteSequencePredicate::ValidUtf8,
        ] {
            let ops = views(&p, &["r13", "r14", "r12", "r15", "r8"]);
            let kind = Kind::ScanByteSequence { predicate: pred };
            let key = Key {
                family: Family::ScanByteSequence,
                variant: 0,
            };
            let enc = encode_x86_64_selected_form(&p, kind, key, &ops).unwrap();
            assert_eq!(
                validate_x86_64_selected_form_encoding(&p, kind, key, &ops, enc.bytes()).unwrap(),
                enc
            );
        }
    }

    #[test]
    fn branches_memory_installed_and_hosted_forms_round_trip() {
        let p = phys();
        let op = OperationId::new(1).unwrap();
        let place = PlaceId::new(2).unwrap();
        let local = FrameStorageSlotId::Local(LocalStorageSlotId::Structural {
            operation: op,
            place,
        });
        let boundary = LocalStorageSlotId::Boundary { operation: op };
        let structural = LocalStorageSlotId::Structural {
            operation: op,
            place,
        };

        let jmp = encode_x86_64_selected_jump_form(
            &p,
            Key {
                family: Family::Jump,
                variant: 0,
            },
            -5,
        )
        .unwrap();
        assert_eq!(jmp.bytes(), &[0xe9, 0xfb, 0xff, 0xff, 0xff]);
        assert_eq!(
            validate_x86_64_selected_jump_form(
                &p,
                Key {
                    family: Family::Jump,
                    variant: 0
                },
                -5,
                jmp.bytes()
            )
            .unwrap(),
            jmp
        );

        let jnz = encode_x86_64_selected_short_nonzero_branch_form(
            &p,
            Key {
                family: Family::ConditionalBranchNonZero,
                variant: 0,
            },
            12,
        )
        .unwrap();
        assert_eq!(jnz.bytes(), &[0x75, 12]);
        assert!(
            validate_x86_64_selected_short_nonzero_branch_form(
                &p,
                Key {
                    family: Family::ConditionalBranchNonZero,
                    variant: 0
                },
                13,
                jnz.bytes()
            )
            .is_err()
        );

        for (kind, family, names, disp) in [
            (
                Kind::Load8 { byte_offset: 12 },
                Family::Load8,
                &["r12", "r15"][..],
                12i64,
            ),
            (
                Kind::Load64 { byte_offset: 16 },
                Family::Load64,
                &["r12", "r15"],
                16,
            ),
            (
                Kind::Store {
                    byte_offset: 6,
                    byte_size: 2,
                },
                Family::Store,
                &["rdi", "rax"],
                6,
            ),
            (
                Kind::Load64Indexed,
                Family::Load64Indexed,
                &["r13", "rcx", "r15"],
                0,
            ),
            (
                Kind::LoadPacked {
                    byte_offset: 8,
                    width: PackedByteWidth::Five,
                },
                Family::LoadPacked5,
                &["r12", "r13", "r15"],
                8,
            ),
            (
                Kind::StorePacked {
                    byte_offset: 8,
                    width: PackedByteWidth::Three,
                },
                Family::StorePacked,
                &["r12", "r13", "r15"],
                8,
            ),
            (
                Kind::SaveFloatingControl { slot: boundary },
                Family::SaveFloatingControl,
                &[],
                16,
            ),
        ] {
            let ops = views(&p, names);
            let key = Key { family, variant: 0 };
            let enc = encode_x86_64_selected_memory_form(&p, kind, key, &ops, disp).unwrap();
            assert_eq!(
                validate_x86_64_selected_memory_form(&p, kind, key, &ops, disp, enc.bytes())
                    .unwrap(),
                enc
            );
        }
        for form in AccessorScalarForm::ALL {
            let kind = Kind::AccessorScalarAccess {
                form,
                byte_offset: 16,
                contract: [7; 32],
            };
            let key = Key {
                family: Family::AccessorScalarAccess(form),
                variant: 0,
            };
            let ops = views(&p, &["r12", "r15"]);
            let enc = encode_x86_64_selected_memory_form(&p, kind, key, &ops, 16).unwrap();
            assert_eq!(
                validate_x86_64_selected_memory_form(&p, kind, key, &ops, 16, enc.bytes()).unwrap(),
                enc
            );
        }

        let aligned = encode_x86_64_selected_frame_form(
            &p,
            Kind::FrameAddress {
                slot: local,
                byte_offset: 24,
            },
            Key {
                family: Family::FrameAddress,
                variant: 0,
            },
            &views(&p, &["r12"]),
            48,
            64,
        )
        .unwrap();
        assert_eq!(
            validate_x86_64_selected_frame_form(
                &p,
                Kind::FrameAddress {
                    slot: local,
                    byte_offset: 24
                },
                Key {
                    family: Family::FrameAddress,
                    variant: 0
                },
                &views(&p, &["r12"]),
                48,
                64,
                aligned.bytes()
            )
            .unwrap(),
            aligned
        );

        for target in [NativeTarget::linux_x64(), NativeTarget::macos_x64()] {
            let exit = hosted_exit_process::encode_x86_64_selected_hosted_exit_process_form(
                target,
                &p,
                Kind::HostedExitProcessI32,
                Key {
                    family: Family::HostedExitProcessI32,
                    variant: 0,
                },
                &views(&p, &["r11"]),
            )
            .unwrap();
            assert_eq!(
                hosted_exit_process::validate_x86_64_selected_hosted_exit_process_form(
                    target,
                    &p,
                    Kind::HostedExitProcessI32,
                    Key {
                        family: Family::HostedExitProcessI32,
                        variant: 0
                    },
                    &views(&p, &["r11"]),
                    exit.bytes()
                )
                .unwrap(),
                exit
            );
            let read = hosted_read_byte::encode_x86_64_selected_hosted_read_byte_form(
                target,
                &p,
                Kind::HostedReadByte { slot: structural },
                Key {
                    family: Family::HostedReadByte,
                    variant: 0,
                },
                &[],
                16,
            )
            .unwrap();
            assert_eq!(
                hosted_read_byte::validate_x86_64_selected_hosted_read_byte_form(
                    target,
                    &p,
                    Kind::HostedReadByte { slot: structural },
                    Key {
                        family: Family::HostedReadByte,
                        variant: 0
                    },
                    &[],
                    16,
                    read.bytes()
                )
                .unwrap(),
                read
            );
            let write = encode_x86_64_selected_hosted_write_byte_form(
                target,
                &p,
                Kind::HostedWriteByteI32 { slot: boundary },
                Key {
                    family: Family::HostedWriteByteI32,
                    variant: 0,
                },
                &views(&p, &["r11"]),
                32,
            )
            .unwrap();
            assert_eq!(
                validate_x86_64_selected_hosted_write_byte_form(
                    target,
                    &p,
                    Kind::HostedWriteByteI32 { slot: boundary },
                    Key {
                        family: Family::HostedWriteByteI32,
                        variant: 0
                    },
                    &views(&p, &["r11"]),
                    32,
                    write.bytes()
                )
                .unwrap(),
                write
            );
        }

        let cpu = encode_x86_64_selected_form(
            &p,
            Kind::InstalledInterruptCpuCheck { gdt_bytes: 64 },
            Key {
                family: Family::InstalledInterruptCpuCheck,
                variant: 0,
            },
            &views(&p, &["rdi", "rsi", "rcx", "rax"]),
        )
        .unwrap();
        assert_eq!(
            validate_x86_64_selected_form_encoding(
                &p,
                Kind::InstalledInterruptCpuCheck { gdt_bytes: 64 },
                Key {
                    family: Family::InstalledInterruptCpuCheck,
                    variant: 0
                },
                &views(&p, &["rdi", "rsi", "rcx", "rax"]),
                cpu.bytes()
            )
            .unwrap(),
            cpu
        );
    }

    #[test]
    fn system_call_conformance_and_callback_field_vectors() {
        let p = crate::validated_x86_64_physical_register_model();
        let target = NativeTarget::linux_x64();
        let catalog = crate::x86_64_register_constraint_catalog_for(p);
        let kind = Kind::NormalizedForeignCall {
            boundary: semantic_vocabulary::BoundaryMachineId::new(3).unwrap(),
            ordinal: 0,
        };
        let key = Key {
            family: Family::NormalizedForeignCall,
            variant: 0,
        };
        fn summarize(
            enc: ValidatedX86_64SelectedFormEncoding,
        ) -> isa_conformance::system_call::Encoding {
            isa_conformance::system_call::Encoding {
                bytes: enc.bytes().to_vec(),
                effects: enc.footprint().encoded.clone(),
            }
        }
        let rows: Vec<_> = crate::x86_64_linux_system_call_keys()
            .into_iter()
            .map(|k| catalog.constraint(k).unwrap())
            .collect();
        isa_conformance::system_call::conformance(
            &rows,
            p.model().view_named("rsp").unwrap().id,
            |v, a, n| {
                system_call::encode_x86_64_selected_system_call_form(target, p, kind, key, v, a, n)
                    .map(summarize)
            },
            |v, a, n, b| {
                system_call::validate_x86_64_selected_system_call_form(
                    target, p, kind, key, v, a, n, b,
                )
                .map(summarize)
            },
            |v, a, n, b| {
                system_call::validate_x86_64_selected_system_call_form(
                    NativeTarget::windows_x64(),
                    p,
                    kind,
                    key,
                    v,
                    a,
                    n,
                    b,
                )
                .is_err()
            },
        );

        let field = X86_64CallbackMaterializationDestination::RecordField {
            root_register: calling_conventions::MachineRegister::X86Rdi,
            placements: calling_conventions::PrivateCallbackPlacements::new(&[
                layout_plans::LayoutPlacementReport::Bits {
                    container: 4,
                    container_width: 32,
                    destination_lsb: 5,
                    source_lsb: 17,
                    width: 10,
                },
            ]),
        };
        assert_eq!(
            normalized_foreign_call::callback_group_bytes(field).unwrap(),
            [
                0x4c, 0x8d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x40, 0x8b, 0x87, 0x04, 0x00, 0x00, 0x00,
                0x48, 0xc1, 0xc8, 0x05, 0x48, 0xc1, 0xe8, 0x0a, 0x48, 0xc1, 0xe0, 0x0a, 0x48, 0xc1,
                0xc0, 0x05, 0x4d, 0x89, 0xda, 0x49, 0xc1, 0xea, 0x11, 0x49, 0xc1, 0xe2, 0x36, 0x49,
                0xc1, 0xea, 0x36, 0x49, 0xc1, 0xe2, 0x05, 0x4c, 0x09, 0xd0, 0x40, 0x89, 0x87, 0x04,
                0x00, 0x00, 0x00
            ]
        );
    }
}
