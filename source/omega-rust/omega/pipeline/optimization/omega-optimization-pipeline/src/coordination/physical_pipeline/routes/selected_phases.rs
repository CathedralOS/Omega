use omega_machine_optimizer::selected_post_allocation_machine_rule;
use omega_optimization_core::{Optimization, OptimizationExecutionPhase};
use omega_regalloc::{FixedViewCopyPolicy, selected_allocation_recovery_rule};

use crate::{
    StagedOptimizedVerifiedPhysicalPipeline, ValidatedOptimizedTargetOperations,
    run_selected_lowering_optimizations, stage_function_relative_layout_optimization_realization,
    stage_optimized_allocation_legality, stage_optimized_allocation_legality_for_frameless_leaf,
    stage_optimized_fixed_view_copies, stage_optimized_instruction_selection,
    stage_optimized_live_ranges, stage_optimized_liveness,
    stage_optimized_post_allocation_machine_optimization,
    stage_optimized_post_allocation_machine_optimization_after_selected_lowering,
    stage_optimized_post_allocation_machine_plan,
    stage_optimized_post_allocation_machine_plan_after_fixed_view_copies,
    stage_optimized_post_allocation_machine_plan_after_selected_lowering,
    stage_optimized_register_homes, stage_optimized_register_homes_after_fixed_view_copies,
    stage_optimized_register_homes_after_selected_lowering, stage_optimized_selected_reanalysis,
    stage_post_allocation_machine_function_relative_realization,
    stage_post_allocation_machine_function_relative_realization_after_selected_lowering,
    stage_selected_lowering_function_relative_realization,
};

use super::super::OptimizedVerifiedPhysicalPipelineError;

#[inline(never)]
pub(in crate::coordination::physical_pipeline) fn stage_non_active_resident_rematerialization_physical_pipeline(
    optimized_target: ValidatedOptimizedTargetOperations,
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    let selected = stage_optimized_instruction_selection(optimized_target)
        .map_err(OptimizedVerifiedPhysicalPipelineError::Selection)?;
    let liveness = stage_optimized_liveness(selected)
        .map_err(OptimizedVerifiedPhysicalPipelineError::Liveness)?;
    let ranges = stage_optimized_live_ranges(liveness)
        .map_err(OptimizedVerifiedPhysicalPipelineError::LiveRanges)?;
    let selections = ranges
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .selections();
    let selected_lowering = selections.for_phase(OptimizationExecutionPhase::SelectedLowering);
    let function_relative_layout =
        selections.for_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    let post_allocation_machine =
        selections.for_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let allocation_recovery = selections.for_phase(OptimizationExecutionPhase::AllocationRecovery);

    if !allocation_recovery.is_empty() {
        if !selected_lowering.is_empty()
            || !function_relative_layout.is_empty()
            || !post_allocation_machine.is_empty()
        {
            return Err(
                OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition,
            );
        }
        let budget = ranges
            .liveness_stage()
            .selected_stage()
            .optimized_target()
            .optimized()
            .budget_per_pass();
        match selected_allocation_recovery_rule(selections).map_err(|_| {
            OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition
        })? {
            Some(Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1) => {
                let legality = stage_optimized_allocation_legality(ranges)
                    .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
                let copies = stage_optimized_fixed_view_copies(
                    legality,
                    FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
                    budget,
                )
                .map_err(OptimizedVerifiedPhysicalPipelineError::FixedViewCopies)?;
                let reanalysis = stage_optimized_selected_reanalysis(copies)
                    .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedReanalysis)?;
                let homes = stage_optimized_register_homes_after_fixed_view_copies(reanalysis)
                    .map_err(OptimizedVerifiedPhysicalPipelineError::PostCopyRegisterHomes)?;
                let machine =
                    stage_optimized_post_allocation_machine_plan_after_fixed_view_copies(&homes)
                        .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
                return Ok(
                    StagedOptimizedVerifiedPhysicalPipeline::AllocationRecovery { homes, machine },
                );
            }
            None | Some(Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1) => {
                return Err(
                    OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition,
                );
            }
            Some(_) => unreachable!("the allocation-recovery rule catalog is closed"),
        }
    }

    if !post_allocation_machine.is_empty() {
        if !function_relative_layout.is_empty() {
            return Err(
                OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition,
            );
        }
        selected_post_allocation_machine_rule(selections).map_err(|_| {
            OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition
        })?;
        if selected_lowering.is_empty() {
            let legality = stage_optimized_allocation_legality(ranges)
                .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
            let homes = stage_optimized_register_homes(legality)
                .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterHomes)?;
            let machine = stage_optimized_post_allocation_machine_plan(&homes)
                .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
            let optimization = stage_optimized_post_allocation_machine_optimization(
                &homes, &machine,
            )
            .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineOptimization)?;
            let realization = stage_post_allocation_machine_function_relative_realization(
                homes,
                machine,
                optimization,
            )
            .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)?;
            return Ok(
                StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization },
            );
        }
        let legality = stage_optimized_allocation_legality_for_frameless_leaf(ranges)
            .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
        let run = run_selected_lowering_optimizations(legality)
            .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedLowering)?;
        let homes = stage_optimized_register_homes_after_selected_lowering(run)
            .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedLoweringHomes)?;
        let machine = stage_optimized_post_allocation_machine_plan_after_selected_lowering(&homes)
            .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
        let optimization =
            stage_optimized_post_allocation_machine_optimization_after_selected_lowering(
                &homes, &machine,
            )
            .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineOptimization)?;
        let realization =
            stage_post_allocation_machine_function_relative_realization_after_selected_lowering(
                homes,
                machine,
                optimization,
            )
            .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)?;
        return Ok(StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization });
    }

    if selected_lowering.is_empty() && function_relative_layout.is_empty() {
        let legality = stage_optimized_allocation_legality(ranges)
            .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
        let homes = stage_optimized_register_homes(legality)
            .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterHomes)?;
        let machine = stage_optimized_post_allocation_machine_plan(&homes)
            .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
        Ok(StagedOptimizedVerifiedPhysicalPipeline::PsiOnly { homes, machine })
    } else if selected_lowering.is_empty() {
        let legality = stage_optimized_allocation_legality_for_frameless_leaf(ranges)
            .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
        let homes = stage_optimized_register_homes(legality)
            .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterHomes)?;
        let realization = stage_function_relative_layout_optimization_realization(homes)
            .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)?;
        Ok(StagedOptimizedVerifiedPhysicalPipeline::FunctionRelativeLayout { realization })
    } else {
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
