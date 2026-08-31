use omega_machine_code::{
    FunctionFragmentConditionalBranchEvidence, FunctionFragmentInstructionSpan,
};
use omega_selected_instructions::{SelectedBlock, SelectedInstruction, SelectedTerminator};

use crate::ResolvedSelectedFormRow;

use super::super::super::FunctionFragmentEmissionError;
use super::control;

pub(super) fn emit(
    block: &SelectedBlock,
    row: &ResolvedSelectedFormRow,
    bytes: &mut Vec<u8>,
) -> Result<FunctionFragmentInstructionSpan, FunctionFragmentEmissionError> {
    let row_offset =
        u64::try_from(bytes.len()).map_err(|_| FunctionFragmentEmissionError::OffsetOverflow)?;
    if row_offset != row.offset {
        return Err(FunctionFragmentEmissionError::RootMismatch);
    }
    let instruction = selected(block, row)?;
    let control = control::provenance(block, instruction.id);
    bytes.extend_from_slice(&row.bytes);
    Ok(FunctionFragmentInstructionSpan {
        instruction: row.instruction,
        alternative: row.alternative,
        offset: row.offset,
        bytes: row.bytes.clone(),
        branch: row.branch.as_deref().map(|branch| {
            Box::new(FunctionFragmentConditionalBranchEvidence {
                source_block: branch.source_block,
                when_nonzero_edge: branch.when_nonzero_edge,
                when_nonzero_block: branch.when_nonzero_block,
                when_nonzero_offset: branch.when_nonzero_offset,
                when_zero_edge: branch.when_zero_edge,
                when_zero_block: branch.when_zero_block,
                when_zero_offset: branch.when_zero_offset,
                byte_displacement: branch.byte_displacement,
                decoded_register_reads: branch.decoded_register_reads.clone(),
                decoded_effects: branch.decoded_effects.clone(),
            })
        }),
        provenance: instruction.provenance.clone(),
        control,
    })
}

fn selected<'a>(
    block: &'a SelectedBlock,
    row: &ResolvedSelectedFormRow,
) -> Result<&'a SelectedInstruction, FunctionFragmentEmissionError> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction,
        }))
        .find(|instruction| instruction.id == row.instruction)
        .ok_or(FunctionFragmentEmissionError::MissingInstruction(
            row.instruction,
        ))
}
