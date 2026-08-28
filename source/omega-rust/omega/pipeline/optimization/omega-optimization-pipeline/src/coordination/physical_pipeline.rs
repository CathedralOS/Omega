use omega_abstract_operations_to_target_operations::{AdmittedBoundarySettlement, LoweringError};
use omega_optimization_core::{
    Optimization, OptimizationExecutionPhase, OptimizationSelectionIdentity,
};
use omega_optimization_run_to_abstract_operations::ValidatedOptimizedAbstractPlan;
use omega_optimization_validation::ValidatedPrePhysicalOptimizationManifest;
use omega_psi_to_abstract_operations::AdmittedProviderInstallation;
use omega_regalloc::{
    FixedViewCopyPolicy, PressureRematerializationPolicy, RecoveryClassificationPolicy,
    SpillChoicePolicy, ValidatedPostAllocationOptimizationManifest,
};
use omega_target::NativeTarget;

use crate::{
    FunctionRelativeOptimizationRealizationError,
    OptimizedAarch64MovnResolvedSelectedFormLayoutError,
    OptimizedActiveResidentRematerializationError,
    OptimizedActiveResidentRematerializationFunctionRelativeRealizationError,
    OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError,
    OptimizedActiveResidentRematerializationSelectedFormEncodingError,
    OptimizedAllocationLegalityCustodyError, OptimizedFixedViewCopyCustodyError,
    OptimizedLiteralFoldCustodyError, OptimizedLiveRangeCustodyError,
    OptimizedLivenessCustodyError, OptimizedPostAllocationMachineOptimizationError,
    OptimizedPostAllocationMachinePipelineError, OptimizedPostCopyRegisterHomeCustodyError,
    OptimizedPostSelectedLoweringHomeCustodyError, OptimizedRegisterHomeCustodyError,
    OptimizedSelectedReanalysisError, OptimizedSelectionPipelineError,
    StagedAarch64CbnzFunctionRelativeRealization,
    StagedFunctionRelativeLayoutOptimizationRealization, StagedOptimizedAarch64CbnzFusion,
    StagedOptimizedAarch64MovnMaterialization,
    StagedOptimizedAarch64MovnResolvedSelectedFormLayout,
    StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
    StagedOptimizedLiveRanges, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterFixedViewCopies,
    StagedSelectedLoweringAarch64CbnzFunctionRelativeRealization,
    StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayout,
    StagedSelectedLoweringFunctionRelativeRealization,
    ValidatedFunctionRelativeOptimizationRealizationManifest, ValidatedOptimizedTargetOperations,
    lower_optimized_to_target_operations_with_provider_executions,
    lower_optimized_to_target_operations_with_provider_executions_and_installation,
    run_selected_lowering_optimizations, stage_aarch64_cbnz_function_relative_realization,
    stage_function_relative_layout_optimization_realization, stage_optimized_aarch64_cbnz_fusion,
    stage_optimized_aarch64_cbnz_fusion_after_selected_lowering,
    stage_optimized_aarch64_movn_materialization,
    stage_optimized_aarch64_movn_materialization_after_selected_lowering,
    stage_optimized_aarch64_movn_resolved_selected_form_layout,
    stage_optimized_active_resident_rematerialization,
    stage_optimized_active_resident_rematerialization_function_relative_realization,
    stage_optimized_active_resident_rematerialization_resolved_selected_form_layout,
    stage_optimized_active_resident_rematerialization_selected_form_encoding,
    stage_optimized_allocation_legality,
    stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1,
    stage_optimized_allocation_legality_for_frameless_leaf, stage_optimized_fixed_view_copies,
    stage_optimized_instruction_selection, stage_optimized_live_ranges, stage_optimized_liveness,
    stage_optimized_post_allocation_machine_plan,
    stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization,
    stage_optimized_post_allocation_machine_plan_after_fixed_view_copies,
    stage_optimized_post_allocation_machine_plan_after_selected_lowering,
    stage_optimized_register_homes, stage_optimized_register_homes_after_fixed_view_copies,
    stage_optimized_register_homes_after_selected_lowering, stage_optimized_selected_reanalysis,
    stage_selected_lowering_aarch64_cbnz_function_relative_realization,
    stage_selected_lowering_aarch64_movn_resolved_selected_form_layout,
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
        realization: StagedAarch64CbnzFunctionRelativeRealization,
    },
    PostAllocationMachineMovn {
        layout: StagedOptimizedAarch64MovnResolvedSelectedFormLayout,
    },
    SelectedLoweringPostAllocationMachine {
        realization: StagedSelectedLoweringAarch64CbnzFunctionRelativeRealization,
    },
    SelectedLoweringPostAllocationMachineMovn {
        layout: StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayout,
    },
    AllocationRecovery {
        homes: StagedOptimizedRegisterHomesAfterFixedViewCopies,
        machine: StagedOptimizedPostAllocationMachinePlan,
    },
    ActiveResidentRematerialization {
        realization: Box<StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization>,
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
            Self::PostAllocationMachine { realization } => realization
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::PostAllocationMachineMovn { layout } => layout
                .homes()
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::SelectedLoweringPostAllocationMachine { realization } => realization
                .homes()
                .selected_lowering_run()
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .optimized_target()
                .optimized()
                .pre_physical_manifest(),
            Self::SelectedLoweringPostAllocationMachineMovn { layout } => layout
                .homes()
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
            Self::ActiveResidentRematerialization { realization } => realization
                .source()
                .pre_layout()
                .source()
                .source()
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
            Self::PostAllocationMachine { realization } => {
                realization.homes().post_allocation_manifest()
            }
            Self::PostAllocationMachineMovn { layout } => layout.homes().post_allocation_manifest(),
            Self::SelectedLoweringPostAllocationMachine { realization } => {
                realization.homes().post_allocation_manifest()
            }
            Self::SelectedLoweringPostAllocationMachineMovn { layout } => {
                layout.homes().post_allocation_manifest()
            }
            Self::AllocationRecovery { homes, .. } => homes.post_allocation_manifest(),
            Self::ActiveResidentRematerialization { realization } => realization
                .source()
                .pre_layout()
                .source()
                .post_allocation_manifest(),
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
            Self::PostAllocationMachine { realization } => realization.machine(),
            Self::PostAllocationMachineMovn { layout } => layout.machine(),
            Self::SelectedLoweringPostAllocationMachine { realization } => realization.machine(),
            Self::SelectedLoweringPostAllocationMachineMovn { layout } => layout.machine(),
            Self::AllocationRecovery { machine, .. } => machine,
            Self::ActiveResidentRematerialization { realization } => {
                realization.source().pre_layout().machine()
            }
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
            | Self::PostAllocationMachineMovn { .. }
            | Self::SelectedLoweringPostAllocationMachine { .. }
            | Self::SelectedLoweringPostAllocationMachineMovn { .. }
            | Self::AllocationRecovery { .. }
            | Self::ActiveResidentRematerialization { .. }
            | Self::FunctionRelativeLayout { .. } => None,
            Self::SelectedLowering { realization } => Some(realization),
        }
    }

    pub const fn function_relative_manifest(
        &self,
    ) -> Option<&ValidatedFunctionRelativeOptimizationRealizationManifest> {
        match self {
            Self::PsiOnly { .. }
            | Self::PostAllocationMachineMovn { .. }
            | Self::SelectedLoweringPostAllocationMachineMovn { .. }
            | Self::AllocationRecovery { .. } => None,
            Self::ActiveResidentRematerialization { realization } => Some(realization.manifest()),
            Self::PostAllocationMachine { realization } => Some(realization.manifest()),
            Self::SelectedLoweringPostAllocationMachine { realization } => {
                Some(realization.manifest())
            }
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
            Self::PostAllocationMachine { realization } => {
                realization.fusion().custody().selections()
            }
            Self::PostAllocationMachineMovn { layout } => {
                layout.materialization().custody().selections()
            }
            Self::SelectedLoweringPostAllocationMachine { realization } => {
                realization.fusion().custody().selections()
            }
            Self::SelectedLoweringPostAllocationMachineMovn { layout } => {
                layout.materialization().custody().selections()
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
            Self::ActiveResidentRematerialization { realization } => realization
                .source()
                .pre_layout()
                .source()
                .source()
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
            | Self::PostAllocationMachineMovn { .. }
            | Self::AllocationRecovery { .. }
            | Self::ActiveResidentRematerialization { .. }
            | Self::FunctionRelativeLayout { .. } => None,
            Self::SelectedLoweringPostAllocationMachine { realization } => Some(
                realization
                    .homes()
                    .selected_lowering_run()
                    .custody()
                    .identity(),
            ),
            Self::SelectedLoweringPostAllocationMachineMovn { layout } => {
                Some(layout.homes().selected_lowering_run().custody().identity())
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
            Self::PostAllocationMachine { realization } => Some(realization.fusion()),
            Self::SelectedLoweringPostAllocationMachine { realization } => {
                Some(realization.fusion())
            }
            Self::PsiOnly { .. }
            | Self::PostAllocationMachineMovn { .. }
            | Self::SelectedLoweringPostAllocationMachineMovn { .. }
            | Self::AllocationRecovery { .. }
            | Self::ActiveResidentRematerialization { .. }
            | Self::FunctionRelativeLayout { .. }
            | Self::SelectedLowering { .. } => None,
        }
    }

    pub const fn post_allocation_movn_optimization(
        &self,
    ) -> Option<&StagedOptimizedAarch64MovnMaterialization> {
        match self {
            Self::PostAllocationMachineMovn { layout } => Some(layout.materialization()),
            Self::SelectedLoweringPostAllocationMachineMovn { layout } => {
                Some(layout.materialization())
            }
            _ => None,
        }
    }

    pub const fn active_resident_rematerialization_function_relative_realization(
        &self,
    ) -> Option<&StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization> {
        match self {
            Self::ActiveResidentRematerialization { realization } => Some(realization),
            _ => None,
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
    ActiveResidentRematerialization(OptimizedActiveResidentRematerializationError),
    ActiveResidentRematerializationEncoding(
        OptimizedActiveResidentRematerializationSelectedFormEncodingError,
    ),
    ActiveResidentRematerializationLayout(
        OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError,
    ),
    ActiveResidentRematerializationFunctionRelative(
        OptimizedActiveResidentRematerializationFunctionRelativeRealizationError,
    ),
    Aarch64MovnResolvedLayout(OptimizedAarch64MovnResolvedSelectedFormLayoutError),
    UnsupportedPhysicalPhaseComposition,
    FunctionRelativeRealization(FunctionRelativeOptimizationRealizationError),
}

#[inline(never)]
fn stage_active_resident_rematerialization_pipeline(
    ranges: StagedOptimizedLiveRanges,
) -> Result<
    Box<StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization>,
    OptimizedVerifiedPhysicalPipelineError,
> {
    let budget = ranges
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .budget_per_pass();
    let legality = stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1(ranges)
        .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
    let rematerialization = stage_optimized_active_resident_rematerialization(
        legality,
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        budget,
    )
    .map_err(OptimizedVerifiedPhysicalPipelineError::ActiveResidentRematerialization)?;
    let machine =
        stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization(
            &rematerialization,
        )
        .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
    let encoding = stage_optimized_active_resident_rematerialization_selected_form_encoding(
        rematerialization,
        machine,
    )
    .map_err(OptimizedVerifiedPhysicalPipelineError::ActiveResidentRematerializationEncoding)?;
    let layout = stage_optimized_active_resident_rematerialization_resolved_selected_form_layout(
        encoding,
    )
    .map_err(OptimizedVerifiedPhysicalPipelineError::ActiveResidentRematerializationLayout)?;
    stage_optimized_active_resident_rematerialization_function_relative_realization(layout)
        .map(Box::new)
        .map_err(
            OptimizedVerifiedPhysicalPipelineError::ActiveResidentRematerializationFunctionRelative,
        )
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
    settlements: &[AdmittedBoundarySettlement<'_>],
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    let optimized_target = lower_optimized_to_target_operations_with_provider_executions(
        optimized,
        target,
        settlements,
    )
    .map_err(OptimizedVerifiedPhysicalPipelineError::TargetLowering)?;
    stage_optimized_verified_physical_pipeline(optimized_target)
}

/// Lower and validate one optimized plan while retaining the exact opaque
/// provider installation that authorized its installed-provider calls. The
/// admission remains owned by the nested target carrier throughout every
/// selected and physical stage.
pub fn stage_optimized_verified_physical_pipeline_with_provider_executions_and_installation(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
    installation: AdmittedProviderInstallation,
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    let optimized_target =
        lower_optimized_to_target_operations_with_provider_executions_and_installation(
            optimized,
            target,
            settlements,
            installation,
        )
        .map_err(OptimizedVerifiedPhysicalPipelineError::TargetLowering)?;
    stage_optimized_verified_physical_pipeline(optimized_target)
}

fn stage_optimized_verified_physical_pipeline(
    optimized_target: ValidatedOptimizedTargetOperations,
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    let allocation_recovery = optimized_target
        .optimized()
        .selections()
        .for_phase(OptimizationExecutionPhase::AllocationRecovery);
    if allocation_recovery.as_slice()
        == [Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1]
    {
        if !optimized_target
            .optimized()
            .selections()
            .for_phase(OptimizationExecutionPhase::SelectedLowering)
            .is_empty()
            || !optimized_target
                .optimized()
                .selections()
                .for_phase(OptimizationExecutionPhase::FunctionRelativeLayout)
                .is_empty()
            || !optimized_target
                .optimized()
                .selections()
                .for_phase(OptimizationExecutionPhase::PostAllocationMachine)
                .is_empty()
        {
            return Err(
                OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition,
            );
        }
        let ranges = stage_active_resident_rematerialization_live_ranges(optimized_target)?;
        let realization = stage_active_resident_rematerialization_pipeline(ranges)?;
        return Ok(
            StagedOptimizedVerifiedPhysicalPipeline::ActiveResidentRematerialization {
                realization,
            },
        );
    }
    stage_non_active_resident_rematerialization_physical_pipeline(optimized_target)
}

#[inline(never)]
fn stage_active_resident_rematerialization_live_ranges(
    optimized_target: ValidatedOptimizedTargetOperations,
) -> Result<StagedOptimizedLiveRanges, OptimizedVerifiedPhysicalPipelineError> {
    let selected = stage_optimized_instruction_selection(optimized_target)
        .map_err(OptimizedVerifiedPhysicalPipelineError::Selection)?;
    let liveness = stage_optimized_liveness(selected)
        .map_err(OptimizedVerifiedPhysicalPipelineError::Liveness)?;
    stage_optimized_live_ranges(liveness)
        .map_err(OptimizedVerifiedPhysicalPipelineError::LiveRanges)
}

#[inline(never)]
fn stage_non_active_resident_rematerialization_physical_pipeline(
    optimized_target: ValidatedOptimizedTargetOperations,
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    let selected = stage_optimized_instruction_selection(optimized_target)
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
        match allocation_recovery.as_slice() {
            [Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1] => {
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
            _ => {
                return Err(
                    OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition,
                );
            }
        }
    }

    if !post_allocation_machine.is_empty() {
        if !function_relative_layout.is_empty() {
            return Err(
                OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition,
            );
        }
        let post_allocation_optimization = match post_allocation_machine.as_slice() {
            [optimization @ Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1]
            | [optimization @ Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1] => {
                *optimization
            }
            _ => {
                return Err(
                    OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition,
                );
            }
        };
        if selected_lowering.is_empty() {
            let legality = stage_optimized_allocation_legality(ranges)
                .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
            let homes = stage_optimized_register_homes(legality)
                .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterHomes)?;
            let machine = stage_optimized_post_allocation_machine_plan(&homes)
                .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
            return match post_allocation_optimization {
                Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 => {
                    let optimization = stage_optimized_aarch64_cbnz_fusion(&homes, &machine)
                        .map_err(
                        OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineOptimization,
                    )?;
                    let realization = stage_aarch64_cbnz_function_relative_realization(
                        homes,
                        machine,
                        optimization,
                    )
                    .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)?;
                    Ok(
                        StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine {
                            realization,
                        },
                    )
                }
                Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1 => {
                    let materialization = stage_optimized_aarch64_movn_materialization(
                        &homes, &machine,
                    )
                    .map_err(
                        OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineOptimization,
                    )?;
                    let layout = stage_optimized_aarch64_movn_resolved_selected_form_layout(
                        homes,
                        machine,
                        materialization,
                    )
                    .map_err(OptimizedVerifiedPhysicalPipelineError::Aarch64MovnResolvedLayout)?;
                    Ok(
                        StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachineMovn {
                            layout,
                        },
                    )
                }
                _ => unreachable!("post-allocation singleton was closed above"),
            };
        }
        let legality = stage_optimized_allocation_legality_for_frameless_leaf(ranges)
            .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
        let run = run_selected_lowering_optimizations(legality)
            .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedLowering)?;
        let homes = stage_optimized_register_homes_after_selected_lowering(run)
            .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedLoweringHomes)?;
        let machine = stage_optimized_post_allocation_machine_plan_after_selected_lowering(&homes)
            .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
        return match post_allocation_optimization {
            Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 => {
                let optimization =
                    stage_optimized_aarch64_cbnz_fusion_after_selected_lowering(&homes, &machine)
                        .map_err(
                        OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineOptimization,
                    )?;
                let realization =
                    stage_selected_lowering_aarch64_cbnz_function_relative_realization(
                        homes,
                        machine,
                        optimization,
                    )
                    .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)?;
                Ok(
                    StagedOptimizedVerifiedPhysicalPipeline::SelectedLoweringPostAllocationMachine {
                        realization,
                    },
                )
            }
            Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1 => {
                let materialization =
                    stage_optimized_aarch64_movn_materialization_after_selected_lowering(
                        &homes, &machine,
                    )
                    .map_err(
                        OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineOptimization,
                    )?;
                let layout = stage_selected_lowering_aarch64_movn_resolved_selected_form_layout(
                    homes,
                    machine,
                    materialization,
                )
                .map_err(OptimizedVerifiedPhysicalPipelineError::Aarch64MovnResolvedLayout)?;
                Ok(
                    StagedOptimizedVerifiedPhysicalPipeline::SelectedLoweringPostAllocationMachineMovn {
                        layout,
                    },
                )
            }
            _ => unreachable!("post-allocation singleton was closed above"),
        };
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
