//! Target-owned encoding for one compiler-private semantic Unit wrapper.
//!
//! The semantic ProgramStorage plan deliberately owns no byte coordinates.
//! This module selects the compact Microsoft-x64 realization whose body mutates
//! no floating-point control state and uses only caller-saved registers. The
//! unresolved rel32 field is not executable authority until section placement
//! binds it to one exact private continuation offset.

use omega_calling_conventions::{MachineRegister, RegisterSet};
use omega_target::NativeTarget;

pub const X86_64_SEMANTIC_UNIT_WRAPPER_CALL_BUNDLE_BYTE_COUNT: usize = 89;
pub const X86_64_SEMANTIC_UNIT_WRAPPER_FUNCTION_BYTE_COUNT: usize = 90;
pub const X86_64_SEMANTIC_UNIT_WRAPPER_CALL_OPCODE_OFFSET: u16 = 80;
pub const X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_OFFSET: u16 = 81;
pub const X86_64_SEMANTIC_UNIT_WRAPPER_NEXT_INSTRUCTION_OFFSET: u16 = 85;
pub const X86_64_SEMANTIC_UNIT_WRAPPER_RETURN_OFFSET: u16 = 89;
pub const X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_WIDTH: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64SemanticUnitWrapperEncodingPolicy {
    /// The wrapper touches RAX, RCX, RDX, RSP, RFLAGS, and memory only. It does
    /// not install Omega's ordinary 33-byte control-state/nonvolatile envelope.
    MicrosoftX64CallerSavedOnlyNoControlStateMutationV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64SemanticUnitWrapperCopy {
    pub source_register: MachineRegister,
    pub source_byte_offset: u32,
    pub outgoing_stack_byte_offset: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64SemanticUnitWrapperArgumentBinding {
    pub register: MachineRegister,
    pub outgoing_stack_byte_offset: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64SemanticUnitWrapperEncodingRequest {
    pub target: NativeTarget,
    pub policy: X86_64SemanticUnitWrapperEncodingPolicy,
    pub shadow_byte_count: u32,
    pub outgoing_frame_byte_count: u32,
    pub outgoing_release_byte_count: u32,
    pub pre_call_stack_alignment: u16,
    pub copies: [X86_64SemanticUnitWrapperCopy; 4],
    pub argument_bindings: [X86_64SemanticUnitWrapperArgumentBinding; 2],
    pub relocation_field_byte_width: u8,
    pub relocation_addend: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64SemanticUnitWrapperTrapBehavior {
    MayArchitecturalFaultV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64SemanticUnitWrapperCallEffect {
    DirectPrivateContinuationUnitV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64SemanticUnitWrapperCleanupEffect {
    NoneV1,
}

/// Physical effects retained beside the exact target bytes. This is target
/// validation evidence, not permission to invoke the wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64SemanticUnitWrapperFootprint {
    pub root_reads: [X86_64SemanticUnitWrapperCopy; 4],
    pub caller_copy_writes: [X86_64SemanticUnitWrapperCopy; 4],
    pub scratch_register_writes: RegisterSet,
    pub argument_pointer_writes: [X86_64SemanticUnitWrapperArgumentBinding; 2],
    pub call_clobbers: RegisterSet,
    pub writes_stack_pointer: bool,
    pub writes_instruction_pointer: bool,
    pub writes_flags: bool,
    pub mutates_control_state: bool,
    pub frame_byte_count: u32,
    pub shadow_byte_count: u32,
    pub pre_call_stack_alignment: u16,
    pub frame_is_balanced: bool,
    pub trap: X86_64SemanticUnitWrapperTrapBehavior,
    pub call: X86_64SemanticUnitWrapperCallEffect,
    pub cleanup: X86_64SemanticUnitWrapperCleanupEffect,
    pub returns_unit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64SemanticUnitWrapperRelocationKind {
    Relative32PrivateContinuationFromNextInstructionV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64SemanticUnitWrapperRelocationState {
    UnresolvedZeroFieldV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64SemanticUnitWrapperRelocation {
    pub kind: X86_64SemanticUnitWrapperRelocationKind,
    pub state: X86_64SemanticUnitWrapperRelocationState,
    pub opcode_function_byte_offset: u16,
    pub field_function_byte_offset: u16,
    pub next_instruction_function_byte_offset: u16,
    pub field_byte_width: u8,
    pub addend: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64SemanticUnitWrapperTemplate {
    request: X86_64SemanticUnitWrapperEncodingRequest,
    bytes: Vec<u8>,
    footprint: X86_64SemanticUnitWrapperFootprint,
    relocation: X86_64SemanticUnitWrapperRelocation,
}

impl ValidatedX86_64SemanticUnitWrapperTemplate {
    pub const fn request(&self) -> X86_64SemanticUnitWrapperEncodingRequest {
        self.request
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn footprint(&self) -> &X86_64SemanticUnitWrapperFootprint {
        &self.footprint
    }

    pub const fn relocation(&self) -> X86_64SemanticUnitWrapperRelocation {
        self.relocation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64SemanticUnitWrapperResolutionState {
    ResolvedInSectionV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64SemanticUnitWrapperResolution {
    pub source: X86_64SemanticUnitWrapperRelocation,
    pub state: X86_64SemanticUnitWrapperResolutionState,
    pub wrapper_section_offset: u64,
    pub continuation_section_offset: u64,
    pub next_instruction_section_offset: u64,
    pub displacement: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64ResolvedSemanticUnitWrapper {
    bytes: Vec<u8>,
    resolution: X86_64SemanticUnitWrapperResolution,
}

impl ValidatedX86_64ResolvedSemanticUnitWrapper {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn resolution(&self) -> X86_64SemanticUnitWrapperResolution {
        self.resolution
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64SemanticUnitWrapperEncodingError {
    UnsupportedTarget,
    NonCanonicalRequest,
    MalformedTemplate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64SemanticUnitWrapperResolutionError {
    RelocationMismatch,
    SectionCoordinateOverflow,
    RelativeDisplacementOutOfRange,
    MalformedResolvedBytes,
    TargetEquationMismatch,
}

pub const fn canonical_x86_64_semantic_unit_wrapper_encoding_request(
    target: NativeTarget,
) -> X86_64SemanticUnitWrapperEncodingRequest {
    X86_64SemanticUnitWrapperEncodingRequest {
        target,
        policy: X86_64SemanticUnitWrapperEncodingPolicy::MicrosoftX64CallerSavedOnlyNoControlStateMutationV1,
        shadow_byte_count: 32,
        outgoing_frame_byte_count: 72,
        outgoing_release_byte_count: 72,
        pre_call_stack_alignment: 16,
        copies: [
            X86_64SemanticUnitWrapperCopy {
                source_register: MachineRegister::X86Rcx,
                source_byte_offset: 0,
                outgoing_stack_byte_offset: 32,
            },
            X86_64SemanticUnitWrapperCopy {
                source_register: MachineRegister::X86Rcx,
                source_byte_offset: 8,
                outgoing_stack_byte_offset: 40,
            },
            X86_64SemanticUnitWrapperCopy {
                source_register: MachineRegister::X86Rdx,
                source_byte_offset: 0,
                outgoing_stack_byte_offset: 48,
            },
            X86_64SemanticUnitWrapperCopy {
                source_register: MachineRegister::X86Rdx,
                source_byte_offset: 8,
                outgoing_stack_byte_offset: 56,
            },
        ],
        argument_bindings: [
            X86_64SemanticUnitWrapperArgumentBinding {
                register: MachineRegister::X86Rcx,
                outgoing_stack_byte_offset: 32,
            },
            X86_64SemanticUnitWrapperArgumentBinding {
                register: MachineRegister::X86Rdx,
                outgoing_stack_byte_offset: 48,
            },
        ],
        relocation_field_byte_width: X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_WIDTH,
        relocation_addend: 0,
    }
}

pub fn encode_x86_64_semantic_unit_wrapper_template(
    request: X86_64SemanticUnitWrapperEncodingRequest,
) -> Result<ValidatedX86_64SemanticUnitWrapperTemplate, X86_64SemanticUnitWrapperEncodingError> {
    validate_request(request)?;
    let mut bytes = Vec::with_capacity(X86_64_SEMANTIC_UNIT_WRAPPER_FUNCTION_BYTE_COUNT);
    bytes.extend([0x48, 0x83, 0xec, 0x48]);
    for copy in request.copies {
        bytes.extend([0x48, 0x8b, source_modrm(copy.source_register)?]);
        bytes.extend(copy.source_byte_offset.to_le_bytes());
        bytes.extend([0x48, 0x89, 0x84, 0x24]);
        bytes.extend(copy.outgoing_stack_byte_offset.to_le_bytes());
    }
    for binding in request.argument_bindings {
        bytes.extend(address_prefix(binding.register)?);
        bytes.extend(binding.outgoing_stack_byte_offset.to_le_bytes());
    }
    bytes.extend([0xe8, 0, 0, 0, 0]);
    bytes.extend([0x48, 0x83, 0xc4, 0x48]);
    bytes.push(0xc3);
    debug_assert_eq!(
        bytes.len(),
        X86_64_SEMANTIC_UNIT_WRAPPER_FUNCTION_BYTE_COUNT
    );
    validate_x86_64_semantic_unit_wrapper_template(request, &bytes)
}

/// Independently parse every opcode, register, displacement, and frame byte.
pub fn validate_x86_64_semantic_unit_wrapper_template(
    request: X86_64SemanticUnitWrapperEncodingRequest,
    bytes: &[u8],
) -> Result<ValidatedX86_64SemanticUnitWrapperTemplate, X86_64SemanticUnitWrapperEncodingError> {
    validate_request(request)?;
    if bytes.len() != X86_64_SEMANTIC_UNIT_WRAPPER_FUNCTION_BYTE_COUNT {
        return Err(X86_64SemanticUnitWrapperEncodingError::MalformedTemplate);
    }
    let mut cursor = Cursor { bytes, offset: 0 };
    cursor.expect(&[0x48, 0x83, 0xec])?;
    let reserved = u32::from(cursor.byte()?);
    let mut copies = Vec::with_capacity(4);
    for _ in 0..4 {
        cursor.expect(&[0x48, 0x8b])?;
        let source_register = source_register(cursor.byte()?)?;
        let source_byte_offset = cursor.u32()?;
        cursor.expect(&[0x48, 0x89, 0x84, 0x24])?;
        copies.push(X86_64SemanticUnitWrapperCopy {
            source_register,
            source_byte_offset,
            outgoing_stack_byte_offset: cursor.u32()?,
        });
    }
    let mut bindings = Vec::with_capacity(2);
    for register in [MachineRegister::X86Rcx, MachineRegister::X86Rdx] {
        cursor.expect(&address_prefix(register)?)?;
        bindings.push(X86_64SemanticUnitWrapperArgumentBinding {
            register,
            outgoing_stack_byte_offset: cursor.u32()?,
        });
    }
    if cursor.offset != usize::from(X86_64_SEMANTIC_UNIT_WRAPPER_CALL_OPCODE_OFFSET) {
        return Err(X86_64SemanticUnitWrapperEncodingError::MalformedTemplate);
    }
    cursor.expect(&[0xe8])?;
    if cursor.offset != usize::from(X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_OFFSET)
        || cursor.u32()? != 0
        || cursor.offset != usize::from(X86_64_SEMANTIC_UNIT_WRAPPER_NEXT_INSTRUCTION_OFFSET)
    {
        return Err(X86_64SemanticUnitWrapperEncodingError::MalformedTemplate);
    }
    cursor.expect(&[0x48, 0x83, 0xc4])?;
    let released = u32::from(cursor.byte()?);
    if cursor.offset != usize::from(X86_64_SEMANTIC_UNIT_WRAPPER_RETURN_OFFSET) {
        return Err(X86_64SemanticUnitWrapperEncodingError::MalformedTemplate);
    }
    cursor.expect(&[0xc3])?;
    let decoded_copies: [X86_64SemanticUnitWrapperCopy; 4] = copies
        .try_into()
        .map_err(|_| X86_64SemanticUnitWrapperEncodingError::MalformedTemplate)?;
    let decoded_bindings: [X86_64SemanticUnitWrapperArgumentBinding; 2] = bindings
        .try_into()
        .map_err(|_| X86_64SemanticUnitWrapperEncodingError::MalformedTemplate)?;
    if cursor.offset != bytes.len()
        || reserved != request.outgoing_frame_byte_count
        || released != request.outgoing_release_byte_count
        || decoded_copies != request.copies
        || decoded_bindings != request.argument_bindings
    {
        return Err(X86_64SemanticUnitWrapperEncodingError::MalformedTemplate);
    }
    Ok(ValidatedX86_64SemanticUnitWrapperTemplate {
        request,
        bytes: bytes.to_vec(),
        footprint: expected_footprint(request),
        relocation: expected_relocation(),
    })
}

pub fn resolve_x86_64_semantic_unit_wrapper_private_continuation(
    template: &ValidatedX86_64SemanticUnitWrapperTemplate,
    relocation: X86_64SemanticUnitWrapperRelocation,
    wrapper_section_offset: u64,
    continuation_section_offset: u64,
) -> Result<ValidatedX86_64ResolvedSemanticUnitWrapper, X86_64SemanticUnitWrapperResolutionError> {
    validate_relocation(template, relocation)?;
    let next_instruction_section_offset = wrapper_section_offset
        .checked_add(u64::from(relocation.next_instruction_function_byte_offset))
        .ok_or(X86_64SemanticUnitWrapperResolutionError::SectionCoordinateOverflow)?;
    let displacement = checked_displacement(
        next_instruction_section_offset,
        continuation_section_offset,
        relocation.addend,
    )?;
    let mut bytes = template.bytes.clone();
    let field_start = usize::from(relocation.field_function_byte_offset);
    let field_end = field_start
        .checked_add(usize::from(relocation.field_byte_width))
        .ok_or(X86_64SemanticUnitWrapperResolutionError::MalformedResolvedBytes)?;
    bytes
        .get_mut(field_start..field_end)
        .ok_or(X86_64SemanticUnitWrapperResolutionError::MalformedResolvedBytes)?
        .copy_from_slice(&displacement.to_le_bytes());
    validate_x86_64_resolved_semantic_unit_wrapper(
        template,
        relocation,
        wrapper_section_offset,
        continuation_section_offset,
        &bytes,
    )
}

pub fn validate_x86_64_resolved_semantic_unit_wrapper(
    template: &ValidatedX86_64SemanticUnitWrapperTemplate,
    relocation: X86_64SemanticUnitWrapperRelocation,
    wrapper_section_offset: u64,
    continuation_section_offset: u64,
    bytes: &[u8],
) -> Result<ValidatedX86_64ResolvedSemanticUnitWrapper, X86_64SemanticUnitWrapperResolutionError> {
    validate_relocation(template, relocation)?;
    let next_instruction_section_offset = wrapper_section_offset
        .checked_add(u64::from(relocation.next_instruction_function_byte_offset))
        .ok_or(X86_64SemanticUnitWrapperResolutionError::SectionCoordinateOverflow)?;
    let expected = checked_displacement(
        next_instruction_section_offset,
        continuation_section_offset,
        relocation.addend,
    )?;
    let field_start = usize::from(relocation.field_function_byte_offset);
    let field_end = field_start
        .checked_add(usize::from(relocation.field_byte_width))
        .ok_or(X86_64SemanticUnitWrapperResolutionError::MalformedResolvedBytes)?;
    if bytes.len() != template.bytes.len()
        || bytes.get(usize::from(relocation.opcode_function_byte_offset)) != Some(&0xe8)
        || bytes.get(..field_start) != template.bytes.get(..field_start)
        || bytes.get(field_end..) != template.bytes.get(field_end..)
    {
        return Err(X86_64SemanticUnitWrapperResolutionError::MalformedResolvedBytes);
    }
    let displacement = bytes
        .get(field_start..field_end)
        .and_then(|field| field.try_into().ok())
        .map(i32::from_le_bytes)
        .ok_or(X86_64SemanticUnitWrapperResolutionError::MalformedResolvedBytes)?;
    if displacement != expected
        || i128::from(next_instruction_section_offset) + i128::from(displacement)
            - i128::from(relocation.addend)
            != i128::from(continuation_section_offset)
    {
        return Err(X86_64SemanticUnitWrapperResolutionError::TargetEquationMismatch);
    }
    Ok(ValidatedX86_64ResolvedSemanticUnitWrapper {
        bytes: bytes.to_vec(),
        resolution: X86_64SemanticUnitWrapperResolution {
            source: relocation,
            state: X86_64SemanticUnitWrapperResolutionState::ResolvedInSectionV1,
            wrapper_section_offset,
            continuation_section_offset,
            next_instruction_section_offset,
            displacement,
        },
    })
}

fn validate_request(
    request: X86_64SemanticUnitWrapperEncodingRequest,
) -> Result<(), X86_64SemanticUnitWrapperEncodingError> {
    if request.target != NativeTarget::uefi_x64() {
        return Err(X86_64SemanticUnitWrapperEncodingError::UnsupportedTarget);
    }
    if request != canonical_x86_64_semantic_unit_wrapper_encoding_request(request.target) {
        return Err(X86_64SemanticUnitWrapperEncodingError::NonCanonicalRequest);
    }
    Ok(())
}

fn expected_footprint(
    request: X86_64SemanticUnitWrapperEncodingRequest,
) -> X86_64SemanticUnitWrapperFootprint {
    X86_64SemanticUnitWrapperFootprint {
        root_reads: request.copies,
        caller_copy_writes: request.copies,
        scratch_register_writes: RegisterSet::new([MachineRegister::X86Rax]),
        argument_pointer_writes: request.argument_bindings,
        call_clobbers: RegisterSet::new([
            MachineRegister::X86Rax,
            MachineRegister::X86Rcx,
            MachineRegister::X86Rdx,
            MachineRegister::X86R8,
            MachineRegister::X86R9,
            MachineRegister::X86R10,
            MachineRegister::X86R11,
            MachineRegister::X86Xmm(0),
            MachineRegister::X86Xmm(1),
            MachineRegister::X86Xmm(2),
            MachineRegister::X86Xmm(3),
            MachineRegister::X86Xmm(4),
            MachineRegister::X86Xmm(5),
        ]),
        writes_stack_pointer: true,
        writes_instruction_pointer: true,
        writes_flags: true,
        mutates_control_state: false,
        frame_byte_count: request.outgoing_frame_byte_count,
        shadow_byte_count: request.shadow_byte_count,
        pre_call_stack_alignment: request.pre_call_stack_alignment,
        frame_is_balanced: request.outgoing_frame_byte_count == request.outgoing_release_byte_count,
        trap: X86_64SemanticUnitWrapperTrapBehavior::MayArchitecturalFaultV1,
        call: X86_64SemanticUnitWrapperCallEffect::DirectPrivateContinuationUnitV1,
        cleanup: X86_64SemanticUnitWrapperCleanupEffect::NoneV1,
        returns_unit: true,
    }
}

const fn expected_relocation() -> X86_64SemanticUnitWrapperRelocation {
    X86_64SemanticUnitWrapperRelocation {
        kind: X86_64SemanticUnitWrapperRelocationKind::Relative32PrivateContinuationFromNextInstructionV1,
        state: X86_64SemanticUnitWrapperRelocationState::UnresolvedZeroFieldV1,
        opcode_function_byte_offset: X86_64_SEMANTIC_UNIT_WRAPPER_CALL_OPCODE_OFFSET,
        field_function_byte_offset: X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_OFFSET,
        next_instruction_function_byte_offset: X86_64_SEMANTIC_UNIT_WRAPPER_NEXT_INSTRUCTION_OFFSET,
        field_byte_width: X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_WIDTH,
        addend: 0,
    }
}

fn validate_relocation(
    template: &ValidatedX86_64SemanticUnitWrapperTemplate,
    relocation: X86_64SemanticUnitWrapperRelocation,
) -> Result<(), X86_64SemanticUnitWrapperResolutionError> {
    if relocation != template.relocation
        || relocation != expected_relocation()
        || template
            .bytes
            .get(usize::from(relocation.opcode_function_byte_offset))
            != Some(&0xe8)
        || template.bytes.get(
            usize::from(relocation.field_function_byte_offset)
                ..usize::from(relocation.next_instruction_function_byte_offset),
        ) != Some(&[0, 0, 0, 0])
    {
        return Err(X86_64SemanticUnitWrapperResolutionError::RelocationMismatch);
    }
    Ok(())
}

fn checked_displacement(
    next_instruction_section_offset: u64,
    continuation_section_offset: u64,
    addend: i64,
) -> Result<i32, X86_64SemanticUnitWrapperResolutionError> {
    i32::try_from(
        i128::from(continuation_section_offset) - i128::from(next_instruction_section_offset)
            + i128::from(addend),
    )
    .map_err(|_| X86_64SemanticUnitWrapperResolutionError::RelativeDisplacementOutOfRange)
}

fn source_modrm(register: MachineRegister) -> Result<u8, X86_64SemanticUnitWrapperEncodingError> {
    match register {
        MachineRegister::X86Rcx => Ok(0x81),
        MachineRegister::X86Rdx => Ok(0x82),
        _ => Err(X86_64SemanticUnitWrapperEncodingError::NonCanonicalRequest),
    }
}

fn source_register(modrm: u8) -> Result<MachineRegister, X86_64SemanticUnitWrapperEncodingError> {
    match modrm {
        0x81 => Ok(MachineRegister::X86Rcx),
        0x82 => Ok(MachineRegister::X86Rdx),
        _ => Err(X86_64SemanticUnitWrapperEncodingError::MalformedTemplate),
    }
}

fn address_prefix(
    register: MachineRegister,
) -> Result<[u8; 4], X86_64SemanticUnitWrapperEncodingError> {
    match register {
        MachineRegister::X86Rcx => Ok([0x48, 0x8d, 0x8c, 0x24]),
        MachineRegister::X86Rdx => Ok([0x48, 0x8d, 0x94, 0x24]),
        _ => Err(X86_64SemanticUnitWrapperEncodingError::NonCanonicalRequest),
    }
}

struct Cursor<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl Cursor<'_> {
    fn expect(&mut self, expected: &[u8]) -> Result<(), X86_64SemanticUnitWrapperEncodingError> {
        let end = self
            .offset
            .checked_add(expected.len())
            .ok_or(X86_64SemanticUnitWrapperEncodingError::MalformedTemplate)?;
        if self.bytes.get(self.offset..end) != Some(expected) {
            return Err(X86_64SemanticUnitWrapperEncodingError::MalformedTemplate);
        }
        self.offset = end;
        Ok(())
    }

    fn byte(&mut self) -> Result<u8, X86_64SemanticUnitWrapperEncodingError> {
        let byte = *self
            .bytes
            .get(self.offset)
            .ok_or(X86_64SemanticUnitWrapperEncodingError::MalformedTemplate)?;
        self.offset = self
            .offset
            .checked_add(1)
            .ok_or(X86_64SemanticUnitWrapperEncodingError::MalformedTemplate)?;
        Ok(byte)
    }

    fn u32(&mut self) -> Result<u32, X86_64SemanticUnitWrapperEncodingError> {
        let end = self
            .offset
            .checked_add(4)
            .ok_or(X86_64SemanticUnitWrapperEncodingError::MalformedTemplate)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(X86_64SemanticUnitWrapperEncodingError::MalformedTemplate)?;
        self.offset = end;
        Ok(u32::from_le_bytes(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> X86_64SemanticUnitWrapperEncodingRequest {
        canonical_x86_64_semantic_unit_wrapper_encoding_request(NativeTarget::uefi_x64())
    }

    #[test]
    fn compact_wrapper_template_is_independently_decoded() {
        let template = encode_x86_64_semantic_unit_wrapper_template(request()).unwrap();
        assert_eq!(template.bytes().len(), 90);
        assert_eq!(template.bytes()[80], 0xe8);
        assert_eq!(&template.bytes()[81..85], &[0, 0, 0, 0]);
        assert_eq!(template.bytes()[89], 0xc3);
        assert_eq!(template.relocation(), expected_relocation());
        assert!(template.footprint().frame_is_balanced);
        assert!(template.footprint().writes_stack_pointer);
        assert!(template.footprint().writes_instruction_pointer);
        assert!(template.footprint().writes_flags);
        assert!(!template.footprint().mutates_control_state);
        assert_eq!(template.footprint(), &expected_footprint(request()));
        assert_eq!(
            validate_x86_64_semantic_unit_wrapper_template(request(), template.bytes()),
            Ok(template)
        );
    }

    #[test]
    fn every_template_byte_is_replayed() {
        let template = encode_x86_64_semantic_unit_wrapper_template(request()).unwrap();
        for index in 0..template.bytes().len() {
            let mut bytes = template.bytes().to_vec();
            bytes[index] ^= 0x5a;
            assert_eq!(
                validate_x86_64_semantic_unit_wrapper_template(request(), &bytes),
                Err(X86_64SemanticUnitWrapperEncodingError::MalformedTemplate),
                "byte {index} escaped replay"
            );
        }
    }

    #[test]
    fn request_drift_and_unsupported_targets_fail_closed() {
        let mut drift = request();
        drift.outgoing_frame_byte_count = 88;
        assert_eq!(
            encode_x86_64_semantic_unit_wrapper_template(drift),
            Err(X86_64SemanticUnitWrapperEncodingError::NonCanonicalRequest)
        );

        let unsupported =
            canonical_x86_64_semantic_unit_wrapper_encoding_request(NativeTarget::linux_x64());
        assert_eq!(
            encode_x86_64_semantic_unit_wrapper_template(unsupported),
            Err(X86_64SemanticUnitWrapperEncodingError::UnsupportedTarget)
        );
    }

    #[test]
    fn forward_and_backward_private_continuations_resolve_exactly() {
        let template = encode_x86_64_semantic_unit_wrapper_template(request()).unwrap();
        let forward = resolve_x86_64_semantic_unit_wrapper_private_continuation(
            &template,
            template.relocation(),
            0,
            90,
        )
        .unwrap();
        assert_eq!(&forward.bytes()[81..85], &5_i32.to_le_bytes());
        assert_eq!(forward.resolution().displacement, 5);

        let backward = resolve_x86_64_semantic_unit_wrapper_private_continuation(
            &template,
            template.relocation(),
            100,
            8,
        )
        .unwrap();
        assert_eq!(backward.resolution().displacement, -177);
        validate_x86_64_resolved_semantic_unit_wrapper(
            &template,
            template.relocation(),
            100,
            8,
            backward.bytes(),
        )
        .unwrap();
    }

    #[test]
    fn resolution_rejects_drift_corruption_and_overflow() {
        let template = encode_x86_64_semantic_unit_wrapper_template(request()).unwrap();
        let mut relocation = template.relocation();
        relocation.field_function_byte_offset = 82;
        assert_eq!(
            resolve_x86_64_semantic_unit_wrapper_private_continuation(&template, relocation, 0, 90,),
            Err(X86_64SemanticUnitWrapperResolutionError::RelocationMismatch)
        );
        assert_eq!(
            resolve_x86_64_semantic_unit_wrapper_private_continuation(
                &template,
                template.relocation(),
                0,
                u64::MAX,
            ),
            Err(X86_64SemanticUnitWrapperResolutionError::RelativeDisplacementOutOfRange)
        );

        let resolved = resolve_x86_64_semantic_unit_wrapper_private_continuation(
            &template,
            template.relocation(),
            0,
            90,
        )
        .unwrap();
        for index in 0..resolved.bytes().len() {
            let mut bytes = resolved.bytes().to_vec();
            bytes[index] ^= 0x5a;
            assert!(
                validate_x86_64_resolved_semantic_unit_wrapper(
                    &template,
                    template.relocation(),
                    0,
                    90,
                    &bytes,
                )
                .is_err(),
                "resolved byte {index} escaped replay"
            );
        }
    }
}
