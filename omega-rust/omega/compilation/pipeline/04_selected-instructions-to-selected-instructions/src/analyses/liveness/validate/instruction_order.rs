//! One block's instructions in program order, terminator last.

use selected_instructions::{SelectedBlock, SelectedInstruction};

pub(super) fn ordered_instructions(block: &SelectedBlock) -> Vec<&SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(
            selected_instructions::terminator_instruction(&block.terminator),
        ))
        .collect()
}
