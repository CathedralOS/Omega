use crate::{
    StagedOptimizedLiveRanges, StagedOptimizedVerifiedPhysicalPipeline,
    run_selected_lowering_optimizations, stage_function_relative_layout_optimization_realization,
    stage_optimized_allocation_legality, stage_optimized_allocation_legality_for_frameless_leaf,
    stage_optimized_post_allocation_machine_optimization_for_catalog_entry,
    stage_optimized_post_allocation_machine_plan, stage_optimized_register_homes,
    stage_optimized_register_homes_after_selected_lowering,
    stage_post_allocation_machine_function_relative_realization, stage_register_allocation,
    stage_selected_lowering_function_relative_realization,
};

use super::super::{OptimizedVerifiedPhysicalPipelineError, ResolvedRealizationPlan};
use super::stage_identity_function_relative_pipeline;

#[inline(never)]
pub(in crate::coordination::physical_pipeline) fn stage_allocation_and_realization(
    ranges: StagedOptimizedLiveRanges,
    composition: ResolvedRealizationPlan,
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    match composition {
        ResolvedRealizationPlan::PostAllocationMachine { entry } => {
            let allocation = stage_register_allocation(ranges)
                .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterAllocation)?;
            let current = allocation.current();
            let machine = stage_optimized_post_allocation_machine_plan(&current)
                .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
            let optimization =
                stage_optimized_post_allocation_machine_optimization_for_catalog_entry(
                    &current, &machine, entry,
                )
                .map_err(
                    OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineOptimization,
                )?;
            let realization = stage_post_allocation_machine_function_relative_realization(
                allocation,
                machine,
                optimization,
            )
            .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)?;
            Ok(StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization })
        }
        ResolvedRealizationPlan::Identity => {
            let legality = stage_optimized_allocation_legality(ranges)
                .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
            let homes = stage_optimized_register_homes(legality)
                .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterHomes)?;
            stage_identity_function_relative_pipeline(homes)
        }
        ResolvedRealizationPlan::FunctionRelativeLayout => {
            let legality = stage_optimized_allocation_legality_for_frameless_leaf(ranges)
                .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
            let homes = stage_optimized_register_homes(legality)
                .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterHomes)?;
            let realization = stage_function_relative_layout_optimization_realization(homes)
                .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)?;
            Ok(StagedOptimizedVerifiedPhysicalPipeline::FunctionRelativeLayout { realization })
        }
        ResolvedRealizationPlan::SelectedLowering => {
            let legality = stage_optimized_allocation_legality_for_frameless_leaf(ranges)
                .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
            let run = run_selected_lowering_optimizations(legality)
                .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedLowering)?;
            let homes = stage_optimized_register_homes_after_selected_lowering(run)
                .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedLoweringHomes)?;
            let realization = stage_selected_lowering_function_relative_realization(homes)
                .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)?;
            Ok(StagedOptimizedVerifiedPhysicalPipeline::SelectedLowering { realization })
        }
    }
}
