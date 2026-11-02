//! Report counts for one admitted function, carried only with its unchanged rows.

use selected_instructions::FunctionLiveRanges;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FunctionCounts {
    pub(super) virtual_register_count: usize,
    pub(super) virtual_fragment_count: usize,
}

impl FunctionCounts {
    pub(super) fn new(function: &FunctionLiveRanges) -> Self {
        Self {
            virtual_register_count: function.virtual_registers.len(),
            virtual_fragment_count: function
                .virtual_registers
                .iter()
                .map(|row| row.fragments.len())
                .sum(),
        }
    }
}
