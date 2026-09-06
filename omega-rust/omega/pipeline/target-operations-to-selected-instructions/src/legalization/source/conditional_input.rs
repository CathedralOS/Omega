//! Catalog-backed input recognition shared by routing and source construction.

use super::conditions::{self, DerivedCondition};
use super::matchers::match_scalar_form;
use super::shared::*;
use crate::legalization::catalog::{LegalizationFormDescriptor, LegalizationShapeConstraints};

pub(super) fn match_input<'a>(
    function: usize,
    target: &'a target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
) -> Result<(DerivedCondition<'a>, &'static LegalizationFormDescriptor), LegalizationError> {
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || target.attachment != abstracted.attachment
        || abstracted.block_entries.len() != 3
        || optimized.blocks.len() != 3
        || optimized.entry != abstracted.entry
        || optimized.blocks[0].id != abstracted.block_entries[0].block
        || optimized.blocks[1].id != abstracted.block_entries[1].block
        || optimized.blocks[2].id != abstracted.block_entries[2].block
        || abstracted
            .block_entries
            .iter()
            .any(|entry| !entry.parameters.is_empty())
        || optimized
            .blocks
            .iter()
            .any(|block| !block.parameters.is_empty())
    {
        return Err(Error::UnsupportedSourceShape { function });
    }

    let condition = conditions::derive(function, target, abstracted, optimized)?;
    if condition.result_type.is_address()
        || condition.result_type.sign() != IntegerSign::Unsigned
        || condition.result_type.bits() != 64
    {
        return Err(Error::UnsupportedIntegerShape { function });
    }
    let form = match_scalar_form(
        condition.shape,
        condition.when_true.control.as_ref(),
        condition.when_false.control.as_ref(),
    )
    .ok_or(Error::UnsupportedSourceShape { function })?;
    let LegalizationShapeConstraints::Scalar(constraints) = form.constraints else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if optimized.blocks[0].nodes.len() != constraints.entry_node_count
        || abstracted.operations.len() != constraints.operation_count
        || abstracted.parameters.len() != constraints.parameter_count
        || optimized.parameters.len() != constraints.parameter_count
        || abstracted
            .block_entries
            .iter()
            .zip(constraints.block_offsets)
            .any(|(entry, offset)| entry.operation_offset != offset)
        || optimized.blocks[1].nodes.len() != constraints.leaf_node_counts[0]
        || optimized.blocks[2].nodes.len() != constraints.leaf_node_counts[1]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    Ok((condition, form))
}
