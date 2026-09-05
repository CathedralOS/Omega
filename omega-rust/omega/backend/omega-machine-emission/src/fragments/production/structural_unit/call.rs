use omega_machine_code::{
    FunctionFragmentInternalMachineFixup, FunctionFragmentInternalMachineFixupKind,
    FunctionFragmentInternalMachineFixupState, StructuralUnitCallFragmentSpan,
};
use omega_machine_code::{
    X86_64StructuralUnitInternalControlFixupKind, X86_64StructuralUnitInternalControlFixupState,
};
use omega_selected_instructions::SelectedStructuralUnitCallInstruction;

use omega_machine_code::ResolvedStructuralUnitCallLayout;

use crate::fragments::ResolvedFragmentEmissionError;

pub(super) fn emit(
    selected: &SelectedStructuralUnitCallInstruction,
    resolved: &ResolvedStructuralUnitCallLayout,
) -> Result<StructuralUnitCallFragmentSpan, ResolvedFragmentEmissionError> {
    if selected.id != resolved.instruction
        || selected.operation != resolved.operation
        || selected.callee != resolved.callee
    {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    let fixup = resolved.fixup;
    let kind = match fixup.kind {
        X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1 => FunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1,
    };
    let state = match fixup.state {
        X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1 => {
            FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1
        }
    };
    let base = resolved.offset;
    Ok(StructuralUnitCallFragmentSpan {
        instruction: resolved.instruction,
        operation: resolved.operation,
        callee: resolved.callee,
        offset: base,
        bytes: resolved.bytes.clone(),
        provenance: selected.provenance.clone(),
        fixup: FunctionFragmentInternalMachineFixup {
            kind,
            state,
            callee: fixup.callee,
            opcode_function_offset: base
                .checked_add(u64::from(fixup.opcode_byte_offset))
                .ok_or(ResolvedFragmentEmissionError::OffsetOverflow)?,
            patch_function_offset: base
                .checked_add(u64::from(fixup.field_byte_offset))
                .ok_or(ResolvedFragmentEmissionError::OffsetOverflow)?,
            reference_function_offset: base
                .checked_add(u64::from(fixup.next_instruction_byte_offset))
                .ok_or(ResolvedFragmentEmissionError::OffsetOverflow)?,
            patch_byte_width: fixup.field_byte_width,
            addend: fixup.addend,
        },
    })
}
