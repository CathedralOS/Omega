//! Structural fragments retain the call, unresolved fixup, and terminal return.

use super::{ResolvedFragmentEmissionError, byte_span, require};
use ::machine_code::*;
use selected_instructions::SelectedInstructionPlan;

pub(super) fn check(
    selected: &SelectedInstructionPlan,
    layout: &ResolvedMachineLayout,
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<(), ResolvedFragmentEmissionError> {
    require(
        selected.structural_unit_functions.len() == layout.structural_unit_functions.len()
            && selected.structural_unit_functions.len()
                == fragments.structural_unit_functions.len(),
    )?;
    for ((source, resolved), claimed) in selected
        .structural_unit_functions
        .iter()
        .zip(&layout.structural_unit_functions)
        .zip(&fragments.structural_unit_functions)
    {
        let block = &claimed.block;
        require(
            source.machine == resolved.machine
                && claimed.machine == source.machine
                && claimed.attachment == source.attachment
                && claimed.provenance == source.provenance
                && claimed.byte_count == resolved.byte_count
                && u64::try_from(claimed.bytes.len()).ok() == Some(claimed.byte_count)
                && source.entry_block == resolved.block
                && block.block == resolved.block
                && resolved.offset == 0
                && block.offset == 0
                && block.byte_count == resolved.byte_count,
        )?;
        let mut offset = 0_u64;
        match (&source.call, &resolved.call, &block.call) {
            (None, None, None) => {}
            (Some(call), Some(layout_call), Some(actual)) => {
                require(
                    actual.instruction == call.id
                        && actual.instruction == layout_call.instruction
                        && actual.operation == call.operation
                        && actual.operation == layout_call.operation
                        && actual.callee == call.callee
                        && actual.callee == layout_call.callee
                        && actual.offset == layout_call.offset
                        && actual.offset == offset
                        && actual.bytes == layout_call.bytes
                        && actual.provenance == call.provenance,
                )?;
                let fixup = &actual.fixup;
                let original = &layout_call.fixup;
                require(matches!(original.kind, X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1)
                    && fixup.kind == FunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1
                    && original.state == X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1
                    && fixup.state == FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1
                    && fixup.callee == original.callee
                    && Some(fixup.opcode_function_offset) == actual.offset.checked_add(u64::from(original.opcode_byte_offset))
                    && Some(fixup.patch_function_offset) == actual.offset.checked_add(u64::from(original.field_byte_offset))
                    && Some(fixup.reference_function_offset) == actual.offset.checked_add(u64::from(original.next_instruction_byte_offset))
                    && fixup.patch_byte_width == original.field_byte_width && fixup.addend == original.addend)?;
                byte_span(&claimed.bytes, offset, &actual.bytes)?;
                offset = u64::try_from(actual.bytes.len())
                    .map_err(|_| ResolvedFragmentEmissionError::OffsetOverflow)?;
            }
            _ => return Err(ResolvedFragmentEmissionError::ArtifactMismatch),
        }
        let actual = &block.return_instruction;
        let row = &resolved.return_instruction;
        require(
            actual.instruction == source.terminator.instruction.id
                && actual.instruction == row.instruction
                && actual.alternative == row.alternative
                && actual.offset == row.offset
                && row.offset == offset
                && actual.bytes == row.bytes
                && actual.branch.is_none()
                && actual.internal_machine_fixup.is_none()
                && actual.provenance == source.terminator.instruction.provenance
                && matches!(actual.control, FunctionFragmentControlProvenance::Return { psi_return_edge }
                if psi_return_edge == source.terminator.psi_return_edge),
        )?;
        byte_span(&claimed.bytes, offset, &actual.bytes)?;
        require(
            offset.checked_add(
                u64::try_from(actual.bytes.len())
                    .map_err(|_| ResolvedFragmentEmissionError::OffsetOverflow)?,
            ) == Some(claimed.byte_count),
        )?;
    }
    Ok(())
}
