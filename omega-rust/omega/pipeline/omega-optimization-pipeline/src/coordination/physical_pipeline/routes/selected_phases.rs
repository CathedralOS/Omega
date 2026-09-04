use crate::{
    StagedOptimizedVerifiedPhysicalPipeline, ValidatedOptimizedTargetOperations,
    baseline_target_register_environment, run_selected_lowering_optimizations,
    stage_function_relative_layout_optimization_realization, stage_optimized_allocation_legality,
    stage_optimized_allocation_legality_for_frameless_leaf, stage_optimized_live_ranges,
    stage_optimized_liveness,
    stage_optimized_post_allocation_machine_optimization_after_selected_lowering_for_catalog_entry,
    stage_optimized_post_allocation_machine_optimization_for_catalog_entry,
    stage_optimized_post_allocation_machine_plan,
    stage_optimized_post_allocation_machine_plan_after_selected_lowering,
    stage_optimized_register_homes, stage_optimized_register_homes_after_selected_lowering,
    stage_post_allocation_machine_function_relative_realization,
    stage_post_allocation_machine_function_relative_realization_after_selected_lowering,
    stage_selected_lowering_function_relative_realization,
};

use super::super::{OptimizedVerifiedPhysicalPipelineError, ResolvedNonAllocationComposition};
use super::stage_identity_function_relative_pipeline;

#[inline(never)]
pub(in crate::coordination::physical_pipeline) fn stage_non_allocation_recovery_physical_pipeline(
    optimized_target: ValidatedOptimizedTargetOperations,
    composition: ResolvedNonAllocationComposition,
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    let register_environment = baseline_target_register_environment(optimized_target.target())
        .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterEnvironment)?;
    let selected =
        omega_target_operations_to_selected_instructions::stage_optimized_instruction_selection(
            optimized_target,
            register_environment,
        )
        .map_err(OptimizedVerifiedPhysicalPipelineError::Selection)?;
    let liveness = stage_optimized_liveness(selected)
        .map_err(OptimizedVerifiedPhysicalPipelineError::Liveness)?;
    let ranges = stage_optimized_live_ranges(liveness)
        .map_err(OptimizedVerifiedPhysicalPipelineError::LiveRanges)?;
    match composition {
        ResolvedNonAllocationComposition::PostAllocationMachine {
            entry,
            after_selected_lowering: false,
        } => {
            let legality = stage_optimized_allocation_legality(ranges)
                .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
            let homes = stage_optimized_register_homes(legality)
                .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterHomes)?;
            let machine = stage_optimized_post_allocation_machine_plan(&homes)
                .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
            let optimization =
                stage_optimized_post_allocation_machine_optimization_for_catalog_entry(
                    &homes, &machine, entry,
                )
                .map_err(
                    OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineOptimization,
                )?;
            let realization = stage_post_allocation_machine_function_relative_realization(
                homes,
                machine,
                optimization,
            )
            .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)?;
            Ok(StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization })
        }
        ResolvedNonAllocationComposition::PostAllocationMachine {
            entry,
            after_selected_lowering: true,
        } => {
            let legality = stage_optimized_allocation_legality_for_frameless_leaf(ranges)
                .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
            let run = run_selected_lowering_optimizations(legality)
                .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedLowering)?;
            let homes = stage_optimized_register_homes_after_selected_lowering(run)
                .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedLoweringHomes)?;
            let machine =
                stage_optimized_post_allocation_machine_plan_after_selected_lowering(&homes)
                    .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
            let optimization =
                stage_optimized_post_allocation_machine_optimization_after_selected_lowering_for_catalog_entry(
                    &homes, &machine, entry,
                )
                .map_err(
                    OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineOptimization,
                )?;
            let realization =
                stage_post_allocation_machine_function_relative_realization_after_selected_lowering(
                    homes,
                    machine,
                    optimization,
                )
                .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)?;
            Ok(StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization })
        }
        ResolvedNonAllocationComposition::Identity => {
            let legality = stage_optimized_allocation_legality(ranges)
                .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
            let homes = stage_optimized_register_homes(legality)
                .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterHomes)?;
            stage_identity_function_relative_pipeline(homes)
        }
        ResolvedNonAllocationComposition::FunctionRelativeLayout => {
            let legality = stage_optimized_allocation_legality_for_frameless_leaf(ranges)
                .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
            let homes = stage_optimized_register_homes(legality)
                .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterHomes)?;
            let realization = stage_function_relative_layout_optimization_realization(homes)
                .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)?;
            Ok(StagedOptimizedVerifiedPhysicalPipeline::FunctionRelativeLayout { realization })
        }
        ResolvedNonAllocationComposition::SelectedLowering
        | ResolvedNonAllocationComposition::SelectedLoweringWithFunctionRelativeLayout => {
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
