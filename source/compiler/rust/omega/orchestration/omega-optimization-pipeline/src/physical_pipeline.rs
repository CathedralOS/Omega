use omega_lowering_optimizer::{
    ValidatedOptimizedAbstractPlan, lower_optimized_to_target_operations_with_provider_executions,
};
use omega_optimization_core::{OptimizationExecutionPhase, OptimizationSelectionIdentity};
use omega_target::NativeTarget;
use omega_terminal_abstract_operations_to_target_operations::{
    AdmittedTerminalBoundarySettlement, LoweringError,
};

use crate::{
    FunctionRelativeOptimizationRealizationError, OptimizedAllocationLegalityCustodyError,
    OptimizedLiteralFoldCustodyError, OptimizedLiveRangeCustodyError,
    OptimizedLivenessCustodyError, OptimizedPostAllocationMachinePipelineError,
    OptimizedPostSelectedLoweringHomeCustodyError, OptimizedRegisterHomeCustodyError,
    OptimizedSelectionPipelineError, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedRegisterHomes, StagedSelectedLoweringFunctionRelativeRealization,
    run_selected_lowering_optimizations, stage_optimized_allocation_legality,
    stage_optimized_instruction_selection, stage_optimized_live_ranges, stage_optimized_liveness,
    stage_optimized_post_allocation_machine_plan, stage_optimized_register_homes,
    stage_optimized_register_homes_after_selected_lowering,
    stage_selected_lowering_function_relative_realization,
};

/// Complete currently admitted physical validation for one explicitly selected
/// optimized source. Both variants stop before frame construction, machine
/// emission, object construction, installation, or publication.
#[derive(Debug)]
pub enum StagedOptimizedVerifiedPhysicalPipeline {
    PsiOnly {
        homes: StagedOptimizedRegisterHomes,
        machine: StagedOptimizedPostAllocationMachinePlan,
    },
    SelectedLowering {
        realization: StagedSelectedLoweringFunctionRelativeRealization,
    },
}

impl StagedOptimizedVerifiedPhysicalPipeline {
    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachinePlan {
        match self {
            Self::PsiOnly { machine, .. } => machine,
            Self::SelectedLowering { realization } => realization.machine(),
        }
    }

    pub const fn function_relative_realization(
        &self,
    ) -> Option<&StagedSelectedLoweringFunctionRelativeRealization> {
        match self {
            Self::PsiOnly { .. } => None,
            Self::SelectedLowering { realization } => Some(realization),
        }
    }

    pub fn selections(&self) -> OptimizationSelectionIdentity {
        match self {
            Self::PsiOnly { homes, .. } => homes
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .selections()
                .identity(),
            Self::SelectedLowering { realization } => realization
                .homes()
                .selected_lowering_run()
                .custody()
                .selections(),
        }
    }

    pub const fn selected_lowering_completion(
        &self,
    ) -> Option<omega_optimization_core::SelectedLoweringOptimizationCompletionIdentity> {
        match self {
            Self::PsiOnly { .. } => None,
            Self::SelectedLowering { realization } => Some(
                realization
                    .homes()
                    .selected_lowering_run()
                    .custody()
                    .identity(),
            ),
        }
    }
}

#[derive(Debug)]
pub enum OptimizedVerifiedPhysicalPipelineError {
    TargetLowering(LoweringError),
    Selection(OptimizedSelectionPipelineError),
    Liveness(OptimizedLivenessCustodyError),
    LiveRanges(OptimizedLiveRangeCustodyError),
    AllocationLegality(OptimizedAllocationLegalityCustodyError),
    RegisterHomes(OptimizedRegisterHomeCustodyError),
    SelectedLowering(OptimizedLiteralFoldCustodyError),
    SelectedLoweringHomes(OptimizedPostSelectedLoweringHomeCustodyError),
    PostAllocationMachine(OptimizedPostAllocationMachinePipelineError),
    FunctionRelativeRealization(FunctionRelativeOptimizationRealizationError),
}

impl std::fmt::Display for OptimizedVerifiedPhysicalPipelineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized verified physical staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedVerifiedPhysicalPipelineError {}

/// Lower one verified optimized plan through every currently admitted
/// selected/physical validation stage. Phase routing is derived from the exact
/// retained build suite; callers cannot request or skip selected-lowering work
/// independently.
pub fn stage_optimized_verified_physical_pipeline_with_provider_executions(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
    settlements: &[AdmittedTerminalBoundarySettlement<'_>],
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    let target = lower_optimized_to_target_operations_with_provider_executions(
        optimized,
        target,
        settlements,
    )
    .map_err(OptimizedVerifiedPhysicalPipelineError::TargetLowering)?;
    let selected = stage_optimized_instruction_selection(target)
        .map_err(OptimizedVerifiedPhysicalPipelineError::Selection)?;
    let liveness = stage_optimized_liveness(selected)
        .map_err(OptimizedVerifiedPhysicalPipelineError::Liveness)?;
    let ranges = stage_optimized_live_ranges(liveness)
        .map_err(OptimizedVerifiedPhysicalPipelineError::LiveRanges)?;
    let legality = stage_optimized_allocation_legality(ranges)
        .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
    let selected_lowering = legality
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .selections()
        .for_phase(OptimizationExecutionPhase::SelectedLowering);

    if selected_lowering.is_empty() {
        let homes = stage_optimized_register_homes(legality)
            .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterHomes)?;
        let machine = stage_optimized_post_allocation_machine_plan(&homes)
            .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
        Ok(StagedOptimizedVerifiedPhysicalPipeline::PsiOnly { homes, machine })
    } else {
        let run = run_selected_lowering_optimizations(legality)
            .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedLowering)?;
        let homes = stage_optimized_register_homes_after_selected_lowering(run)
            .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedLoweringHomes)?;
        let realization = stage_selected_lowering_function_relative_realization(homes)
            .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)?;
        Ok(StagedOptimizedVerifiedPhysicalPipeline::SelectedLowering { realization })
    }
}
