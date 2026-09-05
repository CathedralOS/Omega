use omega_machine_code::FunctionFragment;
use omega_machine_code::{PlacedBlockSpan, PlacedInstructionSpan};
use omega_target::Architecture;

use super::super::super::TextPlacementError;
use super::super::conversion::usize_to_u64;
use super::alignment;

pub(crate) fn place(
    architecture: Architecture,
    function: &FunctionFragment,
    function_section_offset: u64,
) -> Result<Vec<PlacedBlockSpan>, TextPlacementError> {
    let mut blocks = Vec::with_capacity(function.blocks.len());
    for block in &function.blocks {
        alignment::validate(architecture, block.offset, block.byte_count)?;
        let section_offset = function_section_offset
            .checked_add(block.offset)
            .ok_or(TextPlacementError::OffsetOverflow)?;
        if block
            .offset
            .checked_add(block.byte_count)
            .is_none_or(|end| end > function.byte_count)
        {
            return Err(TextPlacementError::SourceShapeMismatch);
        }
        let mut instructions = Vec::with_capacity(block.instructions.len());
        for row in &block.instructions {
            let byte_count = usize_to_u64(row.bytes.len())?;
            alignment::validate(architecture, row.offset, byte_count)?;
            let row_section_offset = function_section_offset
                .checked_add(row.offset)
                .ok_or(TextPlacementError::OffsetOverflow)?;
            if row
                .offset
                .checked_add(byte_count)
                .is_none_or(|end| end > function.byte_count)
            {
                return Err(TextPlacementError::SourceShapeMismatch);
            }
            instructions.push(PlacedInstructionSpan {
                instruction: row.instruction,
                alternative: row.alternative,
                function_offset: row.offset,
                section_offset: row_section_offset,
                byte_count,
            });
        }
        blocks.push(PlacedBlockSpan {
            block: block.block,
            function_offset: block.offset,
            section_offset,
            byte_count: block.byte_count,
            instructions,
        });
    }
    Ok(blocks)
}
