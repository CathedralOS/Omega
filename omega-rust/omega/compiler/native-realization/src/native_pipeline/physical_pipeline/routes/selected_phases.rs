use crate::StagedOptimizedVerifiedPhysicalPipeline;
use machine_emission::{
    stage_function_relative_layout_optimization_realization,
    stage_post_allocation_machine_function_relative_realization,
    stage_selected_lowering_function_relative_realization,
};
use post_allocation_machine_to_post_allocation_machine::stage_optimized_post_allocation_machine_optimization_for_catalog_entry;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use selected_instructions_to_register_homes::RetainedAllocation;

use super::super::{OptimizedVerifiedPhysicalPipelineError, ResolvedRealizationPlan};
use super::stage_identity_function_relative_pipeline;

#[inline(never)]
pub(in crate::native_pipeline::physical_pipeline) fn realize_allocated_program(
    allocation: RetainedAllocation,
    machine: StagedOptimizedPostAllocationMachinePlan,
    composition: ResolvedRealizationPlan,
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    match composition {
        ResolvedRealizationPlan::PostAllocationMachine { entry } => {
            let current = allocation.current();
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
            Ok(StagedOptimizedVerifiedPhysicalPipeline::from(realization))
        }
        ResolvedRealizationPlan::Identity => {
            stage_identity_function_relative_pipeline(allocation, machine)
        }
        ResolvedRealizationPlan::FunctionRelativeLayout => {
            let realization =
                stage_function_relative_layout_optimization_realization(allocation, machine)
                    .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)?;
            Ok(StagedOptimizedVerifiedPhysicalPipeline::from(realization))
        }
        ResolvedRealizationPlan::SelectedLowering => {
            let realization =
                stage_selected_lowering_function_relative_realization(allocation, machine)
                    .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)?;
            Ok(StagedOptimizedVerifiedPhysicalPipeline::from(realization))
        }
    }
}
