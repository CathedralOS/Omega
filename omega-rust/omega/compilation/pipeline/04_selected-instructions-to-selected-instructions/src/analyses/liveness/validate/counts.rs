//! Report counts for one admitted function, carried only with its unchanged rows.

use selected_instructions::FunctionLiveness;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FunctionCounts {
    pub(super) block_count: usize,
    pub(super) instruction_count: usize,
    pub(super) successor_count: usize,
    pub(super) tied_pair_count: usize,
    pub(super) early_clobber_count: usize,
}

impl FunctionCounts {
    pub(super) fn new(function: &FunctionLiveness) -> Self {
        Self {
            block_count: function.blocks.len(),
            instruction_count: function
                .blocks
                .iter()
                .map(|block| block.instructions.len())
                .sum(),
            successor_count: function
                .blocks
                .iter()
                .map(|block| block.successors.len())
                .sum(),
            tied_pair_count: function
                .operand_positions
                .iter()
                .filter(|operand| operand.tied_to.is_some())
                .count(),
            early_clobber_count: function
                .operand_positions
                .iter()
                .filter(|operand| operand.early_clobber)
                .count(),
        }
    }
}
