//! Optimizer module role: executable entrance. Three-block projection replay from entry control through return routes.

mod active_resident_exact_add_chain_return;
mod entry_control;
mod exact_binary_return;
mod immediate_return;
mod instruction_projection;
mod parameter_return;
mod return_routes;

use crate::selection::shared::*;

pub(super) fn validate_selected_blocks(
    function_index: usize,
    source: &SourceFunction,
    function: &SelectedFunction,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    if function.blocks[0].source_block != source.entry_block
        || function.blocks[1].source_block != source.true_block
        || function.blocks[2].source_block != source.false_block
    {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: function
                .blocks
                .iter()
                .enumerate()
                .find(|(index, block)| {
                    block.source_block
                        != [source.entry_block, source.true_block, source.false_block][*index]
                })
                .map_or(0, |(index, _)| index as u32),
        });
    }
    entry_control::validate(function_index, source, function, keys, catalog)?;
    return_routes::validate(function_index, source, function, keys, catalog)
}
