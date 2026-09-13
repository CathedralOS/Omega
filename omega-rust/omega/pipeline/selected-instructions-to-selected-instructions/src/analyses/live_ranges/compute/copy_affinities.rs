//! Copy affinities observe existing register-to-register copies; they record a
//! same-home preference for placement and never impose a constraint.

use crate::{CopyAffinity, LiveRangeError};
use selected_instructions::{SelectedFunction, SelectedInstructionKind};

pub(super) fn derive(
    function_index: usize,
    selected: &SelectedFunction,
) -> Result<Vec<CopyAffinity>, LiveRangeError> {
    let mut rows = Vec::new();
    for block in &selected.blocks {
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
    Ok(rows)
}
