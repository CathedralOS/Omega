use omega_lowering_optimizer::{
    ValidatedOptimizedAbstractPlan, lower_optimized_to_target_operations_with_provider_executions,
};
use omega_optimization_core::{OptimizationExecutionPhase, OptimizationSelectionIdentity};
use omega_optimization_validation::ValidatedPrePhysicalOptimizationManifest;
use omega_regalloc::{TerminalFixedViewCopyPolicy, ValidatedPostAllocationOptimizationManifest};
use omega_target::NativeTarget;
use omega_terminal_abstract_operations_to_target_operations::{
    AdmittedTerminalBoundarySettlement, LoweringError,
};

use crate::{
    FunctionRelativeOptimizationRealizationError, OptimizedAllocationLegalityCustodyError,
    OptimizedFixedViewCopyCustodyError, OptimizedLiteralFoldCustodyError,
    OptimizedLiveRangeCustodyError, OptimizedLivenessCustodyError,
    OptimizedPostAllocationMachineOptimizationError, OptimizedPostAllocationMachinePipelineError,
    OptimizedPostCopyRegisterHomeCustodyError, OptimizedPostSelectedLoweringHomeCustodyError,
    OptimizedRegisterHomeCustodyError, OptimizedSelectedReanalysisError,
    OptimizedSelectionPipelineError, StagedFunctionRelativeLayoutOptimizationRealization,
    StagedOptimizedAarch64CbnzFusion, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterFixedViewCopies,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
    StagedSelectedLoweringFunctionRelativeRealization,
    ValidatedFunctionRelativeOptimizationRealizationManifest, run_selected_lowering_optimizations,
    stage_function_relative_layout_optimization_realization, stage_optimized_aarch64_cbnz_fusion,
    stage_optimized_aarch64_cbnz_fusion_after_selected_lowering,
    stage_optimized_allocation_legality, stage_optimized_allocation_legality_for_frameless_leaf,
    stage_optimized_fixed_view_copies, stage_optimized_instruction_selection,
    stage_optimized_live_ranges, stage_optimized_liveness,
    stage_optimized_post_allocation_machine_plan,
    stage_optimized_post_allocation_machine_plan_after_fixed_view_copies,
    stage_optimized_post_allocation_machine_plan_after_selected_lowering,
    stage_optimized_register_homes, stage_optimized_register_homes_after_fixed_view_copies,
    stage_optimized_register_homes_after_selected_lowering, stage_optimized_selected_reanalysis,
    stage_selected_lowering_function_relative_realization,
};

/// Complete currently admitted physical validation for one explicitly selected
/// optimized source. All variants stop before frame construction, machine
/// emission, object construction, installation, or publication.
#[derive(Debug)]
pub enum StagedOptimizedVerifiedPhysicalPipeline {
    PsiOnly {
        homes: StagedOptimizedRegisterHomes,
        machine: StagedOptimizedPostAllocationMachinePlan,
    },
    PostAllocationMachine {
        homes: StagedOptimizedRegisterHomes,
        machine: StagedOptimizedPostAllocationMachinePlan,
        optimization: StagedOptimizedAarch64CbnzFusion,
    },
    SelectedLoweringPostAllocationMachine {
        homes: StagedOptimizedRegisterHomesAfterSelectedLowering,
        machine: StagedOptimizedPostAllocationMachinePlan,
        optimization: StagedOptimizedAarch64CbnzFusion,
    },
    AllocationRecovery {
        homes: StagedOptimizedRegisterHomesAfterFixedViewCopies,
        machine: StagedOptimizedPostAllocationMachinePlan,
    },
    FunctionRelativeLayout {
        realization: StagedFunctionRelativeLayoutOptimizationRealization,
    },
    SelectedLowering {
        realization: StagedSelectedLoweringFunctionRelativeRealization,
    },
}

impl StagedOptimizedVerifiedPhysicalPipeline {
    pub const fn pre_physical_manifest(&self) -> &ValidatedPrePhysicalOptimizationManifest {
        match self {
            Self::PsiOnly { homes, .. } => homes
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::PostAllocationMachine { homes, .. } => homes
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::SelectedLoweringPostAllocationMachine { homes, .. } => homes
                .selected_lowering_run()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::AllocationRecovery { homes, .. } => homes
                .reanalysis_stage()
                .transformation_stage()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::FunctionRelativeLayout { realization } => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::SelectedLowering { realization } => realization
                .homes()
                .selected_lowering_run()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
        }
    }

    pub const fn post_allocation_manifest(&self) -> &ValidatedPostAllocationOptimizationManifest {
        match self {
            Self::PsiOnly { homes, .. } => homes.post_allocation_manifest(),
            Self::PostAllocationMachine { homes, .. } => homes.post_allocation_manifest(),
            Self::SelectedLoweringPostAllocationMachine { homes, .. } => {
                homes.post_allocation_manifest()
            }
            Self::AllocationRecovery { homes, .. } => homes.post_allocation_manifest(),
            Self::FunctionRelativeLayout { realization } => {
                realization.homes().post_allocation_manifest()
            }
            Self::SelectedLowering { realization } => {
                realization.homes().post_allocation_manifest()
            }
        }
    }

    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachinePlan {
        match self {
            Self::PsiOnly { machine, .. } => machine,
            Self::PostAllocationMachine { machine, .. }
            | Self::SelectedLoweringPostAllocationMachine { machine, .. }
            | Self::AllocationRecovery { machine, .. } => machine,
            Self::FunctionRelativeLayout { realization } => realization.machine(),
            Self::SelectedLowering { realization } => realization.machine(),
        }
    }

    pub const fn function_relative_realization(
        &self,
    ) -> Option<&StagedSelectedLoweringFunctionRelativeRealization> {
        match self {
            Self::PsiOnly { .. }
            | Self::PostAllocationMachine { .. }
            | Self::SelectedLoweringPostAllocationMachine { .. }
            | Self::AllocationRecovery { .. }
            | Self::FunctionRelativeLayout { .. } => None,
            Self::SelectedLowering { realization } => Some(realization),
        }
    }

    pub const fn function_relative_manifest(
        &self,
    ) -> Option<&ValidatedFunctionRelativeOptimizationRealizationManifest> {
        match self {
            Self::PsiOnly { .. }
            | Self::PostAllocationMachine { .. }
            | Self::SelectedLoweringPostAllocationMachine { .. }
            | Self::AllocationRecovery { .. } => None,
            Self::FunctionRelativeLayout { realization } => Some(realization.manifest()),
            Self::SelectedLowering { realization } => Some(realization.manifest()),
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
            Self::PostAllocationMachine { optimization, .. }
            | Self::SelectedLoweringPostAllocationMachine { optimization, .. } => {
                optimization.custody().selections()
            }
            Self::AllocationRecovery { homes, .. } => homes
                .reanalysis_stage()
                .transformation_stage()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .selections()
                .identity(),
            Self::FunctionRelativeLayout { realization } => realization
                .homes()
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
            Self::PsiOnly { .. }
            | Self::PostAllocationMachine { .. }
            | Self::AllocationRecovery { .. }
            | Self::FunctionRelativeLayout { .. } => None,
            Self::SelectedLoweringPostAllocationMachine { homes, .. } => {
                Some(homes.selected_lowering_run().custody().identity())
            }
            Self::SelectedLowering { realization } => Some(
                realization
                    .homes()
                    .selected_lowering_run()
                    .custody()
                    .identity(),
            ),
        }
    }

    pub const fn post_allocation_machine_optimization(
        &self,
    ) -> Option<&StagedOptimizedAarch64CbnzFusion> {
        match self {
            Self::PostAllocationMachine { optimization, .. }
            | Self::SelectedLoweringPostAllocationMachine { optimization, .. } => {
                Some(optimization)
            }
            Self::PsiOnly { .. }
            | Self::AllocationRecovery { .. }
            | Self::FunctionRelativeLayout { .. }
            | Self::SelectedLowering { .. } => None,
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
    PostAllocationMachineOptimization(OptimizedPostAllocationMachineOptimizationError),
    FixedViewCopies(OptimizedFixedViewCopyCustodyError),
    SelectedReanalysis(OptimizedSelectedReanalysisError),
    PostCopyRegisterHomes(OptimizedPostCopyRegisterHomeCustodyError),
    UnsupportedPhysicalPhaseComposition,
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
    let selected_lowering = ranges
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .selections()
        .for_phase(OptimizationExecutionPhase::SelectedLowering);
    let function_relative_layout = ranges
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .selections()
        .for_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    let post_allocation_machine = ranges
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .selections()
        .for_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let allocation_recovery = ranges
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .selections()
        .for_phase(OptimizationExecutionPhase::AllocationRecovery);

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
        let legality = stage_optimized_allocation_legality(ranges)
            .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
        let copies = stage_optimized_fixed_view_copies(
            legality,
            TerminalFixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
            budget,
        )
        .map_err(OptimizedVerifiedPhysicalPipelineError::FixedViewCopies)?;
        let reanalysis = stage_optimized_selected_reanalysis(copies)
            .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedReanalysis)?;
        let homes = stage_optimized_register_homes_after_fixed_view_copies(reanalysis)
            .map_err(OptimizedVerifiedPhysicalPipelineError::PostCopyRegisterHomes)?;
        let machine = stage_optimized_post_allocation_machine_plan_after_fixed_view_copies(&homes)
            .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
        return Ok(StagedOptimizedVerifiedPhysicalPipeline::AllocationRecovery { homes, machine });
    }

    if !post_allocation_machine.is_empty() {
        if !function_relative_layout.is_empty() {
            return Err(
                OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition,
            );
        }
        if selected_lowering.is_empty() {
            let legality = stage_optimized_allocation_legality(ranges)
                .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
            let homes = stage_optimized_register_homes(legality)
                .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterHomes)?;
            let machine = stage_optimized_post_allocation_machine_plan(&homes)
                .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
            let optimization = stage_optimized_aarch64_cbnz_fusion(&homes, &machine).map_err(
                OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineOptimization,
            )?;
            return Ok(
                StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine {
                    homes,
                    machine,
                    optimization,
                },
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
        let optimization = stage_optimized_aarch64_cbnz_fusion_after_selected_lowering(
            &homes, &machine,
        )
        .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineOptimization)?;
        return Ok(
            StagedOptimizedVerifiedPhysicalPipeline::SelectedLoweringPostAllocationMachine {
                homes,
                machine,
                optimization,
            },
        );
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
