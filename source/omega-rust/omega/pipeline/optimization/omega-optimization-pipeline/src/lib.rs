#![forbid(unsafe_code)]

//! Fail-closed optimized-native realization.
//!
//! The ordinary empty-selection compiler path does not call this crate. This
//! entry point begins with the verified Terminal-Psi artifact boundary, runs
//! every selected named pass under the same explicit per-pass work ceiling,
//! and returns only the custody-preserving optimized abstract-plan carrier.

use omega_optimization_core::{OptimizationSelections, OptimizationWorkBudget};
use omega_optimization_run_to_abstract_operations::{
    OptimizedAbstractProjectionError, ValidatedOptimizedAbstractPlan, project_optimization_run,
};
use omega_psi_optimizer::{OptimizationRunError, run_psi_pipeline};
use omega_psi_to_abstract_operations::{
    ArtifactLoweringError, VerifiedPsiOptimizationInput, VerifiedPsiOptimizationUnitBuildError,
    build_verified_psi_optimization_unit, lower_artifact_sections_for_optimization,
};
use psi_proof_admission::AdmissionProfile;

#[path = "stages/realization/active_resident_function_relative_realization.rs"]
mod active_resident_function_relative_realization;
#[path = "stages/machine/active_resident_rematerialization.rs"]
mod active_resident_rematerialization;
#[path = "stages/layout/active_resident_resolved_selected_form_layout.rs"]
mod active_resident_resolved_selected_form_layout;
#[path = "stages/encoding/active_resident_selected_form_encoding.rs"]
mod active_resident_selected_form_encoding;
#[path = "stages/allocation/allocation_legality.rs"]
mod allocation_legality;
#[path = "stages/selection/assignment.rs"]
mod assignment;
#[path = "stages/allocation/fixed_view_copies.rs"]
mod fixed_view_copies;
#[path = "stages/artifacts/function_fragment_emission.rs"]
mod function_fragment_emission;
#[path = "stages/artifacts/function_fragment_object_container.rs"]
mod function_fragment_object_container;
#[path = "stages/artifacts/function_fragment_text_section.rs"]
mod function_fragment_text_section;
#[path = "stages/realization/function_relative_realization.rs"]
mod function_relative_realization;
#[path = "stages/machine/literal_fold_homes.rs"]
mod literal_fold_homes;
#[path = "stages/machine/literal_folds.rs"]
mod literal_folds;
#[path = "stages/allocation/live_ranges.rs"]
mod live_ranges;
#[path = "stages/allocation/liveness.rs"]
mod liveness;
#[path = "stages/machine/machine_effects.rs"]
mod machine_effects;
#[path = "coordination/native_continuation.rs"]
mod native_continuation;
#[path = "stages/artifacts/object_artifact.rs"]
mod object_artifact;
#[path = "stages/selection/optimized_target_operations.rs"]
mod optimized_target_operations;
#[path = "stages/realization/ordinary_callable_entry.rs"]
mod ordinary_callable_entry;
#[path = "coordination/physical_pipeline.rs"]
mod physical_pipeline;
#[path = "stages/machine/post_allocation_machine_effects.rs"]
mod post_allocation_machine_effects;
#[path = "stages/machine/post_allocation_machine_optimizations.rs"]
mod post_allocation_machine_optimizations;
#[path = "stages/encoding/post_allocation_selected_form_encoding.rs"]
mod post_allocation_selected_form_encoding;
#[path = "stages/allocation/register_environment.rs"]
mod register_environment;
#[path = "stages/allocation/register_homes.rs"]
mod register_homes;
#[path = "coordination/report.rs"]
mod report;
#[path = "stages/layout/resolved_selected_form_layout.rs"]
mod resolved_selected_form_layout;
#[path = "stages/allocation/selected_reanalysis.rs"]
mod selected_reanalysis;
#[path = "stages/selection/selection.rs"]
mod selection;
#[path = "stages/realization/structural_unit_function_relative_realization.rs"]
mod structural_unit_function_relative_realization;
#[path = "stages/realization/unit_function_relative_realization.rs"]
mod unit_function_relative_realization;
#[path = "stages/layout/whole_function_exit_contract.rs"]
mod whole_function_exit_contract;
#[path = "stages/layout/x86_branch_relaxation.rs"]
mod x86_branch_relaxation;

pub use active_resident_function_relative_realization::{
    OptimizedActiveResidentRematerializationFunctionRelativeRealizationError,
    StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
    StagedOptimizedActiveResidentRematerializationFunctionRelativeRealizationCustodyReceipt,
    stage_optimized_active_resident_rematerialization_function_relative_realization,
    validate_optimized_active_resident_rematerialization_function_relative_realization,
};
pub use active_resident_rematerialization::{
    OptimizedActiveResidentRematerializationError, StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    stage_optimized_active_resident_rematerialization,
    validate_optimized_active_resident_rematerialization,
};
pub use active_resident_resolved_selected_form_layout::{
    OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError,
    StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
    StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt,
    stage_optimized_active_resident_rematerialization_resolved_selected_form_layout,
    validate_optimized_active_resident_rematerialization_resolved_selected_form_layout,
};
pub use active_resident_selected_form_encoding::{
    OptimizedActiveResidentRematerializationSelectedFormEncodingError,
    StagedOptimizedActiveResidentRematerializationSelectedFormEncoding,
    StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt,
    stage_optimized_active_resident_rematerialization_selected_form_encoding,
    validate_optimized_active_resident_rematerialization_selected_form_encoding,
};
pub use allocation_legality::{
    OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegality,
    StagedOptimizedAllocationLegalityCustodyReceipt, stage_optimized_allocation_legality,
    stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1,
    stage_optimized_allocation_legality_for_frameless_leaf,
    stage_optimized_allocation_legality_with_availability,
    validate_optimized_allocation_legality_custody,
};
pub use assignment::{
    OptimizedAssignmentCustodyError, OptimizedAssignmentPipelineError,
    StagedOptimizedAssignedOperations, StagedOptimizedAssignmentCustodyReceipt,
};
#[cfg(test)]
use assignment::{stage_optimized_assignment, validate_optimized_assignment_custody};
pub use fixed_view_copies::{
    OptimizedFixedViewCopyCustodyError, StagedOptimizedFixedViewCopies,
    StagedOptimizedFixedViewCopyCustodyReceipt, stage_optimized_fixed_view_copies,
    validate_optimized_fixed_view_copy_custody,
};
pub use function_fragment_emission::{
    FunctionFragmentEmissionError, FunctionFragmentEmissionManifest,
    FunctionFragmentEmissionManifestDecodeError, FunctionFragmentEmissionSourceKind,
    FunctionFragmentEmissionStage, FunctionFragmentEmissionStatistics,
    FunctionFragmentEmissionUnavailableData, StagedFunctionFragmentEmissionCustodyReceipt,
    StagedOptimizedFunctionFragmentEmission, StagedOptimizedFunctionFragmentEmissionSource,
    ValidatedFunctionFragmentEmissionManifest, stage_optimized_function_fragment_emission,
    validate_optimized_function_fragment_emission,
};
pub use function_fragment_object_container::{
    FunctionFragmentObjectContainerManifest, FunctionFragmentObjectContainerManifestDecodeError,
    FunctionFragmentObjectContainerStage, FunctionFragmentObjectContainerStatistics,
    FunctionFragmentObjectContainerUnavailableData, RelocationFreeObjectContainerError,
    StagedOptimizedRelocationFreeObjectContainer,
    StagedRelocationFreeObjectContainerCustodyReceipt,
    ValidatedFunctionFragmentObjectContainerManifest,
    stage_optimized_relocation_free_object_container,
    validate_optimized_relocation_free_object_container,
};
pub use function_fragment_text_section::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionManifestDecodeError,
    FunctionFragmentTextSectionStage, FunctionFragmentTextSectionStatistics,
    FunctionFragmentTextSectionUnavailableData, RelocationFreeTextSectionPlacementError,
    StagedOptimizedRelocationFreeTextSection, StagedRelocationFreeTextSectionCustodyReceipt,
    ValidatedFunctionFragmentTextSectionManifest, stage_optimized_relocation_free_text_section,
    validate_optimized_relocation_free_text_section,
};
pub use function_relative_realization::{
    FunctionRelativeOptimizationRealizationError, FunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationManifestDecodeError,
    FunctionRelativeOptimizationRealizationScope, FunctionRelativeOptimizationRealizationStage,
    FunctionRelativeOptimizationRealizationStatistics, FunctionRelativeOptimizationUnavailableData,
    StagedAarch64CbnzFunctionRelativeRealization,
    StagedAarch64CbnzFunctionRelativeRealizationCustodyReceipt,
    StagedFunctionRelativeLayoutOptimizationRealization,
    StagedFunctionRelativeLayoutOptimizationRealizationCustodyReceipt,
    StagedSelectedLoweringAarch64CbnzFunctionRelativeRealization,
    StagedSelectedLoweringAarch64CbnzFunctionRelativeRealizationCustodyReceipt,
    StagedSelectedLoweringFunctionRelativeRealization,
    StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt,
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    stage_aarch64_cbnz_function_relative_realization,
    stage_function_relative_layout_optimization_realization,
    stage_selected_lowering_aarch64_cbnz_function_relative_realization,
    stage_selected_lowering_function_relative_realization,
    validate_aarch64_cbnz_function_relative_realization_custody,
    validate_function_relative_layout_optimization_realization_custody,
    validate_selected_lowering_aarch64_cbnz_function_relative_realization_custody,
    validate_selected_lowering_function_relative_realization_custody,
};
pub use literal_fold_homes::{
    OptimizedPostLiteralFoldHomeCustodyError, OptimizedPostSelectedLoweringHomeCustodyError,
    StagedOptimizedPostLiteralFoldHomeCustodyReceipt,
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    StagedOptimizedRegisterHomesAfterLiteralFolds,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
    stage_optimized_register_homes_after_literal_folds,
    stage_optimized_register_homes_after_selected_lowering,
    validate_optimized_register_home_after_literal_fold_custody,
    validate_optimized_register_home_after_selected_lowering_custody,
};
pub use literal_folds::{
    OptimizedLiteralFoldCustodyError, SelectedLoweringOptimizationSchedule,
    StagedOptimizedLiteralFoldAttempt, StagedOptimizedLiteralFoldAttemptReceipt,
    StagedOptimizedLiteralFoldCustodyReceipt, StagedOptimizedLiteralFoldIterationReceipt,
    StagedOptimizedLiteralFoldStep, StagedOptimizedLiteralFolds,
    StagedSelectedLoweringOptimizationCustodyReceipt, StagedSelectedLoweringOptimizationRun,
    run_selected_lowering_optimizations, stage_first_optimized_literal_fold,
    stage_next_optimized_literal_fold, validate_optimized_literal_fold_custody,
    validate_selected_lowering_optimization_custody,
};
pub use live_ranges::{
    OptimizedLiveRangeCustodyError, StagedOptimizedLiveRangeCustodyReceipt,
    StagedOptimizedLiveRanges, stage_optimized_live_ranges, validate_optimized_live_range_custody,
};
pub use liveness::{
    OptimizedLivenessCustodyError, StagedOptimizedLiveness, StagedOptimizedLivenessCustodyReceipt,
    stage_optimized_liveness, validate_optimized_liveness_custody,
};
pub use machine_effects::{
    OptimizedMachineEffectPipelineError, StagedOptimizedMachineEffectCustodyReceipt,
    StagedOptimizedMachineEffectSourceCustodyReceipt, StagedOptimizedMachineEffects,
    stage_optimized_machine_effects,
    stage_optimized_machine_effects_after_active_resident_rematerialization,
    stage_optimized_machine_effects_after_fixed_view_copies,
    stage_optimized_machine_effects_after_literal_folds,
    stage_optimized_machine_effects_after_selected_lowering,
    validate_optimized_machine_effect_custody,
    validate_optimized_machine_effect_custody_after_active_resident_rematerialization,
    validate_optimized_machine_effect_custody_after_fixed_view_copies,
    validate_optimized_machine_effect_custody_after_literal_folds,
    validate_optimized_machine_effect_custody_after_selected_lowering,
};
pub use native_continuation::{
    OptimizedNativeContinuationError, StagedOptimizedNativeContinuation,
    stage_optimized_native_continuation_with_provider_executions,
    stage_optimized_native_continuation_with_provider_executions_and_installation,
};
pub use object_artifact::{
    OptimizedObjectArtifactCustodyReceipt, OptimizedObjectArtifactError,
    OptimizedObjectArtifactManifest, OptimizedObjectArtifactManifestDecodeError,
    OptimizedObjectArtifactRecord, OptimizedObjectArtifactRecordDecodeError,
    OptimizedObjectArtifactStage, OptimizedObjectArtifactStatistics,
    OptimizedObjectArtifactUnavailableData, StagedValidatedOptimizedObjectArtifact,
    ValidatedOptimizedObjectArtifactManifest, stage_validated_optimized_object_artifact,
    validate_optimized_object_artifact,
};
pub use optimized_target_operations::{
    ValidatedOptimizedTargetOperations, lower_optimized_to_target_operations,
    lower_optimized_to_target_operations_with_provider_executions,
    lower_optimized_to_target_operations_with_provider_executions_and_installation,
};
pub use ordinary_callable_entry::{
    OptimizedOrdinaryCallableEntryCustodyReceipt, OptimizedOrdinaryCallableEntryDecodeError,
    OptimizedOrdinaryCallableEntryDisposition, OptimizedOrdinaryCallableEntryError,
    OptimizedOrdinaryCallableEntryManifest, OptimizedOrdinaryCallableEntryManifestDecodeError,
    OptimizedOrdinaryCallableEntryRecord, OptimizedOrdinaryCallableEntryStage,
    OptimizedOrdinaryCallableEntryUnavailableData, OptimizedOrdinaryCallableParameter,
    OptimizedOrdinaryCallableResult, OptimizedOrdinaryCallableReturn,
    StagedValidatedOptimizedOrdinaryCallableEntry, ValidatedOptimizedOrdinaryCallableEntryManifest,
    stage_validated_optimized_ordinary_callable_entry, validate_optimized_ordinary_callable_entry,
};
#[cfg(test)]
use physical_pipeline::stage_optimized_verified_physical_pipeline_with_provider_executions;
pub use physical_pipeline::{
    OptimizedVerifiedPhysicalPipelineError, StagedOptimizedVerifiedPhysicalPipeline,
};
pub use post_allocation_machine_effects::{
    OptimizedPostAllocationMachinePipelineError,
    StagedOptimizedPostAllocationMachineCustodyReceipt, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedPostAllocationMachineSourceCustodyReceipt,
    stage_optimized_post_allocation_machine_plan,
    stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization,
    stage_optimized_post_allocation_machine_plan_after_fixed_view_copies,
    stage_optimized_post_allocation_machine_plan_after_literal_folds,
    stage_optimized_post_allocation_machine_plan_after_selected_lowering,
    validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody,
    validate_optimized_post_allocation_machine_plan_after_fixed_view_copy_custody,
    validate_optimized_post_allocation_machine_plan_after_literal_fold_custody,
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody,
    validate_optimized_post_allocation_machine_plan_custody,
};
pub use post_allocation_machine_optimizations::{
    OptimizedPostAllocationMachineOptimizationError, StagedOptimizedAarch64CbnzFusion,
    StagedOptimizedAarch64CbnzFusionCustodyReceipt, StagedOptimizedAarch64MovnMaterialization,
    StagedOptimizedAarch64MovnMaterializationCustodyReceipt, stage_optimized_aarch64_cbnz_fusion,
    stage_optimized_aarch64_cbnz_fusion_after_selected_lowering,
    stage_optimized_aarch64_movn_materialization,
    stage_optimized_aarch64_movn_materialization_after_selected_lowering,
    validate_optimized_aarch64_cbnz_fusion_after_selected_lowering_custody,
    validate_optimized_aarch64_cbnz_fusion_custody,
    validate_optimized_aarch64_movn_materialization_after_selected_lowering_custody,
    validate_optimized_aarch64_movn_materialization_custody,
};
pub use post_allocation_selected_form_encoding::{
    DeferredControlEncodingReason, OptimizedSelectedFormEncodingError,
    SelectedFormDecodedFootprint, SelectedFormEncodingCounts, SelectedFormEncodingIdentity,
    SelectedFormEncodingRow, SelectedFormEncodingState, SelectedFormMachineOptimizationCustody,
    SelectedFormMovnOptimizationCustody, SelectedStructuralUnitCallEncodingRow,
    SelectedStructuralUnitFunctionEncoding, StagedOptimizedSelectedFormEncoding,
    stage_optimized_layout_independent_selected_form_encoding,
    stage_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion,
    stage_optimized_layout_independent_selected_form_encoding_after_aarch64_movn_materialization,
    validate_optimized_layout_independent_selected_form_encoding,
    validate_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion,
    validate_optimized_layout_independent_selected_form_encoding_after_aarch64_movn_materialization,
};
pub use register_environment::{
    TargetRegisterEnvironmentValidationError, ValidatedTargetRegisterEnvironment,
    baseline_target_register_environment, validate_target_register_environment,
    validate_target_register_environment_with_reservations,
};
pub use register_homes::{
    OptimizedPostCopyRegisterHomeCustodyError, OptimizedRegisterHomeCustodyError,
    StagedOptimizedPostCopyRegisterHomeCustodyReceipt, StagedOptimizedRegisterHomeCustodyReceipt,
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterFixedViewCopies,
    stage_optimized_register_homes, stage_optimized_register_homes_after_fixed_view_copies,
    validate_optimized_register_home_after_fixed_view_copy_custody,
    validate_optimized_register_home_custody,
};
pub use report::{
    OptimizationPipelineReport, OptimizationReportRequest, optimization_pipeline_report,
    optimization_pipeline_report_from_object_artifact,
    optimization_pipeline_report_from_ordinary_callable_entry,
};
pub use resolved_selected_form_layout::{
    OptimizedResolvedSelectedFormLayoutError, ResolvedConditionalBranchEvidence,
    ResolvedSelectedBlockLayout, ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFormRow,
    ResolvedSelectedFunctionLayout, ResolvedStructuralUnitCallLayout,
    ResolvedStructuralUnitFunctionLayout, SelectedFunctionLayoutPolicy,
    StagedOptimizedResolvedSelectedFormLayout, stage_optimized_resolved_selected_form_layout,
    stage_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion,
    validate_optimized_resolved_selected_form_layout,
    validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion,
};
pub use selected_reanalysis::{
    OptimizedSelectedReanalysisError, StagedOptimizedSelectedReanalysis,
    StagedOptimizedSelectedReanalysisCustodyReceipt, stage_optimized_selected_reanalysis,
    validate_optimized_selected_reanalysis_custody,
};
pub use selection::{
    OptimizedSelectionCustodyError, OptimizedSelectionPipelineError,
    StagedOptimizedSelectedInstructions, StagedOptimizedSelectionCustodyReceipt,
    stage_optimized_instruction_selection, validate_optimized_selection_custody,
};
pub use structural_unit_function_relative_realization::{
    OptimizedStructuralUnitFunctionRelativeRealizationError,
    StagedOptimizedStructuralUnitFunctionRelativeRealization,
    StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt,
    stage_optimized_structural_unit_function_relative_realization,
    validate_optimized_structural_unit_function_relative_realization,
};
pub use unit_function_relative_realization::{
    OptimizedUnitFunctionRelativeRealizationError, StagedOptimizedUnitFunctionRelativeRealization,
    StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt,
    stage_optimized_unit_function_relative_realization,
    validate_optimized_unit_function_relative_realization,
};
pub use whole_function_exit_contract::{
    ValidatedWholeFunctionExitContract, WholeFunctionEntryAssumption, WholeFunctionExitContract,
    WholeFunctionExitContractError, WholeFunctionExitContractIdentity, WholeFunctionExitEvidence,
    WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy, WholeFunctionHardeningPolicy,
    WholeFunctionReturnEvidence, WholeFunctionReturnMechanism, WholeFunctionReturnValueEvidence,
    WholeFunctionStructuralUnitCallEvidence, WholeFunctionStructuralUnitExitEvidence,
    stage_whole_function_exit_contract,
    stage_whole_function_exit_contract_after_aarch64_cbnz_fusion,
    stage_whole_function_exit_contract_after_x86_branch_relaxation,
    validate_whole_function_exit_contract,
    validate_whole_function_exit_contract_after_aarch64_cbnz_fusion,
    validate_whole_function_exit_contract_after_x86_branch_relaxation,
};
pub use x86_branch_relaxation::{
    OptimizedX86BranchRelaxationError, StagedOptimizedX86BranchRelaxation,
    X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
    X86BranchRelaxationIdentity, X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
    X86BranchRelaxationWorkAxis, stage_optimized_x86_branch_relaxation,
    validate_optimized_x86_branch_relaxation,
};

/// Exact optimizer inputs chosen by the compiler coordinator.
///
/// Construction rejects the empty selection so compatibility builds cannot
/// accidentally enter this crate or manufacture optimizer work records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplicitOptimizationRequest {
    selections: OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
}

impl ExplicitOptimizationRequest {
    pub fn new(
        selections: OptimizationSelections,
        budget_per_pass: OptimizationWorkBudget,
    ) -> Result<Self, EmptyOptimizationSelections> {
        if selections.is_empty() {
            return Err(EmptyOptimizationSelections);
        }
        Ok(Self {
            selections,
            budget_per_pass,
        })
    }

    pub const fn selections(&self) -> &OptimizationSelections {
        &self.selections
    }

    pub const fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.budget_per_pass
    }
}

/// Compiler-owned bounded baseline for the experimental optimized lane.
/// Every value is a per-pass-group ceiling; this is not a source-visible
/// optimization level or an intensity preset.
pub fn compiler_baseline_request_v1(
    selections: &OptimizationSelections,
) -> Result<ExplicitOptimizationRequest, EmptyOptimizationSelections> {
    ExplicitOptimizationRequest::new(
        selections.clone(),
        OptimizationWorkBudget::new(1_000_000, 100_000, 100_000, 100_000, 10_000)
            .expect("compiler baseline optimizer ceilings are nonzero"),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyOptimizationSelections;

impl std::fmt::Display for EmptyOptimizationSelections {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("the explicit optimizer pipeline requires at least one named selection")
    }
}

impl std::error::Error for EmptyOptimizationSelections {}

#[derive(Debug)]
pub enum OptimizationPipelineError {
    ArtifactLowering(ArtifactLoweringError),
    UnitBuild(VerifiedPsiOptimizationUnitBuildError),
    Run(OptimizationRunError),
    AbstractProjection(OptimizedAbstractProjectionError),
}

impl std::fmt::Display for OptimizationPipelineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "explicit optimization pipeline failed: {self:?}")
    }
}

impl std::error::Error for OptimizationPipelineError {}

pub fn optimize_artifact_sections(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &AdmissionProfile,
    request: ExplicitOptimizationRequest,
) -> Result<ValidatedOptimizedAbstractPlan, OptimizationPipelineError> {
    let input = lower_artifact_sections_for_optimization(semantic_bytes, proof_bytes, profile)
        .map_err(OptimizationPipelineError::ArtifactLowering)?;
    optimize_verified_psi_input(input, request)
}

pub fn optimize_verified_psi_input(
    input: VerifiedPsiOptimizationInput,
    request: ExplicitOptimizationRequest,
) -> Result<ValidatedOptimizedAbstractPlan, OptimizationPipelineError> {
    let verified = build_verified_psi_optimization_unit(
        input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .map_err(OptimizationPipelineError::UnitBuild)?;
    let run = run_psi_pipeline(verified, request.selections(), request.budget_per_pass())
        .map_err(OptimizationPipelineError::Run)?;
    project_optimization_run(run).map_err(OptimizationPipelineError::AbstractProjection)
}

#[cfg(test)]
mod tests;
