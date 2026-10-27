//! Copy affinities observe existing register-to-register copies; they record a
//! same-home preference for placement and never impose a constraint.

use crate::LiveRangeError;
use selected_instructions::CopyAffinity;
use selected_instructions::{SelectedFunction, SelectedInstructionKind};

pub(super) fn derive(
    function_index: usize,
    selected: &SelectedFunction,
) -> Result<Vec<CopyAffinity>, LiveRangeError> {
    let mut rows = Vec::new();
    for (block_index, block) in selected.blocks.iter().enumerate() {
        if let Some((block_affinities, has_malformed_copy)) =
            selected.blocks.block_copy_affinities(block_index)
        {
            if has_malformed_copy {
                return Err(LiveRangeError::FunctionMismatch {
                    function: function_index,
                });
            }
            rows.extend_from_slice(block_affinities);
            continue;
        }
        for instruction in &block.instructions {
            if instruction.kind != SelectedInstructionKind::CopyI64 {
                continue;
            }
            let [source, destination] = instruction.operands.as_slice() else {
                return Err(LiveRangeError::FunctionMismatch {
                    function: function_index,
                });
            };
            if source.virtual_register == destination.virtual_register {
                continue;
            }
            rows.push(CopyAffinity {
                block: block.id,
                instruction: instruction.id,
                source: source.virtual_register,
                destination: destination.virtual_register,
            });
        }
    }
    // Rows are canonical by (block, instruction), not by stream position:
    // recovery inserts copies (fixed-view copies, live-range splits) with
    // fresh identities after every existing instruction, so a block's
    // stream order and its identity order can differ.
    if rows.windows(2).any(|w| w[0] > w[1]) {
        rows.sort_unstable();
    }
    Ok(rows)
}
