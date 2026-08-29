use omega_selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedFunction, SelectedTerminator,
};

use crate::StagedOptimizedAarch64CbnzFusion;

use super::super::OptimizedResolvedSelectedFormLayoutError;

pub(super) fn derive<'a>(
    function: &'a SelectedFunction,
    fusion: Option<&StagedOptimizedAarch64CbnzFusion>,
) -> Result<Vec<&'a SelectedBlock>, OptimizedResolvedSelectedFormLayoutError> {
    if let [block] = function.blocks.as_slice() {
        if function.entry_block != block.id
            || !matches!(block.terminator, SelectedTerminator::Return { .. })
            || fusion.is_some()
        {
            return unsupported(function);
        }
        return Ok(vec![block]);
    }
    if function.blocks.len() != 3 {
        return unsupported(function);
    }
    let entry = find(function, function.entry_block)?;
    let SelectedTerminator::ConditionalBranch {
        when_nonzero,
        when_zero,
        ..
    } = &entry.terminator
    else {
        return unsupported(function);
    };
    if when_nonzero.block == when_zero.block
        || entry.id == when_nonzero.block
        || entry.id == when_zero.block
    {
        return unsupported(function);
    }
    let zero = find(function, when_zero.block)?;
    let nonzero = find(function, when_nonzero.block)?;
    if !matches!(zero.terminator, SelectedTerminator::Return { .. })
        || !matches!(nonzero.terminator, SelectedTerminator::Return { .. })
    {
        return unsupported(function);
    }
    Ok(vec![entry, zero, nonzero])
}

fn find(
    function: &SelectedFunction,
    id: SelectedBlockId,
) -> Result<&SelectedBlock, OptimizedResolvedSelectedFormLayoutError> {
    function
        .blocks
        .iter()
        .find(|block| block.id == id)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine))
}

fn unsupported<T>(
    function: &SelectedFunction,
) -> Result<T, OptimizedResolvedSelectedFormLayoutError> {
    Err(OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine))
}
