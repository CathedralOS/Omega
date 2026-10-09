//! Block equality outside a rewrite's independently reconstructed region.
//! Each replay chooses its input iterator, including reachability filtering.

use optimization_unit::{OptimizationBlock, PsiOptimizationFunction};
use semantic_vocabulary::BlockId;

use crate::OptimizationUnitValidationError;

pub(super) fn validate_outside_region<'input>(
    input_blocks: impl IntoIterator<Item = &'input OptimizationBlock>,
    output_function: &PsiOptimizationFunction,
    expected_blocks: &[BlockId],
) -> Result<(), OptimizationUnitValidationError> {
    for input_block in input_blocks {
        if !expected_blocks.contains(&input_block.id)
            && output_function.block(input_block.id) != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(())
}
