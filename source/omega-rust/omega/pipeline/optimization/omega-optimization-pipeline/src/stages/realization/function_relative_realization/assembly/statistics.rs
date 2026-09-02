use super::super::prelude::*;
use super::super::{error::*, model::*};

pub(crate) fn function_relative_statistics(
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<
    FunctionRelativeOptimizationRealizationStatistics,
    FunctionRelativeOptimizationRealizationError,
> {
    let count = |value: usize| {
        u64::try_from(value)
            .map_err(|_| FunctionRelativeOptimizationRealizationError::StatisticsOverflow)
    };
    let functions = count(layout.functions().len())?;
    let blocks = layout
        .functions()
        .iter()
        .try_fold(0_u64, |total, function| {
            total
                .checked_add(count(function.blocks.len())?)
                .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)
        })?;
    let instructions = layout
        .functions()
        .iter()
        .flat_map(|function| &function.blocks)
        .try_fold(0_u64, |total, block| {
            total
                .checked_add(count(block.instructions.len())?)
                .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)
        })?;
    let bytes = layout
        .functions()
        .iter()
        .try_fold(0_u64, |total, function| {
            total
                .checked_add(function.byte_count)
                .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)
        })?;
    let structural_unit_functions = count(layout.structural_unit_functions().len())?;
    let structural_unit_blocks = structural_unit_functions;
    let structural_call_templates = layout
        .structural_unit_functions()
        .iter()
        .filter(|function| function.call.is_some())
        .try_fold(0_u64, |total, _| {
            total
                .checked_add(1)
                .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)
        })?;
    let structural_unit_instructions = structural_call_templates
        .checked_add(structural_unit_functions)
        .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)?;
    let structural_unit_bytes =
        layout
            .structural_unit_functions()
            .iter()
            .try_fold(0_u64, |total, function| {
                total
                    .checked_add(function.byte_count)
                    .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)
            })?;
    let resolved_conditional_branches = layout
        .functions()
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .filter(|instruction| instruction.branch.is_some())
        .try_fold(0_u64, |total, _| {
            total
                .checked_add(1)
                .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)
        })?;
    let ordinary_internal_machine_fixups = layout
        .functions()
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .filter(|instruction| instruction.internal_machine_fixup.is_some())
        .try_fold(0_u64, |total, _| {
            total
                .checked_add(1)
                .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)
        })?;
    let unresolved_internal_machine_fixups = ordinary_internal_machine_fixups
        .checked_add(structural_call_templates)
        .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)?;
    Ok(FunctionRelativeOptimizationRealizationStatistics {
        functions,
        blocks,
        instructions,
        bytes,
        resolved_conditional_branches,
        structural_unit_functions,
        structural_unit_blocks,
        structural_unit_instructions,
        structural_unit_bytes,
        unresolved_internal_machine_fixups,
    })
}

pub(crate) fn seal_function_relative_manifest(
    mut record: FunctionRelativeOptimizationRealizationManifest,
) -> ValidatedFunctionRelativeOptimizationRealizationManifest {
    record.identity = record.recomputed_identity();
    ValidatedFunctionRelativeOptimizationRealizationManifest { record }
}
