use omega_machine_code::{
    FunctionFragmentConditionalBranchEvidence, FunctionFragmentConditionalBranchPredicate,
    FunctionFragmentInstructionSpan,
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
                predicate: match branch.predicate {
                    crate::ResolvedConditionalBranchPredicate::NonZeroV1 => {
                        FunctionFragmentConditionalBranchPredicate::NonZeroV1
                    }
                    crate::ResolvedConditionalBranchPredicate::U64LessThanV1 => {
                        FunctionFragmentConditionalBranchPredicate::U64LessThanV1
                    }
                    crate::ResolvedConditionalBranchPredicate::I64LessThanV1 => {
                        FunctionFragmentConditionalBranchPredicate::I64LessThanV1
                    }
                },
                source_block: branch.source_block,
                when_taken_edge: branch.when_taken_edge,
                when_taken_block: branch.when_taken_block,
                when_taken_offset: branch.when_taken_offset,
                when_fallthrough_edge: branch.when_fallthrough_edge,
                when_fallthrough_block: branch.when_fallthrough_block,
                when_fallthrough_offset: branch.when_fallthrough_offset,
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
            | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction,
        }))
        .find(|instruction| instruction.id == row.instruction)
        .ok_or(FunctionFragmentEmissionError::MissingInstruction(
            row.instruction,
        ))
}
