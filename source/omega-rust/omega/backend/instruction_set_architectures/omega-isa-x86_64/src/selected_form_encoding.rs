use omega_calling_conventions::{CallingPolicy, EntryControl, MachineRegister};
use omega_register_model::{
    RegisterUnitId, RegisterViewId, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog,
};
use omega_selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineCleanupEffect,
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineTrapBehavior,
    SelectedInstructionKind, SelectedStructuralUnitCallInstruction,
    SelectedStructuralUnitIndirectBinding, StructuralUnitCallBarrier, StructuralUnitCallEffect,
    StructuralUnitCallEffectDeclaration, StructuralUnitCallFrameEffect,
    StructuralUnitCallMemoryEffect,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use psi_core::{IntegerValue, MachineId};

use crate::{
    X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR, x86_64_physical_register_model,
    x86_64_register_constraint_catalog,
};

pub const X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT: usize = 89;
pub const X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET: u16 = 80;
pub const X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET: u16 = 81;
pub const X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET: u16 = 85;
pub const X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64StructuralUnitInternalControlFixupKind {
    Relative32FromNextInstructionToInternalMachineV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64StructuralUnitInternalControlFixupState {
    UnresolvedZeroFieldV1,
}

/// One section-layout-dependent internal-control field. This is not an object
/// relocation: the selected callee is an in-roster [`MachineId`], but its
/// section coordinate is deliberately unavailable at selected-form encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64StructuralUnitInternalControlFixup {
    pub kind: X86_64StructuralUnitInternalControlFixupKind,
    pub state: X86_64StructuralUnitInternalControlFixupState,
    pub callee: MachineId,
    pub opcode_byte_offset: u16,
    pub field_byte_offset: u16,
    pub next_instruction_byte_offset: u16,
    pub field_byte_width: u8,
    pub addend: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64StructuralUnitInternalControlResolutionState {
    ResolvedInSectionV1,
}

/// Target-owned evidence that one structural Unit call fixup has been
/// discharged against concrete text-section coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64ResolvedStructuralUnitInternalControlFixup {
    pub source: X86_64StructuralUnitInternalControlFixup,
    pub state: X86_64StructuralUnitInternalControlResolutionState,
    pub caller_section_offset: u64,
    pub callee_section_offset: u64,
    pub next_instruction_section_offset: u64,
    pub displacement: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64StructuralUnitRootRead {
    pub root: MachineRegister,
    pub byte_offset: u32,
    pub byte_count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64StructuralUnitCallerCopyWrite {
    pub stack_byte_offset: u32,
    pub byte_count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64StructuralUnitArgumentPointerWrite {
    pub register: MachineRegister,
    pub stack_byte_offset: u32,
}

/// Independently decoded architectural footprint of the exact bounded call
/// bundle. It remains distinct from ordinary alternative effects because the
/// latter cannot express root-indirect reads, caller-copy writes, or a call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64SelectedStructuralUnitCallFootprint {
    pub implicit_unit_uses: Vec<RegisterUnitId>,
    pub implicit_unit_defs: Vec<RegisterUnitId>,
    pub implicit_unit_clobbers: Vec<RegisterUnitId>,
    pub root_reads: [X86_64StructuralUnitRootRead; 4],
    pub caller_copy_writes: [X86_64StructuralUnitCallerCopyWrite; 4],
    pub scratch_register_writes: [MachineRegister; 1],
    pub argument_pointer_writes: [X86_64StructuralUnitArgumentPointerWrite; 2],
    pub writes_rflags: bool,
    pub frame_byte_count: u32,
    pub shadow_byte_count: u32,
    pub pre_call_stack_alignment: u16,
    pub frame_is_balanced: bool,
    pub trap: MachineTrapBehavior,
    pub barrier: StructuralUnitCallBarrier,
    pub call: StructuralUnitCallEffect,
    pub cleanup: MachineCleanupEffect,
}

/// Canonical bytes plus explicit proof that their rel32 field is unresolved.
/// These bytes must not be treated as executable until section placement has
/// discharged the returned fixup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64SelectedStructuralUnitCallTemplate {
    bytes: Vec<u8>,
    footprint: X86_64SelectedStructuralUnitCallFootprint,
    fixup: X86_64StructuralUnitInternalControlFixup,
}

impl ValidatedX86_64SelectedStructuralUnitCallTemplate {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn footprint(&self) -> &X86_64SelectedStructuralUnitCallFootprint {
        &self.footprint
    }

    pub const fn fixup(&self) -> X86_64StructuralUnitInternalControlFixup {
        self.fixup
    }
}

/// Canonical executable bytes plus independently replayed evidence that their
/// rel32 field targets the selected in-section callee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64ResolvedStructuralUnitCall {
    bytes: Vec<u8>,
    footprint: X86_64SelectedStructuralUnitCallFootprint,
    resolution: X86_64ResolvedStructuralUnitInternalControlFixup,
}

impl ValidatedX86_64ResolvedStructuralUnitCall {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn footprint(&self) -> &X86_64SelectedStructuralUnitCallFootprint {
        &self.footprint
    }

    pub const fn resolution(&self) -> X86_64ResolvedStructuralUnitInternalControlFixup {
        self.resolution
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64StructuralUnitInternalControlResolutionError {
    FixupMismatch,
    SectionCoordinateOverflow,
    RelativeDisplacementOutOfRange,
    MalformedResolvedBytes,
    TargetEquationMismatch,
}

impl std::fmt::Display for X86_64StructuralUnitInternalControlResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid x86-64 structural Unit internal-control resolution: {self:?}"
        )
    }
}

impl std::error::Error for X86_64StructuralUnitInternalControlResolutionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64StructuralUnitCallTemplateError {
    UnsupportedTarget,
    NonCanonicalPhysicalModel,
    NonCanonicalConstraintCatalog,
    ConstraintMismatch,
    CallPlanMismatch,
    LayoutMismatch,
    EffectMismatch,
    MalformedTemplate,
}

impl std::fmt::Display for X86_64StructuralUnitCallTemplateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid x86-64 structural Unit call template: {self:?}"
        )
    }
}

impl std::error::Error for X86_64StructuralUnitCallTemplateError {}

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
    validate_branch_request(physical, alternative)?;
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
    validate_branch_request(physical, alternative)?;
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
    validate_branch_request(physical, alternative)?;
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
    validate_branch_request(physical, alternative)?;
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

fn validate_branch_request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
) -> Result<(), X86_64SelectedFormEncodingError> {
    if physical.model() != &x86_64_physical_register_model() {
        return Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    if alternative
        != (MachineAlternativeKey {
            family: MachineAlternativeFamily::ConditionalBranchNonZero,
            variant: 0,
        })
    {
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
        SelectedInstructionKind::MaterializeI64 { .. } => {
            (MachineAlternativeFamily::MaterializeI64, 1, 0..=0)
        }
        SelectedInstructionKind::CopyI64 => (MachineAlternativeFamily::CopyI64, 2, 0..=0),
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
        SelectedInstructionKind::ReturnUnit => (MachineAlternativeFamily::ReturnUnit, 0, 0..=0),
        SelectedInstructionKind::ConditionalBranchNonZero => {
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
        SelectedInstructionKind::CopyI64 => {
            append_register_binary(&mut bytes, 0x89, registers[0], registers[1]);
        }
        SelectedInstructionKind::CompareI64Zero => {
            append_register_binary(&mut bytes, 0x85, registers[0], registers[0]);
        }
        SelectedInstructionKind::ExactAddI64 { .. } => {
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
        SelectedInstructionKind::ReturnI64 | SelectedInstructionKind::ReturnUnit => {
            bytes.push(0xc3)
        }
        SelectedInstructionKind::ConditionalBranchNonZero => {
            return Err(X86_64SelectedFormEncodingError::LayoutDependentForm);
        }
    }
    Ok(bytes)
}

/// Encode the one bounded Microsoft-x64 structural Unit call as an explicitly
/// unresolved template. The returned zero rel32 field is owned by the typed
/// internal-control fixup and is not an executable displacement.
pub fn encode_x86_64_selected_structural_unit_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    call: &SelectedStructuralUnitCallInstruction,
    declaration: StructuralUnitCallEffectDeclaration,
) -> Result<ValidatedX86_64SelectedStructuralUnitCallTemplate, X86_64StructuralUnitCallTemplateError>
{
    validate_structural_unit_call_request(target, physical, constraints, call, declaration)?;

    let mut bytes = Vec::with_capacity(X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT);
    bytes.extend([0x48, 0x83, 0xec, 0x48]);
    for (source_modrm, source_offset, stack_offset) in [
        (0x81, 0_u32, 32_u32),
        (0x81, 8, 40),
        (0x82, 0, 48),
        (0x82, 8, 56),
    ] {
        bytes.extend([0x48, 0x8b, source_modrm]);
        bytes.extend(source_offset.to_le_bytes());
        bytes.extend([0x48, 0x89, 0x84, 0x24]);
        bytes.extend(stack_offset.to_le_bytes());
    }
    bytes.extend([0x48, 0x8d, 0x8c, 0x24]);
    bytes.extend(32_u32.to_le_bytes());
    bytes.extend([0x48, 0x8d, 0x94, 0x24]);
    bytes.extend(48_u32.to_le_bytes());
    bytes.extend([0xe8, 0, 0, 0, 0]);
    bytes.extend([0x48, 0x83, 0xc4, 0x48]);
    debug_assert_eq!(bytes.len(), X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT);

    validate_x86_64_selected_structural_unit_call_template(
        target,
        physical,
        constraints,
        call,
        declaration,
        &bytes,
    )
}

/// Independently decode and validate an unresolved structural Unit call
/// template. In particular, this routine parses every opcode, operand, and
/// displacement instead of comparing the candidate with encoder output.
pub fn validate_x86_64_selected_structural_unit_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    call: &SelectedStructuralUnitCallInstruction,
    declaration: StructuralUnitCallEffectDeclaration,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedStructuralUnitCallTemplate, X86_64StructuralUnitCallTemplateError>
{
    validate_structural_unit_call_request(target, physical, constraints, call, declaration)?;
    let decoded = decode_structural_unit_call_template(bytes, call, declaration)?;
    let expected = expected_structural_unit_call_footprint(call, declaration);
    if decoded != expected {
        return Err(X86_64StructuralUnitCallTemplateError::MalformedTemplate);
    }
    Ok(ValidatedX86_64SelectedStructuralUnitCallTemplate {
        bytes: bytes.to_vec(),
        footprint: decoded,
        fixup: X86_64StructuralUnitInternalControlFixup {
            kind: X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1,
            state: X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1,
            callee: call.callee,
            opcode_byte_offset: X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET,
            field_byte_offset: X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET,
            next_instruction_byte_offset: X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET,
            field_byte_width: X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH,
            addend: 0,
        },
    })
}

/// Resolve the canonical zero rel32 field once dense text placement has made
/// both function coordinates available. The returned bytes are executable;
/// the input template remains unresolved and unchanged.
pub fn resolve_x86_64_structural_unit_internal_call(
    template: &ValidatedX86_64SelectedStructuralUnitCallTemplate,
    fixup: X86_64StructuralUnitInternalControlFixup,
    caller_section_offset: u64,
    callee_section_offset: u64,
) -> Result<
    ValidatedX86_64ResolvedStructuralUnitCall,
    X86_64StructuralUnitInternalControlResolutionError,
> {
    validate_resolution_fixup(template, fixup)?;
    let next_instruction_section_offset = caller_section_offset
        .checked_add(u64::from(fixup.next_instruction_byte_offset))
        .ok_or(X86_64StructuralUnitInternalControlResolutionError::SectionCoordinateOverflow)?;
    let displacement = checked_rel32_displacement(
        next_instruction_section_offset,
        callee_section_offset,
        fixup.addend,
    )?;
    let field_start = usize::from(fixup.field_byte_offset);
    let field_end = field_start
        .checked_add(usize::from(fixup.field_byte_width))
        .ok_or(X86_64StructuralUnitInternalControlResolutionError::MalformedResolvedBytes)?;
    let mut bytes = template.bytes().to_vec();
    bytes
        .get_mut(field_start..field_end)
        .ok_or(X86_64StructuralUnitInternalControlResolutionError::MalformedResolvedBytes)?
        .copy_from_slice(&displacement.to_le_bytes());

    validate_x86_64_resolved_structural_unit_internal_call(
        template,
        fixup,
        caller_section_offset,
        callee_section_offset,
        &bytes,
    )
}

/// Independently replay a structural Unit rel32 resolution. This validates the
/// opcode, every template byte outside the field, the signed little-endian
/// displacement, and the final target equation.
pub fn validate_x86_64_resolved_structural_unit_internal_call(
    template: &ValidatedX86_64SelectedStructuralUnitCallTemplate,
    fixup: X86_64StructuralUnitInternalControlFixup,
    caller_section_offset: u64,
    callee_section_offset: u64,
    bytes: &[u8],
) -> Result<
    ValidatedX86_64ResolvedStructuralUnitCall,
    X86_64StructuralUnitInternalControlResolutionError,
> {
    validate_resolution_fixup(template, fixup)?;
    let next_instruction_section_offset = caller_section_offset
        .checked_add(u64::from(fixup.next_instruction_byte_offset))
        .ok_or(X86_64StructuralUnitInternalControlResolutionError::SectionCoordinateOverflow)?;
    let expected_displacement = checked_rel32_displacement(
        next_instruction_section_offset,
        callee_section_offset,
        fixup.addend,
    )?;

    let field_start = usize::from(fixup.field_byte_offset);
    let field_end = field_start
        .checked_add(usize::from(fixup.field_byte_width))
        .ok_or(X86_64StructuralUnitInternalControlResolutionError::MalformedResolvedBytes)?;
    let opcode_offset = usize::from(fixup.opcode_byte_offset);
    if bytes.len() != template.bytes().len()
        || bytes.get(opcode_offset) != Some(&0xe8)
        || bytes.get(..field_start) != template.bytes().get(..field_start)
        || bytes.get(field_end..) != template.bytes().get(field_end..)
    {
        return Err(X86_64StructuralUnitInternalControlResolutionError::MalformedResolvedBytes);
    }
    let displacement = bytes
        .get(field_start..field_end)
        .and_then(|field| field.try_into().ok())
        .map(i32::from_le_bytes)
        .ok_or(X86_64StructuralUnitInternalControlResolutionError::MalformedResolvedBytes)?;
    if displacement != expected_displacement {
        return Err(X86_64StructuralUnitInternalControlResolutionError::TargetEquationMismatch);
    }
    let replayed_target = i128::from(next_instruction_section_offset) + i128::from(displacement)
        - i128::from(fixup.addend);
    if replayed_target != i128::from(callee_section_offset) {
        return Err(X86_64StructuralUnitInternalControlResolutionError::TargetEquationMismatch);
    }

    Ok(ValidatedX86_64ResolvedStructuralUnitCall {
        bytes: bytes.to_vec(),
        footprint: template.footprint().clone(),
        resolution: X86_64ResolvedStructuralUnitInternalControlFixup {
            source: fixup,
            state: X86_64StructuralUnitInternalControlResolutionState::ResolvedInSectionV1,
            caller_section_offset,
            callee_section_offset,
            next_instruction_section_offset,
            displacement,
        },
    })
}

fn validate_resolution_fixup(
    template: &ValidatedX86_64SelectedStructuralUnitCallTemplate,
    fixup: X86_64StructuralUnitInternalControlFixup,
) -> Result<(), X86_64StructuralUnitInternalControlResolutionError> {
    if fixup != template.fixup()
        || fixup.kind
            != X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1
        || fixup.state != X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1
        || fixup.opcode_byte_offset != X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET
        || fixup.field_byte_offset != X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET
        || fixup.next_instruction_byte_offset
            != X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET
        || fixup.field_byte_width != X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH
        || template
            .bytes()
            .get(usize::from(fixup.opcode_byte_offset))
            != Some(&0xe8)
        || template
            .bytes()
            .get(
                usize::from(fixup.field_byte_offset)
                    ..usize::from(fixup.next_instruction_byte_offset),
            )
            != Some(&[0, 0, 0, 0])
    {
        return Err(X86_64StructuralUnitInternalControlResolutionError::FixupMismatch);
    }
    Ok(())
}

fn checked_rel32_displacement(
    next_instruction_section_offset: u64,
    callee_section_offset: u64,
    addend: i64,
) -> Result<i32, X86_64StructuralUnitInternalControlResolutionError> {
    let displacement = i128::from(callee_section_offset)
        - i128::from(next_instruction_section_offset)
        + i128::from(addend);
    i32::try_from(displacement).map_err(|_| {
        X86_64StructuralUnitInternalControlResolutionError::RelativeDisplacementOutOfRange
    })
}

fn validate_structural_unit_call_request(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    call: &SelectedStructuralUnitCallInstruction,
    declaration: StructuralUnitCallEffectDeclaration,
) -> Result<(), X86_64StructuralUnitCallTemplateError> {
    if target.architecture != Architecture::X86_64
        || target.object_format != ObjectFormat::Coff
        || target.pointer_size != 8
        || target.pointer_alignment != 8
    {
        return Err(X86_64StructuralUnitCallTemplateError::UnsupportedTarget);
    }
    if physical.model() != &x86_64_physical_register_model() {
        return Err(X86_64StructuralUnitCallTemplateError::NonCanonicalPhysicalModel);
    }
    if constraints.architecture() != Architecture::X86_64
        || constraints.physical_identity() != physical.identity()
        || constraints.catalog() != &x86_64_register_constraint_catalog(physical)
    {
        return Err(X86_64StructuralUnitCallTemplateError::NonCanonicalConstraintCatalog);
    }
    let row = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR)
        .ok_or(X86_64StructuralUnitCallTemplateError::ConstraintMismatch)?;
    if !row.operands.is_empty()
        || call.constraint != row.key
        || call.implicit_uses != row.implicit_uses
        || call.implicit_defs != row.implicit_defs
        || call.clobbers != row.clobbers
    {
        return Err(X86_64StructuralUnitCallTemplateError::ConstraintMismatch);
    }
    if [&call.caller_call_plan, &call.callee_call_plan]
        .into_iter()
        .any(|plan| {
            plan.policy != CallingPolicy::MicrosoftX64
                || plan.result.is_some()
                || !plan.callback_materializations.is_empty()
                || plan.stack_alignment != 16
                || plan.shadow_bytes != 32
                || plan.entry_control != EntryControl::CallReturn
        })
    {
        return Err(X86_64StructuralUnitCallTemplateError::CallPlanMismatch);
    }
    let expected_layout =
        omega_selected_instructions::SelectedMicrosoftX64OwnedIndirectPairLayout {
            shadow_byte_count: 32,
            outgoing_frame_byte_count: 72,
            pre_call_stack_alignment: 16,
            bindings: [
                SelectedStructuralUnitIndirectBinding {
                    parameter_index: 0,
                    pointer: MachineRegister::X86Rcx,
                    copy_stack_byte_offset: 32,
                    byte_count: 16,
                    alignment: 8,
                },
                SelectedStructuralUnitIndirectBinding {
                    parameter_index: 1,
                    pointer: MachineRegister::X86Rdx,
                    copy_stack_byte_offset: 48,
                    byte_count: 16,
                    alignment: 8,
                },
            ],
        };
    if call.layout != expected_layout {
        return Err(X86_64StructuralUnitCallTemplateError::LayoutMismatch);
    }
    let expected_declaration = StructuralUnitCallEffectDeclaration {
        constraint: X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR,
        memory: StructuralUnitCallMemoryEffect::ReadOwnedIndirectPairWriteCallerCopiesV1 {
            root_byte_count: 16,
            copy_stack_byte_offsets: [32, 48],
        },
        frame: StructuralUnitCallFrameEffect::BalancedCallerFrameV1 {
            frame_byte_count: 72,
            shadow_byte_count: 32,
            pre_call_stack_alignment: 16,
        },
        trap: MachineTrapBehavior::MayArchitecturalFaultV1,
        barrier: StructuralUnitCallBarrier::CallV1,
        call: StructuralUnitCallEffect::DirectInternalUnitV1,
        cleanup: MachineCleanupEffect::NoneV1,
    };
    if declaration != expected_declaration {
        return Err(X86_64StructuralUnitCallTemplateError::EffectMismatch);
    }
    Ok(())
}

fn expected_structural_unit_call_footprint(
    call: &SelectedStructuralUnitCallInstruction,
    declaration: StructuralUnitCallEffectDeclaration,
) -> X86_64SelectedStructuralUnitCallFootprint {
    let StructuralUnitCallFrameEffect::BalancedCallerFrameV1 {
        frame_byte_count,
        shadow_byte_count,
        pre_call_stack_alignment,
    } = declaration.frame;
    X86_64SelectedStructuralUnitCallFootprint {
        implicit_unit_uses: call.implicit_uses.clone(),
        implicit_unit_defs: call.implicit_defs.clone(),
        implicit_unit_clobbers: call.clobbers.clone(),
        root_reads: [
            X86_64StructuralUnitRootRead {
                root: MachineRegister::X86Rcx,
                byte_offset: 0,
                byte_count: 8,
            },
            X86_64StructuralUnitRootRead {
                root: MachineRegister::X86Rcx,
                byte_offset: 8,
                byte_count: 8,
            },
            X86_64StructuralUnitRootRead {
                root: MachineRegister::X86Rdx,
                byte_offset: 0,
                byte_count: 8,
            },
            X86_64StructuralUnitRootRead {
                root: MachineRegister::X86Rdx,
                byte_offset: 8,
                byte_count: 8,
            },
        ],
        caller_copy_writes: [
            X86_64StructuralUnitCallerCopyWrite {
                stack_byte_offset: 32,
                byte_count: 8,
            },
            X86_64StructuralUnitCallerCopyWrite {
                stack_byte_offset: 40,
                byte_count: 8,
            },
            X86_64StructuralUnitCallerCopyWrite {
                stack_byte_offset: 48,
                byte_count: 8,
            },
            X86_64StructuralUnitCallerCopyWrite {
                stack_byte_offset: 56,
                byte_count: 8,
            },
        ],
        scratch_register_writes: [MachineRegister::X86Rax],
        argument_pointer_writes: [
            X86_64StructuralUnitArgumentPointerWrite {
                register: MachineRegister::X86Rcx,
                stack_byte_offset: 32,
            },
            X86_64StructuralUnitArgumentPointerWrite {
                register: MachineRegister::X86Rdx,
                stack_byte_offset: 48,
            },
        ],
        writes_rflags: true,
        frame_byte_count,
        shadow_byte_count,
        pre_call_stack_alignment,
        frame_is_balanced: true,
        trap: declaration.trap,
        barrier: declaration.barrier,
        call: declaration.call,
        cleanup: declaration.cleanup,
    }
}

fn decode_structural_unit_call_template(
    bytes: &[u8],
    call: &SelectedStructuralUnitCallInstruction,
    declaration: StructuralUnitCallEffectDeclaration,
) -> Result<X86_64SelectedStructuralUnitCallFootprint, X86_64StructuralUnitCallTemplateError> {
    if bytes.len() != X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT {
        return Err(X86_64StructuralUnitCallTemplateError::MalformedTemplate);
    }
    let mut cursor = StructuralTemplateCursor { bytes, offset: 0 };
    cursor.expect(&[0x48, 0x83, 0xec])?;
    let reserved = u32::from(cursor.byte()?);
    let mut root_reads = Vec::with_capacity(4);
    let mut caller_copy_writes = Vec::with_capacity(4);
    for _ in 0..4 {
        cursor.expect(&[0x48, 0x8b])?;
        let root = match cursor.byte()? {
            0x81 => MachineRegister::X86Rcx,
            0x82 => MachineRegister::X86Rdx,
            _ => return Err(X86_64StructuralUnitCallTemplateError::MalformedTemplate),
        };
        let source_offset = cursor.u32()?;
        cursor.expect(&[0x48, 0x89, 0x84, 0x24])?;
        let stack_byte_offset = cursor.u32()?;
        root_reads.push(X86_64StructuralUnitRootRead {
            root,
            byte_offset: source_offset,
            byte_count: 8,
        });
        caller_copy_writes.push(X86_64StructuralUnitCallerCopyWrite {
            stack_byte_offset,
            byte_count: 8,
        });
    }
    let mut argument_pointer_writes = Vec::with_capacity(2);
    for (prefix, register) in [
        ([0x48, 0x8d, 0x8c, 0x24], MachineRegister::X86Rcx),
        ([0x48, 0x8d, 0x94, 0x24], MachineRegister::X86Rdx),
    ] {
        cursor.expect(&prefix)?;
        argument_pointer_writes.push(X86_64StructuralUnitArgumentPointerWrite {
            register,
            stack_byte_offset: cursor.u32()?,
        });
    }
    if cursor.offset != usize::from(X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET) {
        return Err(X86_64StructuralUnitCallTemplateError::MalformedTemplate);
    }
    cursor.expect(&[0xe8])?;
    if cursor.offset != usize::from(X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET)
        || cursor.u32()? != 0
        || cursor.offset != usize::from(X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET)
    {
        return Err(X86_64StructuralUnitCallTemplateError::MalformedTemplate);
    }
    cursor.expect(&[0x48, 0x83, 0xc4])?;
    let released = u32::from(cursor.byte()?);
    if cursor.offset != bytes.len() {
        return Err(X86_64StructuralUnitCallTemplateError::MalformedTemplate);
    }

    Ok(X86_64SelectedStructuralUnitCallFootprint {
        implicit_unit_uses: call.implicit_uses.clone(),
        implicit_unit_defs: call.implicit_defs.clone(),
        implicit_unit_clobbers: call.clobbers.clone(),
        root_reads: root_reads
            .try_into()
            .map_err(|_| X86_64StructuralUnitCallTemplateError::MalformedTemplate)?,
        caller_copy_writes: caller_copy_writes
            .try_into()
            .map_err(|_| X86_64StructuralUnitCallTemplateError::MalformedTemplate)?,
        scratch_register_writes: [MachineRegister::X86Rax],
        argument_pointer_writes: argument_pointer_writes
            .try_into()
            .map_err(|_| X86_64StructuralUnitCallTemplateError::MalformedTemplate)?,
        writes_rflags: true,
        frame_byte_count: reserved,
        shadow_byte_count: 32,
        pre_call_stack_alignment: 16,
        frame_is_balanced: reserved == released,
        trap: declaration.trap,
        barrier: declaration.barrier,
        call: declaration.call,
        cleanup: declaration.cleanup,
    })
}

struct StructuralTemplateCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl StructuralTemplateCursor<'_> {
    fn expect(&mut self, expected: &[u8]) -> Result<(), X86_64StructuralUnitCallTemplateError> {
        let end = self
            .offset
            .checked_add(expected.len())
            .ok_or(X86_64StructuralUnitCallTemplateError::MalformedTemplate)?;
        if self.bytes.get(self.offset..end) != Some(expected) {
            return Err(X86_64StructuralUnitCallTemplateError::MalformedTemplate);
        }
        self.offset = end;
        Ok(())
    }

    fn byte(&mut self) -> Result<u8, X86_64StructuralUnitCallTemplateError> {
        let byte = *self
            .bytes
            .get(self.offset)
            .ok_or(X86_64StructuralUnitCallTemplateError::MalformedTemplate)?;
        self.offset += 1;
        Ok(byte)
    }

    fn u32(&mut self) -> Result<u32, X86_64StructuralUnitCallTemplateError> {
        let end = self
            .offset
            .checked_add(4)
            .ok_or(X86_64StructuralUnitCallTemplateError::MalformedTemplate)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_le_bytes)
            .ok_or(X86_64StructuralUnitCallTemplateError::MalformedTemplate)?;
        self.offset = end;
        Ok(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecodedInstruction {
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
    if matches!(opcode, 0x89 | 0x85 | 0x31 | 0x29 | 0x01 | 0xf7) {
        if mode != 3 || bytes.len() < 3 {
            return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
        }
        let decoded = match opcode {
            0x89 => DecodedInstruction::Move {
                source: reg,
                destination: rm,
            },
            0x85 if reg == rm => DecodedInstruction::Test { register: reg },
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
        SelectedInstructionKind::ExactAddI64 { .. } => {
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
        SelectedInstructionKind::ReturnI64 | SelectedInstructionKind::ReturnUnit => {
            decoded == [DecodedInstruction::Return]
        }
        SelectedInstructionKind::ConditionalBranchNonZero => false,
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
        SelectedInstructionKind::CopyI64 => (vec![operands[0]], vec![operands[1]], false),
        SelectedInstructionKind::CompareI64Zero => (vec![operands[0]], vec![], true),
        SelectedInstructionKind::ExactAddI64 { .. } => {
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
        SelectedInstructionKind::ReturnI64 | SelectedInstructionKind::ReturnUnit => {
            (vec![], vec![], false)
        }
        SelectedInstructionKind::ConditionalBranchNonZero => (vec![], vec![], false),
    };
    let physical = x86_64_physical_register_model();
    let units = |name: &str| physical.view_named(name).unwrap().units.clone();
    let encoded = if matches!(
        kind,
        SelectedInstructionKind::ReturnI64 | SelectedInstructionKind::ReturnUnit
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
    } else if matches!(kind, SelectedInstructionKind::ConditionalBranchNonZero) {
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
                | SelectedInstructionKind::CompareI64Zero
                | SelectedInstructionKind::ExactAddI64Immediate { .. }
                | SelectedInstructionKind::ExactSubtractI64Immediate { .. } => vec![0],
                SelectedInstructionKind::ExactAddI64 { .. } => vec![0, 1],
                SelectedInstructionKind::ExactSubtractI64 { .. } if alternative.variant == 0 => {
                    vec![]
                }
                SelectedInstructionKind::ExactSubtractI64 { .. } => vec![0, 1],
                _ => unreachable!("control forms handled separately"),
            },
            match kind {
                SelectedInstructionKind::MaterializeI64 { .. } => vec![0],
                SelectedInstructionKind::CopyI64
                | SelectedInstructionKind::ExactAddI64Immediate { .. }
                | SelectedInstructionKind::ExactSubtractI64Immediate { .. } => vec![1],
                SelectedInstructionKind::ExactAddI64 { .. }
                | SelectedInstructionKind::ExactSubtractI64 { .. } => vec![2],
                SelectedInstructionKind::CompareI64Zero => vec![],
                _ => unreachable!("control forms handled separately"),
            },
        );
        if matches!(kind, SelectedInstructionKind::CompareI64Zero) {
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
    use omega_calling_conventions::{CallPlan, RegisterSet};
    use omega_optimization_core::AcceptedObligationFactIdentity;
    use omega_optimization_unit::EffectLink;
    use omega_register_model::{
        ValidatedRegisterConstraintCatalog, validate_physical_register_model,
    };
    use psi_core::{ObligationId, OperationId};

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

    fn structural_call_plan() -> CallPlan {
        CallPlan {
            policy: CallingPolicy::MicrosoftX64,
            parameters: Vec::new(),
            result: None,
            callback_materializations: Vec::new(),
            ordinary_clobbers: RegisterSet::default(),
            stack_alignment: 16,
            shadow_bytes: 32,
            entry_control: EntryControl::CallReturn,
        }
    }

    fn structural_layout()
    -> omega_selected_instructions::SelectedMicrosoftX64OwnedIndirectPairLayout {
        omega_selected_instructions::SelectedMicrosoftX64OwnedIndirectPairLayout {
            shadow_byte_count: 32,
            outgoing_frame_byte_count: 72,
            pre_call_stack_alignment: 16,
            bindings: [
                SelectedStructuralUnitIndirectBinding {
                    parameter_index: 0,
                    pointer: MachineRegister::X86Rcx,
                    copy_stack_byte_offset: 32,
                    byte_count: 16,
                    alignment: 8,
                },
                SelectedStructuralUnitIndirectBinding {
                    parameter_index: 1,
                    pointer: MachineRegister::X86Rdx,
                    copy_stack_byte_offset: 48,
                    byte_count: 16,
                    alignment: 8,
                },
            ],
        }
    }

    fn structural_declaration() -> StructuralUnitCallEffectDeclaration {
        StructuralUnitCallEffectDeclaration {
            constraint: X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR,
            memory: StructuralUnitCallMemoryEffect::ReadOwnedIndirectPairWriteCallerCopiesV1 {
                root_byte_count: 16,
                copy_stack_byte_offsets: [32, 48],
            },
            frame: StructuralUnitCallFrameEffect::BalancedCallerFrameV1 {
                frame_byte_count: 72,
                shadow_byte_count: 32,
                pre_call_stack_alignment: 16,
            },
            trap: MachineTrapBehavior::MayArchitecturalFaultV1,
            barrier: StructuralUnitCallBarrier::CallV1,
            call: StructuralUnitCallEffect::DirectInternalUnitV1,
            cleanup: MachineCleanupEffect::NoneV1,
        }
    }

    fn structural_fixture() -> (
        ValidatedPhysicalRegisterModel,
        ValidatedRegisterConstraintCatalog,
        SelectedStructuralUnitCallInstruction,
    ) {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let constraints = crate::validate_x86_64_register_constraint_catalog(
            x86_64_register_constraint_catalog(&physical),
            &physical,
        )
        .unwrap();
        let row = constraints
            .catalog()
            .constraints
            .iter()
            .find(|row| row.key == X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR)
            .unwrap();
        let call = SelectedStructuralUnitCallInstruction {
            id: omega_selected_instructions::SelectedInstructionId(0),
            source: omega_selected_instructions::SelectedStructuralUnitCallSource::AuthoredCallUnit,
            operation: OperationId::new(1).unwrap(),
            callee: MachineId::new(2).unwrap(),
            caller_call_plan: structural_call_plan(),
            callee_call_plan: structural_call_plan(),
            arguments: Vec::new(),
            claim_transfers: Vec::new(),
            layout: structural_layout(),
            constraint: row.key,
            implicit_uses: row.implicit_uses.clone(),
            implicit_defs: row.implicit_defs.clone(),
            clobbers: row.clobbers.clone(),
            provenance: Default::default(),
            effect: EffectLink {
                input: 10,
                output: 11,
            },
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            ownership: Vec::new(),
        };
        (physical, constraints, call)
    }

    #[test]
    fn structural_unit_call_template_is_exact_and_explicitly_unresolved() {
        let (physical, constraints, call) = structural_fixture();
        let encoded = encode_x86_64_selected_structural_unit_call_template(
            NativeTarget::uefi_x64(),
            &physical,
            &constraints,
            &call,
            structural_declaration(),
        )
        .unwrap();
        let expected = [
            0x48, 0x83, 0xec, 0x48, 0x48, 0x8b, 0x81, 0x00, 0x00, 0x00, 0x00, 0x48, 0x89, 0x84,
            0x24, 0x20, 0x00, 0x00, 0x00, 0x48, 0x8b, 0x81, 0x08, 0x00, 0x00, 0x00, 0x48, 0x89,
            0x84, 0x24, 0x28, 0x00, 0x00, 0x00, 0x48, 0x8b, 0x82, 0x00, 0x00, 0x00, 0x00, 0x48,
            0x89, 0x84, 0x24, 0x30, 0x00, 0x00, 0x00, 0x48, 0x8b, 0x82, 0x08, 0x00, 0x00, 0x00,
            0x48, 0x89, 0x84, 0x24, 0x38, 0x00, 0x00, 0x00, 0x48, 0x8d, 0x8c, 0x24, 0x20, 0x00,
            0x00, 0x00, 0x48, 0x8d, 0x94, 0x24, 0x30, 0x00, 0x00, 0x00, 0xe8, 0x00, 0x00, 0x00,
            0x00, 0x48, 0x83, 0xc4, 0x48,
        ];
        assert_eq!(encoded.bytes(), expected);
        assert_eq!(
            encoded.bytes().len(),
            X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT
        );
        assert_eq!(
            encoded.bytes()[usize::from(X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET)],
            0xe8
        );
        assert_eq!(
            &encoded.bytes()[usize::from(X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET)
                ..usize::from(X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET)],
            [0, 0, 0, 0]
        );
        assert_eq!(
            encoded.fixup(),
            X86_64StructuralUnitInternalControlFixup {
                kind: X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1,
                state: X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1,
                callee: call.callee,
                opcode_byte_offset: 80,
                field_byte_offset: 81,
                next_instruction_byte_offset: 85,
                field_byte_width: 4,
                addend: 0,
            }
        );
        assert_eq!(
            encoded.footprint().root_reads[0].root,
            MachineRegister::X86Rcx
        );
        assert_eq!(encoded.footprint().root_reads[3].byte_offset, 8);
        assert_eq!(
            encoded.footprint().caller_copy_writes[3].stack_byte_offset,
            56
        );
        assert_eq!(
            encoded.footprint().scratch_register_writes,
            [MachineRegister::X86Rax]
        );
        assert_eq!(encoded.footprint().frame_byte_count, 72);
        assert!(encoded.footprint().frame_is_balanced);
        assert!(encoded.footprint().writes_rflags);
        assert_eq!(encoded.footprint().implicit_unit_uses, call.implicit_uses);
        assert_eq!(encoded.footprint().implicit_unit_defs, call.implicit_defs);
        assert_eq!(encoded.footprint().implicit_unit_clobbers, call.clobbers);
    }

    #[test]
    fn structural_unit_call_decoder_rejects_every_byte_corruption() {
        let (physical, constraints, call) = structural_fixture();
        let declaration = structural_declaration();
        let encoded = encode_x86_64_selected_structural_unit_call_template(
            NativeTarget::windows_x64(),
            &physical,
            &constraints,
            &call,
            declaration,
        )
        .unwrap();
        for index in 0..encoded.bytes().len() {
            let mut corrupted = encoded.bytes().to_vec();
            corrupted[index] ^= 1;
            assert_eq!(
                validate_x86_64_selected_structural_unit_call_template(
                    NativeTarget::windows_x64(),
                    &physical,
                    &constraints,
                    &call,
                    declaration,
                    &corrupted,
                ),
                Err(X86_64StructuralUnitCallTemplateError::MalformedTemplate),
                "byte {index} was not independently rejected"
            );
        }
    }

    #[test]
    fn structural_unit_call_rejects_target_constraint_layout_effect_and_plan_drift() {
        let (physical, constraints, call) = structural_fixture();
        let declaration = structural_declaration();
        assert_eq!(
            encode_x86_64_selected_structural_unit_call_template(
                NativeTarget::linux_x64(),
                &physical,
                &constraints,
                &call,
                declaration,
            ),
            Err(X86_64StructuralUnitCallTemplateError::UnsupportedTarget)
        );

        let mut wrong_constraint = call.clone();
        wrong_constraint.implicit_uses.pop();
        assert_eq!(
            encode_x86_64_selected_structural_unit_call_template(
                NativeTarget::uefi_x64(),
                &physical,
                &constraints,
                &wrong_constraint,
                declaration,
            ),
            Err(X86_64StructuralUnitCallTemplateError::ConstraintMismatch)
        );

        let mut wrong_layout = call.clone();
        wrong_layout.layout.bindings[1].copy_stack_byte_offset = 56;
        assert_eq!(
            encode_x86_64_selected_structural_unit_call_template(
                NativeTarget::uefi_x64(),
                &physical,
                &constraints,
                &wrong_layout,
                declaration,
            ),
            Err(X86_64StructuralUnitCallTemplateError::LayoutMismatch)
        );

        let mut wrong_effect = declaration;
        wrong_effect.frame = StructuralUnitCallFrameEffect::BalancedCallerFrameV1 {
            frame_byte_count: 88,
            shadow_byte_count: 32,
            pre_call_stack_alignment: 16,
        };
        assert_eq!(
            encode_x86_64_selected_structural_unit_call_template(
                NativeTarget::uefi_x64(),
                &physical,
                &constraints,
                &call,
                wrong_effect,
            ),
            Err(X86_64StructuralUnitCallTemplateError::EffectMismatch)
        );

        let mut wrong_plan = call.clone();
        wrong_plan.caller_call_plan.shadow_bytes = 0;
        assert_eq!(
            encode_x86_64_selected_structural_unit_call_template(
                NativeTarget::uefi_x64(),
                &physical,
                &constraints,
                &wrong_plan,
                declaration,
            ),
            Err(X86_64StructuralUnitCallTemplateError::CallPlanMismatch)
        );
    }

    #[test]
    fn structural_unit_call_fixup_binds_the_selected_callee() {
        let (physical, constraints, mut call) = structural_fixture();
        call.callee = MachineId::new(99).unwrap();
        let encoded = encode_x86_64_selected_structural_unit_call_template(
            NativeTarget::uefi_x64(),
            &physical,
            &constraints,
            &call,
            structural_declaration(),
        )
        .unwrap();
        assert_eq!(encoded.fixup().callee, MachineId::new(99).unwrap());
        assert_eq!(encoded.bytes()[81..85], [0, 0, 0, 0]);
    }

    #[test]
    fn structural_unit_call_rel32_resolves_the_canonical_forward_fixture() {
        let (physical, constraints, call) = structural_fixture();
        let template = encode_x86_64_selected_structural_unit_call_template(
            NativeTarget::uefi_x64(),
            &physical,
            &constraints,
            &call,
            structural_declaration(),
        )
        .unwrap();
        let resolved =
            resolve_x86_64_structural_unit_internal_call(&template, template.fixup(), 0, 90)
                .unwrap();

        assert_eq!(resolved.bytes()[81..85], [5, 0, 0, 0]);
        assert_eq!(resolved.bytes()[80], 0xe8);
        assert_eq!(resolved.footprint(), template.footprint());
        assert_eq!(
            resolved.resolution(),
            X86_64ResolvedStructuralUnitInternalControlFixup {
                source: template.fixup(),
                state: X86_64StructuralUnitInternalControlResolutionState::ResolvedInSectionV1,
                caller_section_offset: 0,
                callee_section_offset: 90,
                next_instruction_section_offset: 85,
                displacement: 5,
            }
        );
        assert_eq!(template.bytes()[81..85], [0, 0, 0, 0]);
    }

    #[test]
    fn structural_unit_call_rel32_supports_backward_targets() {
        let (physical, constraints, call) = structural_fixture();
        let template = encode_x86_64_selected_structural_unit_call_template(
            NativeTarget::windows_x64(),
            &physical,
            &constraints,
            &call,
            structural_declaration(),
        )
        .unwrap();
        let resolved =
            resolve_x86_64_structural_unit_internal_call(&template, template.fixup(), 200, 100)
                .unwrap();

        assert_eq!(resolved.resolution().next_instruction_section_offset, 285);
        assert_eq!(resolved.resolution().displacement, -185);
        assert_eq!(resolved.bytes()[81..85], (-185_i32).to_le_bytes());
    }

    #[test]
    fn structural_unit_call_rel32_rejects_overflow_and_fixup_drift() {
        let (physical, constraints, call) = structural_fixture();
        let template = encode_x86_64_selected_structural_unit_call_template(
            NativeTarget::uefi_x64(),
            &physical,
            &constraints,
            &call,
            structural_declaration(),
        )
        .unwrap();

        assert_eq!(
            resolve_x86_64_structural_unit_internal_call(&template, template.fixup(), 0, u64::MAX,),
            Err(X86_64StructuralUnitInternalControlResolutionError::RelativeDisplacementOutOfRange)
        );
        assert_eq!(
            resolve_x86_64_structural_unit_internal_call(&template, template.fixup(), u64::MAX, 0,),
            Err(X86_64StructuralUnitInternalControlResolutionError::SectionCoordinateOverflow)
        );

        let mut wrong_fixup = template.fixup();
        wrong_fixup.addend = 1;
        assert_eq!(
            resolve_x86_64_structural_unit_internal_call(&template, wrong_fixup, 0, 90),
            Err(X86_64StructuralUnitInternalControlResolutionError::FixupMismatch)
        );
    }

    #[test]
    fn structural_unit_call_resolution_replay_rejects_every_byte_corruption() {
        let (physical, constraints, call) = structural_fixture();
        let template = encode_x86_64_selected_structural_unit_call_template(
            NativeTarget::uefi_x64(),
            &physical,
            &constraints,
            &call,
            structural_declaration(),
        )
        .unwrap();
        let resolved =
            resolve_x86_64_structural_unit_internal_call(&template, template.fixup(), 0, 90)
                .unwrap();

        for index in 0..resolved.bytes().len() {
            let mut corrupted = resolved.bytes().to_vec();
            corrupted[index] ^= 1;
            let result = validate_x86_64_resolved_structural_unit_internal_call(
                &template,
                template.fixup(),
                0,
                90,
                &corrupted,
            );
            assert!(
                result.is_err(),
                "byte {index} was not independently rejected"
            );
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
}
