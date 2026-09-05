use machine_code::{
    FunctionFragmentInternalMachineFixupKind, FunctionFragmentInternalMachineFixupState,
    StructuralUnitCallFragmentSpan,
};
use machine_code::{
    X86_64StructuralUnitInternalControlFixup, X86_64StructuralUnitInternalControlFixupKind,
    X86_64StructuralUnitInternalControlFixupState,
};

use super::super::TextPlacementError;

pub(super) fn matches_target(
    call: &StructuralUnitCallFragmentSpan,
    target: X86_64StructuralUnitInternalControlFixup,
) -> Result<bool, TextPlacementError> {
    let neutral = call.fixup;
    Ok(neutral.kind
        == FunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1
        && neutral.state
            == FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1
        && target.kind
            == X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1
        && target.state == X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1
        && neutral.callee == target.callee
        && neutral.callee == call.callee
        && neutral.opcode_function_offset
            == call
                .offset
                .checked_add(u64::from(target.opcode_byte_offset))
                .ok_or(TextPlacementError::OffsetOverflow)?
        && neutral.patch_function_offset
            == call
                .offset
                .checked_add(u64::from(target.field_byte_offset))
                .ok_or(TextPlacementError::OffsetOverflow)?
        && neutral.reference_function_offset
            == call
                .offset
                .checked_add(u64::from(target.next_instruction_byte_offset))
                .ok_or(TextPlacementError::OffsetOverflow)?
        && neutral.patch_byte_width == target.field_byte_width
        && neutral.addend == target.addend)
}
