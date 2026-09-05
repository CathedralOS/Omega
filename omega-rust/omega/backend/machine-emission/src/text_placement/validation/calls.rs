use super::{TextPlacementError, add, bytes};
use machine_code::{
    FunctionFragmentInternalMachineFixup, FunctionFragmentInternalMachineFixupKind,
    FunctionFragmentInternalMachineFixupState, InternalMachineCallResolutionKind,
    InternalMachineCallResolutionState, PlacedInternalMachineCallResolution,
};
use selected_instructions::{SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::{MachineId, OperationId};
use std::collections::BTreeMap;
use target::Architecture;

pub(super) struct Call<'a> {
    pub caller: MachineId,
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub operation: OperationId,
    pub callee: MachineId,
    pub offset: u64,
    pub bytes: &'a [u8],
    pub fixup: FunctionFragmentInternalMachineFixup,
}

pub(super) fn check(
    call: Call<'_>,
    architecture: Architecture,
    section_offset: u64,
    offsets: &BTreeMap<MachineId, u64>,
    candidate_bytes: &[u8],
    row: &PlacedInternalMachineCallResolution,
) -> Result<(u64, u64), TextPlacementError> {
    let fixup = call.fixup;
    let callee_offset =
        *offsets
            .get(&call.callee)
            .ok_or(TextPlacementError::MissingInternalMachineTarget(
                call.callee,
            ))?;
    if fixup.state != FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1
        || fixup.addend != 0
        || fixup.callee != call.callee
        || fixup.patch_byte_width != 4
    {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    let opcode_relative = fixup
        .opcode_function_offset
        .checked_sub(call.offset)
        .ok_or(TextPlacementError::SourceShapeMismatch)?;
    let patch_relative = fixup
        .patch_function_offset
        .checked_sub(call.offset)
        .ok_or(TextPlacementError::SourceShapeMismatch)?;
    let field = bytes(candidate_bytes, fixup.patch_function_offset, 4)?;
    let field: [u8; 4] = field
        .try_into()
        .map_err(|_| TextPlacementError::ArtifactMismatch)?;
    let (kind, displacement) = match (architecture, fixup.kind) {
        (Architecture::X86_64, FunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1) => {
            if fixup.patch_function_offset != add(fixup.opcode_function_offset, 1)? || fixup.reference_function_offset != add(fixup.opcode_function_offset, 5)? || bytes(call.bytes, opcode_relative, 1)? != [0xe8] || bytes(call.bytes, patch_relative, 4)? != [0, 0, 0, 0] { return Err(TextPlacementError::SourceShapeMismatch); }
            if bytes(candidate_bytes, fixup.opcode_function_offset, 1)? != [0xe8] { return Err(TextPlacementError::ArtifactMismatch); }
            (InternalMachineCallResolutionKind::X86Relative32FromNextInstructionToInternalMachineV1, i32::from_le_bytes(field))
        }
        (Architecture::Aarch64, FunctionFragmentInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1) => {
            if fixup.opcode_function_offset != call.offset || fixup.patch_function_offset != call.offset || fixup.reference_function_offset != call.offset || call.bytes != 0x9400_0000_u32.to_le_bytes() { return Err(TextPlacementError::SourceShapeMismatch); }
            let instruction = u32::from_le_bytes(field);
            if instruction & 0xfc00_0000 != 0x9400_0000 { return Err(TextPlacementError::ArtifactMismatch); }
            let signed_words = ((instruction & 0x03ff_ffff) << 6) as i32 >> 6;
            (InternalMachineCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1, signed_words * 4)
        }
        _ => return Err(TextPlacementError::SourceShapeMismatch),
    };
    let reference = add(section_offset, fixup.reference_function_offset)?;
    if i128::from(reference) + i128::from(displacement) != i128::from(callee_offset)
        || row.kind != kind
        || row.state != InternalMachineCallResolutionState::ResolvedInSectionV1
        || row.caller != call.caller
        || row.block != call.block
        || row.instruction != call.instruction
        || row.operation != call.operation
        || row.callee != call.callee
        || row.call_function_offset != call.offset
        || row.call_section_offset != add(section_offset, call.offset)?
        || row.call_byte_count != call.bytes.len() as u64
        || row.opcode_function_offset != fixup.opcode_function_offset
        || row.opcode_section_offset != add(section_offset, fixup.opcode_function_offset)?
        || row.field_function_offset != fixup.patch_function_offset
        || row.field_section_offset != add(section_offset, fixup.patch_function_offset)?
        || row.next_instruction_function_offset != fixup.reference_function_offset
        || row.next_instruction_section_offset != reference
        || row.callee_section_offset != callee_offset
        || row.field_byte_width != 4
        || row.addend != 0
        || row.displacement != displacement
    {
        return Err(TextPlacementError::ArtifactMismatch);
    }
    Ok((
        fixup.patch_function_offset,
        add(fixup.patch_function_offset, 4)?,
    ))
}
