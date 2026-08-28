#![forbid(unsafe_code)]

//! Fail-closed orchestration for explicit, nonempty optimization selections.
//!
//! The ordinary empty-selection compiler path does not call this crate. This
//! entry point begins with the verified Terminal-Psi artifact boundary, runs
//! every selected named pass under the same explicit per-pass work ceiling,
//! and returns only the custody-preserving optimized abstract-plan carrier.

use omega_lowering_optimizer::{
    OptimizedAbstractProjectionError, ValidatedOptimizedAbstractPlan, project_optimization_run,
};
use omega_optimization_core::{OptimizationSelections, OptimizationWorkBudget};
use omega_psi_optimizer::{OptimizationRunError, run_psi_pipeline};
use omega_terminal_psi_to_abstract_operations::{
    ArtifactLoweringError, VerifiedPsiOptimizationUnitBuildError,
    VerifiedTerminalOptimizationInput, build_verified_psi_optimization_unit,
    lower_artifact_sections_for_optimization,
};
use psi_proof_admission::AdmissionProfile;

mod active_resident_function_relative_realization;
mod active_resident_rematerialization;
mod active_resident_resolved_selected_form_layout;
mod active_resident_selected_form_encoding;
mod allocation_legality;
mod assignment;
mod fixed_view_copies;
mod function_fragment_emission;
mod function_fragment_object_container;
mod function_fragment_text_section;
mod function_relative_realization;
mod literal_fold_homes;
mod literal_folds;
mod live_ranges;
mod liveness;
mod machine_effects;
mod physical_pipeline;
mod post_allocation_machine_effects;
mod post_allocation_machine_optimizations;
mod post_allocation_selected_form_encoding;
mod register_environment;
mod register_homes;
mod report;
mod resolved_selected_form_layout;
mod selected_reanalysis;
mod selection;
mod terminal_object_artifact;
mod terminal_object_callable_entry;
mod whole_function_exit_contract;
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
    stage_optimized_assignment, stage_optimized_assignment_with_provider_executions,
    validate_optimized_assignment_custody,
};
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
    FunctionFragmentObjectContainerUnavailableData, RelocationFreeTerminalObjectContainerError,
    StagedOptimizedRelocationFreeTerminalObjectContainer,
    StagedRelocationFreeTerminalObjectContainerCustodyReceipt,
    ValidatedFunctionFragmentObjectContainerManifest,
    stage_optimized_relocation_free_terminal_object_container,
    validate_optimized_relocation_free_terminal_object_container,
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
pub use physical_pipeline::{
    OptimizedVerifiedPhysicalPipelineError, StagedOptimizedVerifiedPhysicalPipeline,
    stage_optimized_verified_physical_pipeline_with_provider_executions,
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
    StagedOptimizedAarch64CbnzFusionCustodyReceipt, stage_optimized_aarch64_cbnz_fusion,
    stage_optimized_aarch64_cbnz_fusion_after_selected_lowering,
    validate_optimized_aarch64_cbnz_fusion_after_selected_lowering_custody,
    validate_optimized_aarch64_cbnz_fusion_custody,
};
pub use post_allocation_selected_form_encoding::{
    DeferredTerminalControlEncodingReason, OptimizedSelectedFormEncodingError,
    StagedOptimizedSelectedFormEncoding, TerminalSelectedFormDecodedFootprint,
    TerminalSelectedFormEncodingIdentity, TerminalSelectedFormEncodingRow,
    TerminalSelectedFormEncodingState, TerminalSelectedFormMachineOptimizationCustody,
    stage_optimized_layout_independent_selected_form_encoding,
    stage_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion,
    validate_optimized_layout_independent_selected_form_encoding,
    validate_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion,
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
    optimization_pipeline_report_from_terminal_object_artifact,
    optimization_pipeline_report_from_terminal_ordinary_callable_entry,
};
pub use resolved_selected_form_layout::{
    OptimizedResolvedSelectedFormLayoutError, StagedOptimizedResolvedSelectedFormLayout,
    TerminalResolvedConditionalBranchEvidence, TerminalResolvedSelectedBlockLayout,
    TerminalResolvedSelectedFormLayoutIdentity, TerminalResolvedSelectedFormRow,
    TerminalResolvedSelectedFunctionLayout, TerminalSelectedFunctionLayoutPolicy,
    stage_optimized_resolved_selected_form_layout,
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
pub use terminal_object_artifact::{
    OptimizedTerminalObjectArtifactCustodyReceipt, OptimizedTerminalObjectArtifactError,
    OptimizedTerminalObjectArtifactManifest, OptimizedTerminalObjectArtifactManifestDecodeError,
    OptimizedTerminalObjectArtifactRecord, OptimizedTerminalObjectArtifactRecordDecodeError,
    OptimizedTerminalObjectArtifactStage, OptimizedTerminalObjectArtifactStatistics,
    OptimizedTerminalObjectArtifactUnavailableData, StagedValidatedOptimizedTerminalObjectArtifact,
    ValidatedOptimizedTerminalObjectArtifactManifest,
    stage_validated_optimized_terminal_object_artifact,
    validate_optimized_terminal_object_artifact,
};
pub use terminal_object_callable_entry::{
    OptimizedTerminalOrdinaryCallableEntryCustodyReceipt,
    OptimizedTerminalOrdinaryCallableEntryDecodeError,
    OptimizedTerminalOrdinaryCallableEntryDisposition, OptimizedTerminalOrdinaryCallableEntryError,
    OptimizedTerminalOrdinaryCallableEntryManifest,
    OptimizedTerminalOrdinaryCallableEntryManifestDecodeError,
    OptimizedTerminalOrdinaryCallableEntryRecord, OptimizedTerminalOrdinaryCallableEntryStage,
    OptimizedTerminalOrdinaryCallableEntryUnavailableData,
    OptimizedTerminalOrdinaryCallableParameter, OptimizedTerminalOrdinaryCallableResult,
    OptimizedTerminalOrdinaryCallableReturn, StagedValidatedOptimizedTerminalOrdinaryCallableEntry,
    ValidatedOptimizedTerminalOrdinaryCallableEntryManifest,
    stage_validated_optimized_terminal_ordinary_callable_entry,
    validate_optimized_terminal_ordinary_callable_entry,
};
pub use whole_function_exit_contract::{
    TerminalWholeFunctionEntryAssumption, TerminalWholeFunctionExitContract,
    TerminalWholeFunctionExitContractError, TerminalWholeFunctionExitContractIdentity,
    TerminalWholeFunctionExitEvidence, TerminalWholeFunctionExitLayoutCustody,
    TerminalWholeFunctionExitPolicy, TerminalWholeFunctionHardeningPolicy,
    TerminalWholeFunctionReturnEvidence, TerminalWholeFunctionReturnMechanism,
    ValidatedTerminalWholeFunctionExitContract, stage_terminal_whole_function_exit_contract,
    stage_terminal_whole_function_exit_contract_after_aarch64_cbnz_fusion,
    stage_terminal_whole_function_exit_contract_after_x86_branch_relaxation,
    validate_terminal_whole_function_exit_contract,
    validate_terminal_whole_function_exit_contract_after_aarch64_cbnz_fusion,
    validate_terminal_whole_function_exit_contract_after_x86_branch_relaxation,
};
pub use x86_branch_relaxation::{
    OptimizedX86BranchRelaxationError, StagedOptimizedX86BranchRelaxation,
    TerminalX86BranchRelaxationAction, TerminalX86BranchRelaxationAttempt,
    TerminalX86BranchRelaxationAttemptOutcome, TerminalX86BranchRelaxationIdentity,
    TerminalX86BranchRelaxationPolicy, TerminalX86BranchRelaxationRevisionIdentity,
    TerminalX86BranchRelaxationWorkAxis, stage_optimized_x86_branch_relaxation,
    validate_optimized_x86_branch_relaxation,
};

/// Exact optimizer inputs chosen by compiler orchestration.
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
    optimize_verified_terminal_input(input, request)
}

pub fn optimize_verified_terminal_input(
    input: VerifiedTerminalOptimizationInput,
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
mod tests {
    use std::collections::BTreeSet;

    use omega_optimization_core::{
        Optimization, OptimizationSelections, OptimizationWorkBudget, OptimizationWorkUsage,
    };
    use omega_optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
    use omega_psi_optimizer::OptimizationRunError;
    use omega_regalloc::{
        PostAllocationOptimizationManifest, PostAllocationOptimizationManifestError,
        PostAllocationSelectedTransformation, TerminalAllocationLegalityError,
        TerminalAllocatorAvailabilityError, TerminalAllocatorAvailabilityPolicy,
        TerminalArchitecturalUnitActionKind, TerminalFixedViewCopyError,
        TerminalFixedViewCopyPolicy, TerminalLiteralFoldPlan, TerminalLiteralFoldPolicy,
        TerminalLiveRangeError, TerminalLiveRangeFragment, TerminalLiveRangePoint,
        TerminalLivenessError, TerminalPressureRematerializationError,
        TerminalPressureRematerializationPolicy, TerminalRecoveryClassification,
        TerminalRecoveryClassificationPolicy, TerminalRecoveryVictimRole,
        TerminalRegisterHomeError, TerminalRegisterHomePlan, TerminalSpillChoicePolicy,
        TerminalVirtualFixedConstraintSite, TerminalVirtualInterference,
        analyze_terminal_allocation_legality, analyze_terminal_live_ranges,
        analyze_terminal_liveness, choose_terminal_spill_victims,
        classify_terminal_pressure_recovery, fold_terminal_selected_incoming_literal,
        materialize_terminal_allocator_availability, terminal_allocation_legality_identity,
        terminal_fixed_view_copy_identity, terminal_live_range_identity,
        terminal_liveness_identity, terminal_register_home_identity,
        validate_post_allocation_optimization_manifest, validate_terminal_allocation_legality,
        validate_terminal_allocator_availability, validate_terminal_fixed_view_copies,
        validate_terminal_literal_fold, validate_terminal_live_ranges, validate_terminal_liveness,
        validate_terminal_register_homes,
    };
    use omega_register_model::{
        RegisterOperandAccess, RegisterReservationProfile, RegisterUnitId, RegisterViewId,
        target_register_environment_identity, validate_register_reservation_profile,
    };
    use omega_target::NativeTarget;
    use omega_terminal_abstract_operations::{TerminalAbstractOperation, TerminalValueBinding};
    use omega_terminal_legalized_operations::{
        TerminalLegalizationRecipe, TerminalLegalizationTheorem, TerminalLegalizedLeafValue,
        TerminalLegalizedTemporaryId, terminal_legalized_operation_plan_identity,
    };
    use omega_terminal_selected_instructions::{
        TerminalMachineBarrier, TerminalSelectedInstructionKind, TerminalSelectedTerminator,
        TerminalVirtualRegisterId, TerminalVirtualRegisterOrigin,
    };
    use omega_terminal_target_operations::{
        TerminalTargetIntegerControl, TerminalTargetIntegerExpression, TerminalTargetOperation,
    };
    use omega_terminal_target_operations_to_selected_instructions::{
        SelectedInstructionError, TerminalLegalizationError,
        terminal_legalization_validator_identity, terminal_selected_instruction_plan_identity,
        validate_terminal_legalized_operations, validate_terminal_selected_instructions,
    };
    use psi_core::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        ObligationId, OperationId, ScalarType, ValueId,
    };
    use psi_proof_admission::{EvidenceRoute, PrimitiveJudgment};
    use psi_terminal::{
        Block, MachineContract, Operation, OperationKind, OperationResult, SuccessorEdge,
        TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
        VocabularyMarker,
    };
    use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

    use super::*;

    fn budget() -> OptimizationWorkBudget {
        OptimizationWorkBudget::new(128, 128, 128, 128, 16).unwrap()
    }

    fn selected_lowering_budget() -> OptimizationWorkBudget {
        OptimizationWorkBudget::new(10_000, 10_000, 100_000, 10_000, 64).unwrap()
    }

    fn canonical_terminal_artifact(
        semantic: &[u8],
        proof: &[u8],
    ) -> psi_terminal_codec::CanonicalTerminalArtifact {
        let module = psi_terminal_codec::decode_module(semantic).unwrap();
        let proof = psi_terminal_codec::decode_proof_bundle(proof).unwrap();
        psi_terminal_codec::CanonicalTerminalArtifact::from_parts(&module, &proof, None).unwrap()
    }

    fn artifact() -> (Vec<u8>, Vec<u8>) {
        let machine = MachineId::new(2_001).unwrap();
        let entry = BlockId::new(2_002).unwrap();
        let exit = BlockId::new(2_003).unwrap();
        let left = ValueId::new(2_004).unwrap();
        let right = ValueId::new(2_005).unwrap();
        let computed = ValueId::new(2_006).unwrap();
        let forwarded = ValueId::new(2_007).unwrap();
        let also_forwarded = ValueId::new(2_016).unwrap();
        let result = ValueId::new(2_008).unwrap();
        let obligation = ObligationId::new(2_009).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        let declaration = |id| ValueDeclaration { id, scalar_type };
        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            proof_output_calls: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine,
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Scalar(declaration(result)),
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry,
                blocks: vec![
                    Block {
                        id: entry,
                        parameters: Vec::new(),
                        operations: vec![
                            Operation {
                                id: OperationId::new(2_010).unwrap(),
                                result: OperationResult::Scalar(declaration(left)),
                                kind: OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(7),
                                },
                            },
                            Operation {
                                id: OperationId::new(2_011).unwrap(),
                                result: OperationResult::Scalar(declaration(right)),
                                kind: OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(8),
                                },
                            },
                            Operation {
                                id: OperationId::new(2_012).unwrap(),
                                result: OperationResult::Scalar(declaration(computed)),
                                kind: OperationKind::ExactIntegerAdd {
                                    left,
                                    right,
                                    obligation,
                                },
                            },
                        ],
                        terminator: Terminator::Jump {
                            edge: EdgeId::new(2_013).unwrap(),
                            target: exit,
                            arguments: vec![computed, computed],
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    Block {
                        id: exit,
                        parameters: vec![declaration(forwarded), declaration(also_forwarded)],
                        operations: Vec::new(),
                        terminator: Terminator::Return {
                            cleanup_actions: Vec::new(),
                            edge: EdgeId::new(2_014).unwrap(),
                            value: forwarded,
                        },
                    },
                ],
                contract: MachineContract {
                    id: ContractId::new(2_015).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let proof = ProofBundle {
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            }],
        };
        (
            psi_terminal_codec::encode_module(&module).unwrap(),
            psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
        )
    }

    fn conditional_immediate_artifact() -> (Vec<u8>, Vec<u8>) {
        conditional_immediate_artifact_with_type(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        )
    }

    fn conditional_immediate_machine(
        base: u64,
        integer_type: IntegerType,
        literals: [u128; 2],
    ) -> TerminalMachine {
        let machine = MachineId::new(base + 1).unwrap();
        let entry = BlockId::new(base + 2).unwrap();
        let when_true = BlockId::new(base + 3).unwrap();
        let when_false = BlockId::new(base + 4).unwrap();
        let condition = ValueId::new(base + 5).unwrap();
        let true_value = ValueId::new(base + 6).unwrap();
        let false_value = ValueId::new(base + 7).unwrap();
        let result = ValueId::new(base + 8).unwrap();
        let scalar_type = ScalarType::Integer(integer_type);
        let declaration = |id, scalar_type| ValueDeclaration { id, scalar_type };
        TerminalMachine {
            id: machine,
            attachment: None,
            parameters: vec![declaration(condition, ScalarType::Boolean)],
            structural_parameters: Vec::new(),
            result: TerminalMachineResult::Scalar(declaration(result, scalar_type)),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks: vec![
                Block {
                    id: entry,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            edge: EdgeId::new(base + 11).unwrap(),
                            target: when_true,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: EdgeId::new(base + 12).unwrap(),
                            target: when_false,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: when_true,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(base + 9).unwrap(),
                        result: OperationResult::Scalar(declaration(true_value, scalar_type)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(literals[0]),
                        },
                    }],
                    terminator: Terminator::Return {
                        edge: EdgeId::new(base + 13).unwrap(),
                        value: true_value,
                        cleanup_actions: Vec::new(),
                    },
                },
                Block {
                    id: when_false,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(base + 10).unwrap(),
                        result: OperationResult::Scalar(declaration(false_value, scalar_type)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(literals[1]),
                        },
                    }],
                    terminator: Terminator::Return {
                        edge: EdgeId::new(base + 14).unwrap(),
                        value: false_value,
                        cleanup_actions: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(base + 15).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }
    }

    fn conditional_immediate_module(
        entry: MachineId,
        machines: Vec<TerminalMachine>,
    ) -> TerminalModule {
        TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            proof_output_calls: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines,
        }
    }

    fn conditional_immediate_artifact_with_type(integer_type: IntegerType) -> (Vec<u8>, Vec<u8>) {
        let machine = conditional_immediate_machine(3_000, integer_type, [7, 9]);
        let module = conditional_immediate_module(machine.id, vec![machine]);
        let proof = ProofBundle {
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        };
        (
            psi_terminal_codec::encode_module(&module).unwrap(),
            psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
        )
    }

    fn disconnected_conditional_artifact() -> (Vec<u8>, Vec<u8>) {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let entry = conditional_immediate_machine(16_000, integer_type, [7, 9]);
        let detached = conditional_immediate_machine(17_000, integer_type, [11, 13]);
        let module = conditional_immediate_module(entry.id, vec![entry, detached]);
        let proof = ProofBundle {
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        };
        (
            psi_terminal_codec::encode_module(&module).unwrap(),
            psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
        )
    }

    fn staged_conditional(target: NativeTarget) -> StagedOptimizedSelectedInstructions {
        let (semantic, proof) = conditional_immediate_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap();
        let target =
            omega_lowering_optimizer::lower_optimized_to_target_operations(optimized, target)
                .unwrap();
        stage_optimized_instruction_selection(target).unwrap()
    }

    fn conditional_forwarded_parameter_artifact() -> (Vec<u8>, Vec<u8>) {
        let machine = MachineId::new(4_001).unwrap();
        let entry = BlockId::new(4_002).unwrap();
        let when_true = BlockId::new(4_003).unwrap();
        let when_false = BlockId::new(4_004).unwrap();
        let condition = ValueId::new(4_005).unwrap();
        let forwarded = ValueId::new(4_006).unwrap();
        let result = ValueId::new(4_007).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let declaration = |id, scalar_type| ValueDeclaration { id, scalar_type };
        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            proof_output_calls: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine,
                attachment: None,
                parameters: vec![
                    declaration(condition, ScalarType::Boolean),
                    declaration(forwarded, scalar_type),
                ],
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Scalar(declaration(result, scalar_type)),
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry,
                blocks: vec![
                    Block {
                        id: entry,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Conditional {
                            condition,
                            when_true: SuccessorEdge {
                                edge: EdgeId::new(4_011).unwrap(),
                                target: when_true,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: SuccessorEdge {
                                edge: EdgeId::new(4_012).unwrap(),
                                target: when_false,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                    },
                    Block {
                        id: when_true,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Return {
                            edge: EdgeId::new(4_013).unwrap(),
                            value: forwarded,
                            cleanup_actions: Vec::new(),
                        },
                    },
                    Block {
                        id: when_false,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Return {
                            edge: EdgeId::new(4_014).unwrap(),
                            value: forwarded,
                            cleanup_actions: Vec::new(),
                        },
                    },
                ],
                contract: MachineContract {
                    id: ContractId::new(4_015).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let proof = ProofBundle {
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        };
        (
            psi_terminal_codec::encode_module(&module).unwrap(),
            psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
        )
    }

    fn constant_conditional_prune_artifact() -> (Vec<u8>, Vec<u8>) {
        let machine = MachineId::new(4_101).unwrap();
        let entry = BlockId::new(4_102).unwrap();
        let when_true = BlockId::new(4_103).unwrap();
        let when_false = BlockId::new(4_104).unwrap();
        let condition = ValueId::new(4_105).unwrap();
        let forwarded = ValueId::new(4_106).unwrap();
        let result = ValueId::new(4_107).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let declaration = |id, scalar_type| ValueDeclaration { id, scalar_type };
        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            proof_output_calls: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine,
                attachment: None,
                parameters: vec![declaration(forwarded, scalar_type)],
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Scalar(declaration(result, scalar_type)),
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry,
                blocks: vec![
                    Block {
                        id: entry,
                        parameters: Vec::new(),
                        operations: vec![Operation {
                            id: OperationId::new(4_108).unwrap(),
                            result: OperationResult::Scalar(declaration(
                                condition,
                                ScalarType::Boolean,
                            )),
                            kind: OperationKind::BooleanConstant { value: true },
                        }],
                        terminator: Terminator::Conditional {
                            condition,
                            when_true: SuccessorEdge {
                                edge: EdgeId::new(4_111).unwrap(),
                                target: when_true,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: SuccessorEdge {
                                edge: EdgeId::new(4_112).unwrap(),
                                target: when_false,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                    },
                    Block {
                        id: when_true,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Return {
                            edge: EdgeId::new(4_113).unwrap(),
                            value: forwarded,
                            cleanup_actions: Vec::new(),
                        },
                    },
                    Block {
                        id: when_false,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Return {
                            edge: EdgeId::new(4_114).unwrap(),
                            value: forwarded,
                            cleanup_actions: Vec::new(),
                        },
                    },
                ],
                contract: MachineContract {
                    id: ContractId::new(4_115).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let proof = ProofBundle {
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        };
        (
            psi_terminal_codec::encode_module(&module).unwrap(),
            psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
        )
    }

    fn linear_empty_block_artifact() -> (Vec<u8>, Vec<u8>) {
        let machine = MachineId::new(4_201).unwrap();
        let entry = BlockId::new(4_202).unwrap();
        let empty = BlockId::new(4_203).unwrap();
        let target = BlockId::new(4_204).unwrap();
        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            proof_output_calls: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine,
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry,
                blocks: vec![
                    Block {
                        id: entry,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Jump {
                            edge: EdgeId::new(4_211).unwrap(),
                            target: empty,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    Block {
                        id: empty,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Jump {
                            edge: EdgeId::new(4_212).unwrap(),
                            target,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    Block {
                        id: target,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::ReturnUnit {
                            edge: EdgeId::new(4_213).unwrap(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                ],
                contract: MachineContract {
                    id: ContractId::new(4_215).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let proof = ProofBundle {
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        };
        (
            psi_terminal_codec::encode_module(&module).unwrap(),
            psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
        )
    }

    fn adjacent_block_merge_artifact() -> (Vec<u8>, Vec<u8>) {
        let machine = MachineId::new(4_251).unwrap();
        let entry = BlockId::new(4_252).unwrap();
        let target = BlockId::new(4_253).unwrap();
        let input = ValueId::new(4_254).unwrap();
        let forwarded = ValueId::new(4_255).unwrap();
        let result = ValueId::new(4_256).unwrap();
        let computed = ValueId::new(4_261).unwrap();
        let boolean = |id| ValueDeclaration {
            id,
            scalar_type: ScalarType::Boolean,
        };
        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            proof_output_calls: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine,
                attachment: None,
                parameters: vec![boolean(input)],
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Scalar(boolean(result)),
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry,
                blocks: vec![
                    Block {
                        id: entry,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Jump {
                            edge: EdgeId::new(4_257).unwrap(),
                            target,
                            arguments: vec![input],
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    Block {
                        id: target,
                        parameters: vec![boolean(forwarded)],
                        operations: vec![Operation {
                            id: OperationId::new(4_258).unwrap(),
                            result: OperationResult::Scalar(boolean(computed)),
                            kind: OperationKind::BooleanNot { operand: forwarded },
                        }],
                        terminator: Terminator::Return {
                            edge: EdgeId::new(4_259).unwrap(),
                            value: computed,
                            cleanup_actions: Vec::new(),
                        },
                    },
                ],
                contract: MachineContract {
                    id: ContractId::new(4_260).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let proof = ProofBundle {
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        };
        (
            psi_terminal_codec::encode_module(&module).unwrap(),
            psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
        )
    }

    fn adjacent_conditional_merge_artifact() -> (Vec<u8>, Vec<u8>) {
        let machine = MachineId::new(4_271).unwrap();
        let entry = BlockId::new(4_272).unwrap();
        let decision = BlockId::new(4_273).unwrap();
        let left = BlockId::new(4_274).unwrap();
        let right = BlockId::new(4_275).unwrap();
        let condition = ValueId::new(4_276).unwrap();
        let forwarded = ValueId::new(4_277).unwrap();
        let boolean = |id| ValueDeclaration {
            id,
            scalar_type: ScalarType::Boolean,
        };
        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            proof_output_calls: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine,
                attachment: None,
                parameters: vec![boolean(condition)],
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry,
                blocks: vec![
                    Block {
                        id: entry,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Jump {
                            edge: EdgeId::new(4_278).unwrap(),
                            target: decision,
                            arguments: vec![condition],
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    Block {
                        id: decision,
                        parameters: vec![boolean(forwarded)],
                        operations: Vec::new(),
                        terminator: Terminator::Conditional {
                            condition: forwarded,
                            when_true: SuccessorEdge {
                                edge: EdgeId::new(4_279).unwrap(),
                                target: left,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: SuccessorEdge {
                                edge: EdgeId::new(4_280).unwrap(),
                                target: right,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                    },
                    Block {
                        id: left,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::ReturnUnit {
                            edge: EdgeId::new(4_281).unwrap(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    Block {
                        id: right,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::ReturnUnit {
                            edge: EdgeId::new(4_282).unwrap(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                ],
                contract: MachineContract {
                    id: ContractId::new(4_283).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let proof = ProofBundle {
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        };
        (
            psi_terminal_codec::encode_module(&module).unwrap(),
            psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
        )
    }

    fn path_qualified_empty_block_artifact() -> (Vec<u8>, Vec<u8>) {
        let machine = MachineId::new(4_301).unwrap();
        let entry = BlockId::new(4_302).unwrap();
        let left = BlockId::new(4_303).unwrap();
        let right = BlockId::new(4_304).unwrap();
        let empty = BlockId::new(4_305).unwrap();
        let target = BlockId::new(4_306).unwrap();
        let condition = ValueId::new(4_307).unwrap();
        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            proof_output_calls: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine,
                attachment: None,
                parameters: vec![ValueDeclaration {
                    id: condition,
                    scalar_type: ScalarType::Boolean,
                }],
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry,
                blocks: vec![
                    Block {
                        id: entry,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Conditional {
                            condition,
                            when_true: SuccessorEdge {
                                edge: EdgeId::new(4_311).unwrap(),
                                target: left,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: SuccessorEdge {
                                edge: EdgeId::new(4_312).unwrap(),
                                target: right,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                    },
                    Block {
                        id: left,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Jump {
                            edge: EdgeId::new(4_313).unwrap(),
                            target: empty,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    Block {
                        id: right,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Jump {
                            edge: EdgeId::new(4_314).unwrap(),
                            target: empty,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    Block {
                        id: empty,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Jump {
                            edge: EdgeId::new(4_315).unwrap(),
                            target,
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    Block {
                        id: target,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::ReturnUnit {
                            edge: EdgeId::new(4_316).unwrap(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                ],
                contract: MachineContract {
                    id: ContractId::new(4_317).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let proof = ProofBundle {
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        };
        (
            psi_terminal_codec::encode_module(&module).unwrap(),
            psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
        )
    }

    fn staged_forwarded_conditional(target: NativeTarget) -> StagedOptimizedSelectedInstructions {
        let (semantic, proof) = conditional_forwarded_parameter_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap();
        let target =
            omega_lowering_optimizer::lower_optimized_to_target_operations(optimized, target)
                .unwrap();
        stage_optimized_instruction_selection(target).unwrap()
    }

    fn conditional_exact_binary_artifact(subtract: bool) -> (Vec<u8>, Vec<u8>) {
        let machine = MachineId::new(5_001).unwrap();
        let entry = BlockId::new(5_002).unwrap();
        let when_true = BlockId::new(5_003).unwrap();
        let when_false = BlockId::new(5_004).unwrap();
        let condition = ValueId::new(5_005).unwrap();
        let true_left = ValueId::new(5_006).unwrap();
        let true_right = ValueId::new(5_007).unwrap();
        let true_sum = ValueId::new(5_008).unwrap();
        let false_left = ValueId::new(5_009).unwrap();
        let false_right = ValueId::new(5_010).unwrap();
        let false_sum = ValueId::new(5_011).unwrap();
        let result = ValueId::new(5_012).unwrap();
        let true_left_operation = OperationId::new(5_021).unwrap();
        let true_right_operation = OperationId::new(5_022).unwrap();
        let true_add_operation = OperationId::new(5_023).unwrap();
        let false_left_operation = OperationId::new(5_024).unwrap();
        let false_right_operation = OperationId::new(5_025).unwrap();
        let false_add_operation = OperationId::new(5_026).unwrap();
        let true_obligation = ObligationId::new(5_031).unwrap();
        let false_obligation = ObligationId::new(5_032).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let declaration = |id, scalar_type| ValueDeclaration { id, scalar_type };
        let integer_operation = |id, result, kind| Operation {
            id,
            result: OperationResult::Scalar(declaration(result, scalar_type)),
            kind,
        };
        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            proof_output_calls: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine,
                attachment: None,
                parameters: vec![declaration(condition, ScalarType::Boolean)],
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Scalar(declaration(result, scalar_type)),
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry,
                blocks: vec![
                    Block {
                        id: entry,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Conditional {
                            condition,
                            when_true: SuccessorEdge {
                                edge: EdgeId::new(5_041).unwrap(),
                                target: when_true,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: SuccessorEdge {
                                edge: EdgeId::new(5_042).unwrap(),
                                target: when_false,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                    },
                    Block {
                        id: when_true,
                        parameters: Vec::new(),
                        operations: vec![
                            integer_operation(
                                true_left_operation,
                                true_left,
                                OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(if subtract { 13 } else { 7 }),
                                },
                            ),
                            integer_operation(
                                true_right_operation,
                                true_right,
                                OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(if subtract { 5 } else { 8 }),
                                },
                            ),
                            integer_operation(
                                true_add_operation,
                                true_sum,
                                if subtract {
                                    OperationKind::ExactIntegerSubtract {
                                        left: true_left,
                                        right: true_right,
                                        obligation: true_obligation,
                                    }
                                } else {
                                    OperationKind::ExactIntegerAdd {
                                        left: true_left,
                                        right: true_right,
                                        obligation: true_obligation,
                                    }
                                },
                            ),
                        ],
                        terminator: Terminator::Return {
                            edge: EdgeId::new(5_043).unwrap(),
                            value: true_sum,
                            cleanup_actions: Vec::new(),
                        },
                    },
                    Block {
                        id: when_false,
                        parameters: Vec::new(),
                        operations: vec![
                            integer_operation(
                                false_left_operation,
                                false_left,
                                OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(if subtract { 21 } else { 11 }),
                                },
                            ),
                            integer_operation(
                                false_right_operation,
                                false_right,
                                OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(if subtract { 8 } else { 13 }),
                                },
                            ),
                            integer_operation(
                                false_add_operation,
                                false_sum,
                                if subtract {
                                    OperationKind::ExactIntegerSubtract {
                                        left: false_left,
                                        right: false_right,
                                        obligation: false_obligation,
                                    }
                                } else {
                                    OperationKind::ExactIntegerAdd {
                                        left: false_left,
                                        right: false_right,
                                        obligation: false_obligation,
                                    }
                                },
                            ),
                        ],
                        terminator: Terminator::Return {
                            edge: EdgeId::new(5_044).unwrap(),
                            value: false_sum,
                            cleanup_actions: Vec::new(),
                        },
                    },
                ],
                contract: MachineContract {
                    id: ContractId::new(5_051).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let proof = ProofBundle {
            evidence_producers: Vec::new(),
            evidence: [true_obligation, false_obligation]
                .into_iter()
                .map(|obligation| ObligationEvidence {
                    obligation,
                    route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
                })
                .collect(),
        };
        (
            psi_terminal_codec::encode_module(&module).unwrap(),
            psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
        )
    }

    fn conditional_active_resident_exact_add_chain_artifact() -> (Vec<u8>, Vec<u8>) {
        let machine = MachineId::new(5_201).unwrap();
        let entry = BlockId::new(5_202).unwrap();
        let when_true = BlockId::new(5_203).unwrap();
        let when_false = BlockId::new(5_204).unwrap();
        let condition = ValueId::new(5_205).unwrap();
        let resident = ValueId::new(5_206).unwrap();
        let left = ValueId::new(5_207).unwrap();
        let right = ValueId::new(5_208).unwrap();
        let inner = ValueId::new(5_209).unwrap();
        let middle = ValueId::new(5_210).unwrap();
        let result_value = ValueId::new(5_211).unwrap();
        let false_value = ValueId::new(5_212).unwrap();
        let machine_result = ValueId::new(5_213).unwrap();
        let resident_operation = OperationId::new(5_221).unwrap();
        let left_operation = OperationId::new(5_222).unwrap();
        let right_operation = OperationId::new(5_223).unwrap();
        let inner_operation = OperationId::new(5_224).unwrap();
        let middle_operation = OperationId::new(5_225).unwrap();
        let result_operation = OperationId::new(5_226).unwrap();
        let false_operation = OperationId::new(5_227).unwrap();
        let inner_obligation = ObligationId::new(5_231).unwrap();
        let middle_obligation = ObligationId::new(5_232).unwrap();
        let result_obligation = ObligationId::new(5_233).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let declaration = |id, scalar_type| ValueDeclaration { id, scalar_type };
        let operation = |id, result, kind| Operation {
            id,
            result: OperationResult::Scalar(declaration(result, scalar_type)),
            kind,
        };
        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            proof_output_calls: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine,
                attachment: None,
                parameters: vec![declaration(condition, ScalarType::Boolean)],
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Scalar(declaration(machine_result, scalar_type)),
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry,
                blocks: vec![
                    Block {
                        id: entry,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Conditional {
                            condition,
                            when_true: SuccessorEdge {
                                edge: EdgeId::new(5_241).unwrap(),
                                target: when_true,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: SuccessorEdge {
                                edge: EdgeId::new(5_242).unwrap(),
                                target: when_false,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                    },
                    Block {
                        id: when_true,
                        parameters: Vec::new(),
                        operations: vec![
                            operation(
                                resident_operation,
                                resident,
                                OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(3),
                                },
                            ),
                            operation(
                                left_operation,
                                left,
                                OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(5),
                                },
                            ),
                            operation(
                                right_operation,
                                right,
                                OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(7),
                                },
                            ),
                            operation(
                                inner_operation,
                                inner,
                                OperationKind::ExactIntegerAdd {
                                    left,
                                    right,
                                    obligation: inner_obligation,
                                },
                            ),
                            operation(
                                middle_operation,
                                middle,
                                OperationKind::ExactIntegerAdd {
                                    left: resident,
                                    right: inner,
                                    obligation: middle_obligation,
                                },
                            ),
                            operation(
                                result_operation,
                                result_value,
                                OperationKind::ExactIntegerAdd {
                                    left: resident,
                                    right: middle,
                                    obligation: result_obligation,
                                },
                            ),
                        ],
                        terminator: Terminator::Return {
                            edge: EdgeId::new(5_243).unwrap(),
                            value: result_value,
                            cleanup_actions: Vec::new(),
                        },
                    },
                    Block {
                        id: when_false,
                        parameters: Vec::new(),
                        operations: vec![operation(
                            false_operation,
                            false_value,
                            OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(11),
                            },
                        )],
                        terminator: Terminator::Return {
                            edge: EdgeId::new(5_244).unwrap(),
                            value: false_value,
                            cleanup_actions: Vec::new(),
                        },
                    },
                ],
                contract: MachineContract {
                    id: ContractId::new(5_251).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let proof = ProofBundle {
            evidence_producers: Vec::new(),
            evidence: [inner_obligation, middle_obligation, result_obligation]
                .into_iter()
                .map(|obligation| ObligationEvidence {
                    obligation,
                    route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
                })
                .collect(),
        };
        (
            psi_terminal_codec::encode_module(&module).unwrap(),
            psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
        )
    }

    fn staged_active_resident_exact_add_chain(
        target: NativeTarget,
    ) -> StagedOptimizedSelectedInstructions {
        staged_active_resident_exact_add_chain_with_selections(
            target,
            OptimizationSelections::new([
                Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            ])
            .unwrap(),
        )
    }

    fn staged_active_resident_exact_add_chain_with_selections(
        target: NativeTarget,
        selections: OptimizationSelections,
    ) -> StagedOptimizedSelectedInstructions {
        let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(selections),
        )
        .unwrap();
        let target =
            omega_lowering_optimizer::lower_optimized_to_target_operations(optimized, target)
                .unwrap();
        stage_optimized_instruction_selection(target).unwrap()
    }

    fn staged_active_resident_two_view_legality(
        target: NativeTarget,
    ) -> StagedOptimizedAllocationLegality {
        staged_active_resident_two_view_legality_with_selections(
            target,
            OptimizationSelections::new([
                Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            ])
            .unwrap(),
        )
    }

    fn staged_active_resident_two_view_legality_with_selections(
        target: NativeTarget,
        selections: OptimizationSelections,
    ) -> StagedOptimizedAllocationLegality {
        let ranges = stage_optimized_live_ranges(
            stage_optimized_liveness(staged_active_resident_exact_add_chain_with_selections(
                target, selections,
            ))
            .unwrap(),
        )
        .unwrap();
        stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1(ranges)
            .unwrap()
    }

    fn staged_active_resident_rematerialization_and_machine(
        target: NativeTarget,
    ) -> (
        StagedOptimizedActiveResidentRematerialization,
        StagedOptimizedPostAllocationMachinePlan,
    ) {
        let source = stage_optimized_active_resident_rematerialization(
            staged_active_resident_two_view_legality(target),
            TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            selected_lowering_budget(),
        )
        .unwrap();
        let machine =
            stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization(
                &source,
            )
            .unwrap();
        (source, machine)
    }

    fn staged_active_resident_resolved_layout(
        target: NativeTarget,
    ) -> StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout {
        let (source, machine) = staged_active_resident_rematerialization_and_machine(target);
        let pre_layout = stage_optimized_active_resident_rematerialization_selected_form_encoding(
            source, machine,
        )
        .unwrap();
        stage_optimized_active_resident_rematerialization_resolved_selected_form_layout(pre_layout)
            .unwrap()
    }

    fn staged_active_resident_resolved_layout_with_selections(
        target: NativeTarget,
        selections: OptimizationSelections,
    ) -> StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout {
        let source = stage_optimized_active_resident_rematerialization(
            staged_active_resident_two_view_legality_with_selections(target, selections),
            TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            selected_lowering_budget(),
        )
        .unwrap();
        let machine =
            stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization(
                &source,
            )
            .unwrap();
        let pre_layout = stage_optimized_active_resident_rematerialization_selected_form_encoding(
            source, machine,
        )
        .unwrap();
        stage_optimized_active_resident_rematerialization_resolved_selected_form_layout(pre_layout)
            .unwrap()
    }

    fn staged_active_resident_function_relative_realization(
        target: NativeTarget,
    ) -> StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization {
        stage_optimized_active_resident_rematerialization_function_relative_realization(
            staged_active_resident_resolved_layout(target),
        )
        .unwrap()
    }

    fn conditional_widened_u8_exact_add_artifact() -> (Vec<u8>, Vec<u8>) {
        conditional_widened_u8_exact_add_artifact_with_values([200, 55], [254, 1])
    }

    fn conditional_widened_u8_exact_add_artifact_with_values(
        when_true_values: [u128; 2],
        when_false_values: [u128; 2],
    ) -> (Vec<u8>, Vec<u8>) {
        conditional_widened_u8_exact_binary_artifact_with_values(
            false,
            when_true_values,
            when_false_values,
        )
    }

    fn conditional_widened_u8_exact_subtract_artifact() -> (Vec<u8>, Vec<u8>) {
        conditional_widened_u8_exact_binary_artifact_with_values(true, [255, 0], [200, 55])
    }

    fn conditional_widened_u8_exact_subtract_artifact_with_values(
        when_true_values: [u128; 2],
        when_false_values: [u128; 2],
    ) -> (Vec<u8>, Vec<u8>) {
        conditional_widened_u8_exact_binary_artifact_with_values(
            true,
            when_true_values,
            when_false_values,
        )
    }

    fn conditional_widened_u8_exact_binary_artifact_with_values(
        subtract: bool,
        when_true_values: [u128; 2],
        when_false_values: [u128; 2],
    ) -> (Vec<u8>, Vec<u8>) {
        let machine = MachineId::new(5_101).unwrap();
        let entry = BlockId::new(5_102).unwrap();
        let when_true = BlockId::new(5_103).unwrap();
        let when_false = BlockId::new(5_104).unwrap();
        let condition = ValueId::new(5_105).unwrap();
        let true_left = ValueId::new(5_106).unwrap();
        let true_right = ValueId::new(5_107).unwrap();
        let true_narrow_sum = ValueId::new(5_108).unwrap();
        let true_wide_sum = ValueId::new(5_109).unwrap();
        let false_left = ValueId::new(5_110).unwrap();
        let false_right = ValueId::new(5_111).unwrap();
        let false_narrow_sum = ValueId::new(5_112).unwrap();
        let false_wide_sum = ValueId::new(5_113).unwrap();
        let result = ValueId::new(5_114).unwrap();
        let true_left_operation = OperationId::new(5_121).unwrap();
        let true_right_operation = OperationId::new(5_122).unwrap();
        let true_add_operation = OperationId::new(5_123).unwrap();
        let true_widen_operation = OperationId::new(5_124).unwrap();
        let false_left_operation = OperationId::new(5_125).unwrap();
        let false_right_operation = OperationId::new(5_126).unwrap();
        let false_add_operation = OperationId::new(5_127).unwrap();
        let false_widen_operation = OperationId::new(5_128).unwrap();
        let true_obligation = ObligationId::new(5_131).unwrap();
        let false_obligation = ObligationId::new(5_132).unwrap();
        let u8_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        let u64_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let declaration = |id, scalar_type| ValueDeclaration { id, scalar_type };
        let operation = |id, result, scalar_type, kind| Operation {
            id,
            result: OperationResult::Scalar(declaration(result, scalar_type)),
            kind,
        };
        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            proof_output_calls: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine,
                attachment: None,
                parameters: vec![declaration(condition, ScalarType::Boolean)],
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Scalar(declaration(result, u64_type)),
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry,
                blocks: vec![
                    Block {
                        id: entry,
                        parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Conditional {
                            condition,
                            when_true: SuccessorEdge {
                                edge: EdgeId::new(5_141).unwrap(),
                                target: when_true,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: SuccessorEdge {
                                edge: EdgeId::new(5_142).unwrap(),
                                target: when_false,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                    },
                    Block {
                        id: when_true,
                        parameters: Vec::new(),
                        operations: vec![
                            operation(
                                true_left_operation,
                                true_left,
                                u8_type,
                                OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(when_true_values[0]),
                                },
                            ),
                            operation(
                                true_right_operation,
                                true_right,
                                u8_type,
                                OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(when_true_values[1]),
                                },
                            ),
                            operation(
                                true_add_operation,
                                true_narrow_sum,
                                u8_type,
                                if subtract {
                                    OperationKind::ExactIntegerSubtract {
                                        left: true_left,
                                        right: true_right,
                                        obligation: true_obligation,
                                    }
                                } else {
                                    OperationKind::ExactIntegerAdd {
                                        left: true_left,
                                        right: true_right,
                                        obligation: true_obligation,
                                    }
                                },
                            ),
                            operation(
                                true_widen_operation,
                                true_wide_sum,
                                u64_type,
                                OperationKind::IntegerWiden {
                                    operand: true_narrow_sum,
                                },
                            ),
                        ],
                        terminator: Terminator::Return {
                            edge: EdgeId::new(5_143).unwrap(),
                            value: true_wide_sum,
                            cleanup_actions: Vec::new(),
                        },
                    },
                    Block {
                        id: when_false,
                        parameters: Vec::new(),
                        operations: vec![
                            operation(
                                false_left_operation,
                                false_left,
                                u8_type,
                                OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(when_false_values[0]),
                                },
                            ),
                            operation(
                                false_right_operation,
                                false_right,
                                u8_type,
                                OperationKind::IntegerConstant {
                                    value: IntegerValue::Unsigned(when_false_values[1]),
                                },
                            ),
                            operation(
                                false_add_operation,
                                false_narrow_sum,
                                u8_type,
                                if subtract {
                                    OperationKind::ExactIntegerSubtract {
                                        left: false_left,
                                        right: false_right,
                                        obligation: false_obligation,
                                    }
                                } else {
                                    OperationKind::ExactIntegerAdd {
                                        left: false_left,
                                        right: false_right,
                                        obligation: false_obligation,
                                    }
                                },
                            ),
                            operation(
                                false_widen_operation,
                                false_wide_sum,
                                u64_type,
                                OperationKind::IntegerWiden {
                                    operand: false_narrow_sum,
                                },
                            ),
                        ],
                        terminator: Terminator::Return {
                            edge: EdgeId::new(5_144).unwrap(),
                            value: false_wide_sum,
                            cleanup_actions: Vec::new(),
                        },
                    },
                ],
                contract: MachineContract {
                    id: ContractId::new(5_151).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let proof = ProofBundle {
            evidence_producers: Vec::new(),
            evidence: [true_obligation, false_obligation]
                .into_iter()
                .map(|obligation| ObligationEvidence {
                    obligation,
                    route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
                })
                .collect(),
        };
        (
            psi_terminal_codec::encode_module(&module).unwrap(),
            psi_terminal_codec::encode_proof_bundle(&proof).unwrap(),
        )
    }

    fn staged_exact_add_conditional(target: NativeTarget) -> StagedOptimizedSelectedInstructions {
        staged_exact_add_conditional_with_selections(
            target,
            OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
            budget(),
        )
    }

    fn staged_widened_u8_exact_add_conditional(
        target: NativeTarget,
    ) -> StagedOptimizedSelectedInstructions {
        let (semantic, proof) = conditional_widened_u8_exact_add_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap();
        let target =
            omega_lowering_optimizer::lower_optimized_to_target_operations(optimized, target)
                .unwrap();
        stage_optimized_instruction_selection(target).unwrap()
    }

    fn staged_widened_u8_exact_subtract_conditional(
        target: NativeTarget,
    ) -> StagedOptimizedSelectedInstructions {
        let (semantic, proof) = conditional_widened_u8_exact_subtract_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap();
        let target =
            omega_lowering_optimizer::lower_optimized_to_target_operations(optimized, target)
                .unwrap();
        stage_optimized_instruction_selection(target).unwrap()
    }

    fn staged_exact_add_conditional_with_selections(
        target: NativeTarget,
        selections: OptimizationSelections,
        budget: OptimizationWorkBudget,
    ) -> StagedOptimizedSelectedInstructions {
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections, budget).unwrap(),
        )
        .unwrap();
        let target =
            omega_lowering_optimizer::lower_optimized_to_target_operations(optimized, target)
                .unwrap();
        stage_optimized_instruction_selection(target).unwrap()
    }

    fn staged_exact_subtract_conditional(
        target: NativeTarget,
    ) -> StagedOptimizedSelectedInstructions {
        staged_exact_subtract_conditional_with_selections(
            target,
            OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
            budget(),
        )
    }

    fn staged_exact_subtract_conditional_with_selections(
        target: NativeTarget,
        selections: OptimizationSelections,
        budget: OptimizationWorkBudget,
    ) -> StagedOptimizedSelectedInstructions {
        let (semantic, proof) = conditional_exact_binary_artifact(true);
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections, budget).unwrap(),
        )
        .unwrap();
        let target =
            omega_lowering_optimizer::lower_optimized_to_target_operations(optimized, target)
                .unwrap();
        stage_optimized_instruction_selection(target).unwrap()
    }

    fn request(selections: OptimizationSelections) -> ExplicitOptimizationRequest {
        ExplicitOptimizationRequest::new(selections, budget()).unwrap()
    }

    #[test]
    fn empty_selection_cannot_enter_or_decode_artifacts() {
        assert_eq!(
            ExplicitOptimizationRequest::new(OptimizationSelections::default(), budget()),
            Err(EmptyOptimizationSelections)
        );
    }

    #[test]
    fn canonical_three_pass_suite_retains_each_manifest_and_one_ledger() {
        let (semantic, proof) = artifact();
        let selections = OptimizationSelections::new([
            Optimization::CopyPropagation,
            Optimization::SparseConditionalConstantPropagation,
            Optimization::ControlFlowCleanup,
        ])
        .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(selections.clone()),
        )
        .unwrap();

        assert_eq!(optimized.selections(), &selections);
        assert_eq!(optimized.commits().len(), 2);
        assert_eq!(optimized.pass_manifests().len(), 3);
        assert_eq!(optimized.transformation_ledger().records().len(), 2);
        assert_eq!(
            optimized
                .pass_manifests()
                .iter()
                .map(|manifest| manifest.work_usage().commits)
                .collect::<Vec<_>>(),
            [1, 1, 0]
        );
        assert_eq!(
            optimized.pass_manifests()[0].output(),
            optimized.pass_manifests()[1].input()
        );
        assert_eq!(
            optimized.pass_manifests()[1].output(),
            optimized.pass_manifests()[2].input()
        );
        assert!(matches!(
            optimized.plan().functions[0].operations[2],
            TerminalAbstractOperation::IntegerConstant {
                value: IntegerValue::Unsigned(15),
                ..
            }
        ));
        assert_eq!(optimized.plan().functions[0].block_entries.len(), 1);
        assert_eq!(optimized.plan().functions[0].operations.len(), 4);
        assert!(matches!(
            &optimized.plan().functions[0].operations[3],
            TerminalAbstractOperation::Return { value, .. } if *value == ValueId::new(2_006).unwrap()
        ));
        let target = omega_lowering_optimizer::lower_optimized_to_target_operations(
            optimized,
            NativeTarget::linux_x64(),
        )
        .unwrap();
        let staged = stage_optimized_assignment(target).unwrap();
        assert_eq!(staged.assigned().functions.len(), 1);
        assert_eq!(
            staged.register_environment().target(),
            NativeTarget::linux_x64()
        );
        assert_eq!(
            staged
                .register_environment()
                .physical()
                .model()
                .architecture,
            omega_target::Architecture::X86_64
        );
        assert_eq!(staged.custody().function_count(), 1);
        assert_eq!(
            staged.custody().register_environment(),
            staged.register_environment().identity()
        );
        assert_eq!(
            staged.custody().optimization(),
            staged
                .optimized_target()
                .optimized()
                .identity_bundle()
                .identity()
        );
        assert_eq!(
            staged.custody().projection(),
            staged
                .optimized_target()
                .optimized()
                .validation()
                .identity()
        );
        assert_eq!(
            staged.custody().manifest(),
            staged
                .optimized_target()
                .optimized()
                .pre_physical_manifest()
                .record()
                .identity
        );
    }

    #[test]
    fn three_pass_artifact_orchestration_is_deterministic() {
        let (semantic, proof) = artifact();
        let selections = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::ControlFlowCleanup,
            Optimization::CopyPropagation,
        ])
        .unwrap();
        let run = || {
            optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(selections.clone()),
            )
            .unwrap()
        };
        let first = run();
        let second = run();

        assert_eq!(first.plan(), second.plan());
        assert_eq!(first.pass_manifests(), second.pass_manifests());
        assert_eq!(
            first.transformation_ledger(),
            second.transformation_ledger()
        );
        assert_eq!(first.identity_bundle(), second.identity_bundle());
        assert_eq!(first.validation(), second.validation());
    }

    #[test]
    fn control_flow_cleanup_selection_runs_as_its_own_exact_pass() {
        let (semantic, proof) = artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap()),
        )
        .unwrap();
        assert_eq!(optimized.pass_manifests().len(), 1);
        assert_eq!(
            optimized.selections().as_slice(),
            [Optimization::ControlFlowCleanup]
        );
    }

    #[test]
    fn control_flow_cleanup_projects_an_atomically_pruned_block_roster() {
        let (semantic, proof) = constant_conditional_prune_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap()),
        )
        .unwrap();

        assert_eq!(optimized.commits().len(), 2);
        assert_eq!(optimized.plan().functions[0].block_entries.len(), 1);
        assert_eq!(
            optimized.plan().functions[0].block_entries[0].operation_offset,
            0
        );
        assert_eq!(optimized.plan().functions[0].operations.len(), 2);
        assert_eq!(optimized.transformation_ledger().records().len(), 2);
        assert_eq!(
            optimized.transformation_ledger().records()[0]
                .provenance
                .iter()
                .filter(|row| !row.disposition.is_realized())
                .count(),
            2
        );
        let report = optimized.pre_physical_manifest().record().render_text();
        assert!(report.contains("source structure: functions=1, blocks=3, nodes=4"));
        assert!(report.contains("optimized structure: functions=1, blocks=1, nodes=2"));
        assert!(report.contains("proven-unreachable=2"));
        assert!(report.contains("runtime-charge=none reason=proven-unreachable"));
    }

    #[test]
    fn control_flow_cleanup_projects_linear_threading_with_both_fuel_sources() {
        let (semantic, proof) = linear_empty_block_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap()),
        )
        .unwrap();

        assert_eq!(optimized.commits().len(), 2);
        assert_eq!(optimized.plan().functions[0].block_entries.len(), 1);
        assert_eq!(optimized.plan().functions[0].operations.len(), 1);
        assert_eq!(
            optimized.unit().functions[0].blocks[0].nodes[0]
                .provenance
                .len(),
            3
        );
        assert_eq!(
            optimized.unit().functions[0].blocks[0].nodes[0].fuel.len(),
            3
        );
        assert_eq!(optimized.transformation_ledger().records().len(), 2);
        assert!(
            optimized
                .transformation_ledger()
                .records()
                .iter()
                .flat_map(|record| &record.provenance)
                .all(|row| row.disposition.is_realized())
        );
        assert!(
            optimized
                .pre_physical_manifest()
                .record()
                .render_text()
                .contains("optimized structure: functions=1, blocks=1, nodes=1")
        );
    }

    #[test]
    fn control_flow_cleanup_projects_adjacent_block_merge_occurrences() {
        let (semantic, proof) = adjacent_block_merge_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap()),
        )
        .unwrap();

        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(optimized.plan().functions[0].block_entries.len(), 1);
        assert_eq!(optimized.plan().functions[0].operations.len(), 2);
        let first = &optimized.unit().functions[0].blocks[0].nodes[0];
        assert!(matches!(
            first.operation,
            omega_terminal_abstract_operations::TerminalAbstractOperation::BooleanNot {
                operand,
                ..
            } if operand == ValueId::new(4_254).unwrap()
        ));
        assert_eq!(
            first.provenance,
            [
                omega_optimization_unit::PsiProvenance::Operation(OperationId::new(4_258).unwrap()),
                omega_optimization_unit::PsiProvenance::Edge(EdgeId::new(4_257).unwrap()),
            ]
        );
        assert_eq!(first.fuel.len(), 2);
        assert_eq!(optimized.transformation_ledger().records().len(), 1);
        assert_eq!(
            optimized.transformation_ledger().records()[0]
                .provenance
                .len(),
            3
        );
        let report = optimized.pre_physical_manifest().record().render_text();
        assert!(report.contains("source structure: functions=1, blocks=2, nodes=3"));
        assert!(report.contains("optimized structure: functions=1, blocks=1, nodes=2"));
    }

    #[test]
    fn control_flow_cleanup_projects_adjacent_conditional_fanout() {
        let (semantic, proof) = adjacent_conditional_merge_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap()),
        )
        .unwrap();

        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(optimized.plan().functions[0].block_entries.len(), 3);
        assert_eq!(optimized.plan().functions[0].operations.len(), 3);
        let node = &optimized.unit().functions[0].blocks[0].nodes[0];
        assert!(matches!(
            node.operation,
            omega_terminal_abstract_operations::TerminalAbstractOperation::Conditional {
                condition,
                ..
            } if condition == ValueId::new(4_276).unwrap()
        ));
        let inherited = omega_optimization_unit::PsiProvenance::Edge(EdgeId::new(4_278).unwrap());
        assert!(
            node.successors
                .iter()
                .all(|edge| edge.provenance.last() == Some(&inherited))
        );
        let input = omega_optimization_unit::PsiRealizationSite::Edge {
            machine: MachineId::new(4_271).unwrap(),
            edge: EdgeId::new(4_278).unwrap(),
        };
        assert_eq!(
            optimized.transformation_ledger().records()[0]
                .provenance
                .iter()
                .filter(|row| row.input == input)
                .count(),
            2
        );
        let report = optimized.pre_physical_manifest().record().render_text();
        assert!(report.contains("source structure: functions=1, blocks=4, nodes=4"));
        assert!(report.contains("optimized structure: functions=1, blocks=3, nodes=3"));
    }

    #[test]
    fn control_flow_cleanup_projects_path_qualified_fanout_custody() {
        let (semantic, proof) = path_qualified_empty_block_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap()),
        )
        .unwrap();

        assert_eq!(optimized.commits().len(), 3);
        assert_eq!(optimized.plan().functions[0].block_entries.len(), 2);
        assert_eq!(optimized.plan().functions[0].operations.len(), 2);
        let outgoing = omega_optimization_unit::PsiProvenance::Edge(EdgeId::new(4_315).unwrap());
        let occurrences = optimized.unit().functions[0]
            .blocks
            .iter()
            .flat_map(|block| block.nodes.iter())
            .flat_map(|node| node.successors.iter())
            .filter(|edge| edge.provenance.contains(&outgoing))
            .collect::<Vec<_>>();
        assert_eq!(occurrences.len(), 2);
        assert_eq!(occurrences[0].target, BlockId::new(4_306).unwrap());
        assert_eq!(occurrences[1].target, BlockId::new(4_306).unwrap());
        let ledger = optimized.transformation_ledger();
        let outgoing_site = omega_optimization_unit::PsiRealizationSite::Edge {
            machine: MachineId::new(4_301).unwrap(),
            edge: EdgeId::new(4_315).unwrap(),
        };
        assert_eq!(
            ledger
                .records()
                .iter()
                .flat_map(|record| &record.provenance)
                .filter(|row| row.input == outgoing_site)
                .count(),
            2
        );
        assert!(
            optimized
                .pre_physical_manifest()
                .record()
                .render_text()
                .contains("optimized structure: functions=1, blocks=2, nodes=2")
        );
    }

    #[test]
    fn mixed_phase_suite_retains_the_full_request_and_exact_psi_projection() {
        let (semantic, proof) = artifact();
        let selections = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::SelectedIncomingU12ExactAddImmediate,
        ])
        .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(selections.clone()),
        )
        .unwrap();

        assert_eq!(optimized.selections(), &selections);
        assert_eq!(
            optimized.psi_selections().as_slice(),
            &[Optimization::SparseConditionalConstantPropagation]
        );
        assert_eq!(optimized.pass_manifests().len(), 1);
        assert_eq!(
            optimized.pre_physical_manifest().record().selections,
            selections
        );
    }

    #[test]
    fn lower_only_suite_reaches_prephysical_custody_without_claiming_psi_work() {
        let (semantic, proof) = artifact();
        let selections =
            OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate])
                .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(selections.clone()),
        )
        .unwrap();

        assert_eq!(optimized.selections(), &selections);
        assert!(optimized.psi_selections().is_empty());
        assert!(optimized.commits().is_empty());
        assert!(optimized.pass_manifests().is_empty());
        assert!(
            optimized
                .pre_physical_manifest()
                .record()
                .psi_selections
                .is_empty()
        );
    }

    #[test]
    fn unsupported_target_shape_fails_at_legalization_boundary() {
        let (semantic, proof) = artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap();
        let target = omega_lowering_optimizer::lower_optimized_to_target_operations(
            optimized,
            NativeTarget::linux_x64(),
        )
        .unwrap();
        assert!(matches!(
            stage_optimized_instruction_selection(target),
            Err(OptimizedSelectionPipelineError::Legalization(
                TerminalLegalizationError::UnsupportedSourceShape { function: 0 }
            ))
        ));
    }

    #[test]
    fn non_u64_conditional_fails_at_named_integer_legalization_boundary() {
        let (semantic, proof) = conditional_immediate_artifact_with_type(
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        );
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap();
        let target = omega_lowering_optimizer::lower_optimized_to_target_operations(
            optimized,
            NativeTarget::linux_x64(),
        )
        .unwrap();
        assert!(matches!(
            stage_optimized_instruction_selection(target),
            Err(OptimizedSelectionPipelineError::Legalization(
                TerminalLegalizationError::UnsupportedIntegerShape { function: 0 }
            ))
        ));
    }

    #[test]
    fn overflowing_u8_add_is_rejected_before_the_widen_commutation_recipe() {
        let (semantic, proof) =
            conditional_widened_u8_exact_add_artifact_with_values([255, 1], [254, 1]);
        let error = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            OptimizationPipelineError::ArtifactLowering(
                omega_terminal_psi_to_abstract_operations::ArtifactLoweringError::Verification(
                    psi_terminal_verifier::VerificationError::RejectedEvidence {
                        obligation,
                        ..
                    }
                )
            ) if obligation == ObligationId::new(5_131).unwrap()
        ));
    }

    #[test]
    fn underflowing_u8_subtract_is_rejected_before_the_widen_commutation_recipe() {
        let (semantic, proof) =
            conditional_widened_u8_exact_subtract_artifact_with_values([0, 1], [200, 55]);
        let error = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            OptimizationPipelineError::ArtifactLowering(
                omega_terminal_psi_to_abstract_operations::ArtifactLoweringError::Verification(
                    psi_terminal_verifier::VerificationError::RejectedEvidence {
                        obligation,
                        ..
                    }
                )
            ) if obligation == ObligationId::new(5_131).unwrap()
        ));
    }

    #[test]
    fn per_pass_budget_exhaustion_publishes_no_carrier() {
        let (semantic, proof) = artifact();
        let selections = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::CopyPropagation,
        ])
        .unwrap();
        let constrained = OptimizationWorkBudget::new(128, 128, 128, 1, 16).unwrap();
        let error = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections, constrained).unwrap(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            OptimizationPipelineError::Run(OptimizationRunError::WorkBudgetExhausted("commits"))
        ));
    }

    #[test]
    fn staged_assignment_is_deterministic_and_retains_optimizer_custody() {
        let (semantic, proof) = artifact();
        let selections = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::CopyPropagation,
        ])
        .unwrap();
        let stage = || {
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(selections.clone()),
            )
            .unwrap();
            let target = omega_lowering_optimizer::lower_optimized_to_target_operations(
                optimized,
                NativeTarget::linux_x64(),
            )
            .unwrap();
            stage_optimized_assignment(target).unwrap()
        };
        let first = stage();
        let second = stage();

        assert_eq!(first.assigned(), second.assigned());
        assert_eq!(first.custody(), second.custody());
        assert_eq!(
            first.optimized_target().optimized().transformation_ledger(),
            second
                .optimized_target()
                .optimized()
                .transformation_ledger()
        );
        assert_eq!(
            first.optimized_target().optimized().pass_manifests(),
            second.optimized_target().optimized().pass_manifests()
        );
    }

    #[test]
    fn independent_assignment_custody_rejects_each_root_and_provenance_corruption() {
        let (semantic, proof) = artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(
                OptimizationSelections::new([
                    Optimization::SparseConditionalConstantPropagation,
                    Optimization::CopyPropagation,
                ])
                .unwrap(),
            ),
        )
        .unwrap();
        let target = omega_lowering_optimizer::lower_optimized_to_target_operations(
            optimized,
            NativeTarget::linux_x64(),
        )
        .unwrap();
        let staged = stage_optimized_assignment(target).unwrap();

        let wrong_environment =
            baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
        assert_eq!(
            validate_optimized_assignment_custody(
                staged.optimized_target(),
                &wrong_environment,
                staged.assigned(),
            ),
            Err(OptimizedAssignmentCustodyError::RegisterEnvironmentTargetMismatch)
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.terminal_psi.program_fingerprint =
            psi_terminal::SemanticFingerprint::from_bytes([0x44; 32]);
        assert_eq!(
            validate_optimized_assignment_custody(
                staged.optimized_target(),
                staged.register_environment(),
                &corrupted,
            ),
            Err(OptimizedAssignmentCustodyError::TerminalPsiMismatch)
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.target = NativeTarget::windows_x64();
        assert_eq!(
            validate_optimized_assignment_custody(
                staged.optimized_target(),
                staged.register_environment(),
                &corrupted,
            ),
            Err(OptimizedAssignmentCustodyError::NativeTargetMismatch)
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.entry = MachineId::new(9_001).unwrap();
        assert_eq!(
            validate_optimized_assignment_custody(
                staged.optimized_target(),
                staged.register_environment(),
                &corrupted,
            ),
            Err(OptimizedAssignmentCustodyError::EntryMismatch)
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.functions.push(corrupted.functions[0].clone());
        assert_eq!(
            validate_optimized_assignment_custody(
                staged.optimized_target(),
                staged.register_environment(),
                &corrupted,
            ),
            Err(OptimizedAssignmentCustodyError::FunctionCountMismatch {
                expected: 1,
                actual: 2,
            })
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.functions[0].machine = MachineId::new(9_002).unwrap();
        assert_eq!(
            validate_optimized_assignment_custody(
                staged.optimized_target(),
                staged.register_environment(),
                &corrupted,
            ),
            Err(OptimizedAssignmentCustodyError::FunctionMachineMismatch { position: 0 })
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.functions[0].attachment = Some(psi_core::StructuralTypeId::new(9_003).unwrap());
        assert_eq!(
            validate_optimized_assignment_custody(
                staged.optimized_target(),
                staged.register_environment(),
                &corrupted,
            ),
            Err(OptimizedAssignmentCustodyError::FunctionAttachmentMismatch { position: 0 })
        );

        let mut corrupted = staged.assigned().clone();
        corrupted.functions[0]
            .provenance
            .operations
            .push(OperationId::new(9_004).unwrap());
        assert_eq!(
            validate_optimized_assignment_custody(
                staged.optimized_target(),
                staged.register_environment(),
                &corrupted,
            ),
            Err(OptimizedAssignmentCustodyError::FunctionProvenanceMismatch { position: 0 })
        );
    }

    #[test]
    fn verified_three_block_conditional_selects_typed_vregs_on_both_architectures() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let staged = staged_conditional(target);
            let plan = staged.selected().plan();
            assert_eq!(plan.functions.len(), 1);
            assert_eq!(plan.functions[0].blocks.len(), 3);
            assert_eq!(plan.functions[0].virtual_registers.len(), 3);
            assert_eq!(staged.selected().receipt().instruction_count(), 6);
            assert_eq!(
                staged.custody().optimization_unit(),
                staged.optimized_target().optimized().unit().identity
            );
            assert_eq!(staged.custody().fuel_schedule(), plan.fuel_schedule);
            assert_eq!(staged.legalized().receipt().target(), target);
            assert_eq!(staged.legalized().receipt().function_count(), 1);
            assert_eq!(staged.legalized().receipt().decomposition_count(), 0);
            assert_eq!(
                staged.custody().legalized(),
                staged.legalized().receipt().identity()
            );
            assert_eq!(
                staged.selected().receipt().legalized(),
                staged.legalized().receipt().identity()
            );
            assert_eq!(
                staged.custody().register_environment(),
                staged.register_environment().identity()
            );
            assert_eq!(
                staged.custody().selected(),
                staged.selected().receipt().identity()
            );
            let mut copy_tagged = plan.clone();
            copy_tagged.functions[0].blocks[1].instructions[0].kind =
                TerminalSelectedInstructionKind::CopyI64;
            assert_ne!(
                terminal_selected_instruction_plan_identity(&copy_tagged),
                staged.selected().receipt().identity()
            );

            let entry = &plan.functions[0].blocks[0];
            assert_eq!(
                entry.instructions[0].kind,
                TerminalSelectedInstructionKind::CompareI64Zero
            );
            assert!(entry.instructions[0].provenance.fuel.is_empty());
            let TerminalSelectedTerminator::ConditionalBranch {
                instruction,
                when_nonzero,
                when_zero,
            } = &entry.terminator
            else {
                panic!("entry must branch")
            };
            assert_eq!(
                instruction.kind,
                TerminalSelectedInstructionKind::ConditionalBranchNonZero
            );
            assert!(instruction.provenance.fuel.is_empty());
            assert_eq!(when_nonzero.fuel.len(), 1);
            assert_eq!(when_zero.fuel.len(), 1);
            assert_ne!(when_nonzero.psi_edge, when_zero.psi_edge);
            for block in &plan.functions[0].blocks[1..] {
                assert!(matches!(
                    block.instructions[0].kind,
                    TerminalSelectedInstructionKind::MaterializeI64 { .. }
                ));
                assert_eq!(block.instructions[0].provenance.operations.len(), 1);
                assert_eq!(block.instructions[0].provenance.fuel.len(), 1);
                let TerminalSelectedTerminator::Return { instruction, .. } = &block.terminator
                else {
                    panic!("leaf must return")
                };
                assert!(instruction.operands[0].fixed_view.is_some());
                assert_eq!(instruction.provenance.fuel.len(), 1);
            }
        }
    }

    #[test]
    fn legalization_identity_and_replay_reject_target_recipe_provenance_and_fuel_corruption() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let staged = staged_conditional(target);
            let original = staged.legalized().plan();
            let identity = terminal_legalized_operation_plan_identity(original);
            assert_eq!(identity, staged.legalized().receipt().identity());
            assert_eq!(
                staged.legalized().receipt().validator(),
                terminal_legalization_validator_identity()
            );
            assert_eq!(
                staged.selected().receipt().legalization_validator(),
                terminal_legalization_validator_identity()
            );
            assert_eq!(
                staged.custody().legalization_validator(),
                terminal_legalization_validator_identity()
            );
            assert_eq!(
                identity,
                staged_conditional(target).legalized().receipt().identity()
            );
            assert_eq!(
                original.functions[0].recipe,
                TerminalLegalizationRecipe::ReturnU64ImmediateConditionalV1
            );

            let validate = |plan| {
                validate_terminal_legalized_operations(
                    staged.optimized_target().target_operations(),
                    staged.optimized_target().optimized().plan(),
                    staged.optimized_target().optimized().unit(),
                    plan,
                )
            };

            let mut corrupted = original.clone();
            corrupted.target = if target.architecture == omega_target::Architecture::X86_64 {
                NativeTarget::linux_arm64()
            } else {
                NativeTarget::linux_x64()
            };
            assert_ne!(
                terminal_legalized_operation_plan_identity(&corrupted),
                identity
            );
            assert_eq!(
                validate(corrupted),
                Err(TerminalLegalizationError::NonCanonicalLegalizedPlan)
            );

            let mut corrupted = original.clone();
            corrupted.functions[0].recipe =
                TerminalLegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1;
            assert_ne!(
                terminal_legalized_operation_plan_identity(&corrupted),
                identity
            );
            assert_eq!(
                validate(corrupted),
                Err(TerminalLegalizationError::NonCanonicalLegalizedPlan)
            );

            let mut corrupted = original.clone();
            corrupted.functions[0]
                .provenance
                .operations
                .push(OperationId::new(9_601).unwrap());
            assert_ne!(
                terminal_legalized_operation_plan_identity(&corrupted),
                identity
            );
            assert_eq!(
                validate(corrupted),
                Err(TerminalLegalizationError::NonCanonicalLegalizedPlan)
            );

            let mut corrupted = original.clone();
            corrupted.functions[0].branch_true_fuel[0].units += 1;
            assert_ne!(
                terminal_legalized_operation_plan_identity(&corrupted),
                identity
            );
            assert_eq!(
                validate(corrupted),
                Err(TerminalLegalizationError::NonCanonicalLegalizedPlan)
            );

            let mut corrupted = original.clone();
            corrupted.functions[0].condition_definition_site = ValueDefinitionSite::Node {
                block: corrupted.functions[0].entry_block,
                node: 0,
            };
            assert_ne!(
                terminal_legalized_operation_plan_identity(&corrupted),
                identity
            );
            assert_eq!(
                validate(corrupted),
                Err(TerminalLegalizationError::NonCanonicalLegalizedPlan)
            );

            let mut corrupted = original.clone();
            corrupted.functions[0].branch_true_fuel[0].site =
                omega_optimization_unit::PsiProvenance::Edge(
                    corrupted.functions[0].branch_false_edge,
                );
            assert_ne!(
                terminal_legalized_operation_plan_identity(&corrupted),
                identity
            );
            assert_eq!(
                validate(corrupted),
                Err(TerminalLegalizationError::NonCanonicalLegalizedPlan)
            );

            let mut corrupted = original.clone();
            corrupted.functions[0].provenance.edges.swap(0, 1);
            assert_ne!(
                terminal_legalized_operation_plan_identity(&corrupted),
                identity
            );
            assert!(matches!(
                validate(corrupted),
                Err(TerminalLegalizationError::SourceCustodyMismatch)
                    | Err(TerminalLegalizationError::NonCanonicalLegalizedPlan)
            ));
        }
    }

    #[test]
    fn widened_u8_exact_add_legalization_retains_theorem_temporaries_and_exact_custody() {
        let u8_integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let u64_integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let expected_function_operations = vec![
            OperationId::new(5_121).unwrap(),
            OperationId::new(5_122).unwrap(),
            OperationId::new(5_123).unwrap(),
            OperationId::new(5_124).unwrap(),
            OperationId::new(5_125).unwrap(),
            OperationId::new(5_126).unwrap(),
            OperationId::new(5_127).unwrap(),
            OperationId::new(5_128).unwrap(),
        ];
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let staged = staged_widened_u8_exact_add_conditional(target);
            let target_plan = staged.optimized_target().target_operations();
            let target_function = &target_plan.functions[0];
            let TerminalTargetOperation::ReturnIntegerConditionalControl {
                scalar_type,
                when_true,
                when_false,
                ..
            } = &target_function.operation
            else {
                panic!("fixture must lower to bounded integer conditional control")
            };
            assert_eq!(*scalar_type, u64_integer);
            assert_eq!(
                target_function.provenance.operations,
                expected_function_operations
            );
            for (
                arm,
                expected_wide,
                expected_widen_operation,
                expected_add_operation,
                expected_obligation,
                expected_left,
                expected_left_value,
                expected_right,
                expected_right_value,
            ) in [
                (
                    when_true,
                    ValueId::new(5_109).unwrap(),
                    OperationId::new(5_124).unwrap(),
                    OperationId::new(5_123).unwrap(),
                    ObligationId::new(5_131).unwrap(),
                    ValueId::new(5_106).unwrap(),
                    IntegerValue::Unsigned(200),
                    ValueId::new(5_107).unwrap(),
                    IntegerValue::Unsigned(55),
                ),
                (
                    when_false,
                    ValueId::new(5_113).unwrap(),
                    OperationId::new(5_128).unwrap(),
                    OperationId::new(5_127).unwrap(),
                    ObligationId::new(5_132).unwrap(),
                    ValueId::new(5_110).unwrap(),
                    IntegerValue::Unsigned(254),
                    ValueId::new(5_111).unwrap(),
                    IntegerValue::Unsigned(1),
                ),
            ] {
                let TerminalTargetIntegerControl::Return {
                    source_value,
                    expression,
                    ..
                } = arm.control.as_ref()
                else {
                    panic!("conditional arm must return its widened value")
                };
                assert_eq!(*source_value, expected_wide);
                let TerminalTargetIntegerExpression::IntegerWiden {
                    psi_operation: widen_operation,
                    source_type,
                    operand,
                } = expression
                else {
                    panic!("exact u8 addition must remain nested under its widening")
                };
                assert_eq!(*widen_operation, expected_widen_operation);
                assert_eq!(*source_type, u8_integer);
                let TerminalTargetIntegerExpression::ExactAdd {
                    psi_operation: add_operation,
                    obligation,
                    left,
                    right,
                } = operand.as_ref()
                else {
                    panic!("proof-bearing exact addition must remain explicit")
                };
                assert_eq!(*add_operation, expected_add_operation);
                assert_eq!(*obligation, expected_obligation);
                assert_eq!(
                    left.as_ref(),
                    &TerminalTargetIntegerExpression::Immediate {
                        source_value: expected_left,
                        value: expected_left_value,
                    }
                );
                assert_eq!(
                    right.as_ref(),
                    &TerminalTargetIntegerExpression::Immediate {
                        source_value: expected_right,
                        value: expected_right_value,
                    }
                );
            }

            let legalized = staged.legalized();
            assert_eq!(legalized.receipt().target(), target);
            assert_eq!(legalized.receipt().function_count(), 1);
            assert_eq!(legalized.receipt().decomposition_count(), 2);
            assert_eq!(
                legalized.receipt().validator(),
                terminal_legalization_validator_identity()
            );
            assert_eq!(
                legalized.receipt().identity(),
                staged_widened_u8_exact_add_conditional(target)
                    .legalized()
                    .receipt()
                    .identity()
            );
            let legalized_function = &legalized.plan().functions[0];
            assert_eq!(
                legalized_function.recipe,
                TerminalLegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1
            );
            assert_eq!(
                legalized_function.provenance.operations,
                expected_function_operations
            );
            assert_eq!(
                legalized_function.provenance.edges,
                vec![
                    EdgeId::new(5_141).unwrap(),
                    EdgeId::new(5_142).unwrap(),
                    EdgeId::new(5_143).unwrap(),
                    EdgeId::new(5_144).unwrap(),
                ]
            );
            assert_eq!(
                legalized_function.branch_true_fuel,
                vec![FuelSettlement {
                    site: PsiProvenance::Edge(EdgeId::new(5_141).unwrap()),
                    units: 1,
                }]
            );
            assert_eq!(
                legalized_function.branch_false_fuel,
                vec![FuelSettlement {
                    site: PsiProvenance::Edge(EdgeId::new(5_142).unwrap()),
                    units: 1,
                }]
            );
            assert_eq!(
                legalized_function.when_true.return_fuel,
                vec![FuelSettlement {
                    site: PsiProvenance::Edge(EdgeId::new(5_143).unwrap()),
                    units: 1,
                }]
            );
            assert_eq!(
                legalized_function.when_false.return_fuel,
                vec![FuelSettlement {
                    site: PsiProvenance::Edge(EdgeId::new(5_144).unwrap()),
                    units: 1,
                }]
            );
            let accepted = &staged
                .optimized_target()
                .optimized()
                .unit()
                .accepted_obligation_facts;
            assert_eq!(accepted.len(), 2);
            for (
                leaf,
                expected_left_temporary,
                expected_right_temporary,
                expected_left,
                expected_right,
                expected_narrow,
                expected_wide,
                expected_add_operation,
                expected_widen_operation,
                expected_obligation,
                expected_block,
            ) in [
                (
                    &legalized_function.when_true,
                    TerminalLegalizedTemporaryId(0),
                    TerminalLegalizedTemporaryId(1),
                    ValueId::new(5_106).unwrap(),
                    ValueId::new(5_107).unwrap(),
                    ValueId::new(5_108).unwrap(),
                    ValueId::new(5_109).unwrap(),
                    OperationId::new(5_123).unwrap(),
                    OperationId::new(5_124).unwrap(),
                    ObligationId::new(5_131).unwrap(),
                    BlockId::new(5_103).unwrap(),
                ),
                (
                    &legalized_function.when_false,
                    TerminalLegalizedTemporaryId(2),
                    TerminalLegalizedTemporaryId(3),
                    ValueId::new(5_110).unwrap(),
                    ValueId::new(5_111).unwrap(),
                    ValueId::new(5_112).unwrap(),
                    ValueId::new(5_113).unwrap(),
                    OperationId::new(5_127).unwrap(),
                    OperationId::new(5_128).unwrap(),
                    ObligationId::new(5_132).unwrap(),
                    BlockId::new(5_104).unwrap(),
                ),
            ] {
                assert_eq!(leaf.source_value, expected_wide);
                let TerminalLegalizedLeafValue::WidenedExactAdd {
                    source_type,
                    target_type,
                    theorem,
                    obligation,
                    accepted_fact,
                    add_operation,
                    narrow_result,
                    add_definition_site,
                    add_fuel,
                    widen_operation,
                    widen_definition_site,
                    widen_fuel,
                    left_temporary,
                    right_temporary,
                    left,
                    right,
                } = &leaf.value
                else {
                    panic!("legalizer must publish its proof-aware widening recipe")
                };
                assert_eq!(*source_type, u8_integer);
                assert_eq!(*target_type, u64_integer);
                assert_eq!(
                    *theorem,
                    TerminalLegalizationTheorem::UnsignedExactAddCommutesWithWidenV1
                );
                assert_eq!(*obligation, expected_obligation);
                assert_eq!(*add_operation, expected_add_operation);
                assert_eq!(*narrow_result, expected_narrow);
                assert_eq!(
                    *add_definition_site,
                    ValueDefinitionSite::Node {
                        block: expected_block,
                        node: 2,
                    }
                );
                assert_eq!(*widen_operation, expected_widen_operation);
                assert_eq!(
                    *widen_definition_site,
                    ValueDefinitionSite::Node {
                        block: expected_block,
                        node: 3,
                    }
                );
                assert_eq!(*left_temporary, expected_left_temporary);
                assert_eq!(*right_temporary, expected_right_temporary);
                assert_eq!(left.source_value, expected_left);
                assert_eq!(right.source_value, expected_right);
                assert_eq!(
                    add_fuel,
                    &vec![FuelSettlement {
                        site: PsiProvenance::Operation(expected_add_operation),
                        units: 1,
                    }]
                );
                assert_eq!(
                    widen_fuel,
                    &vec![FuelSettlement {
                        site: PsiProvenance::Operation(expected_widen_operation),
                        units: 1,
                    }]
                );
                let fact = accepted
                    .iter()
                    .find(|fact| fact.identity == *accepted_fact)
                    .expect("legalized fact remains verifier-owned");
                assert_eq!(fact.operation, expected_add_operation);
                assert_eq!(fact.obligation, expected_obligation);

                let narrow_sum = source_type.exact_add(left.value, right.value).unwrap();
                let widened_sum = source_type
                    .widen_value_to(*target_type, narrow_sum)
                    .unwrap();
                let widened_left = source_type
                    .widen_value_to(*target_type, left.value)
                    .unwrap();
                let widened_right = source_type
                    .widen_value_to(*target_type, right.value)
                    .unwrap();
                assert_eq!(
                    target_type.exact_add(widened_left, widened_right),
                    Some(widened_sum)
                );
            }

            let replayed = validate_terminal_legalized_operations(
                staged.optimized_target().target_operations(),
                staged.optimized_target().optimized().plan(),
                staged.optimized_target().optimized().unit(),
                legalized.plan().clone(),
            )
            .unwrap();
            assert_eq!(replayed.receipt(), legalized.receipt());
        }
    }

    #[test]
    fn widened_u8_exact_subtract_legalization_preserves_authored_order_and_exact_custody() {
        let u8_integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let u64_integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let expected_function_operations = vec![
            OperationId::new(5_121).unwrap(),
            OperationId::new(5_122).unwrap(),
            OperationId::new(5_123).unwrap(),
            OperationId::new(5_124).unwrap(),
            OperationId::new(5_125).unwrap(),
            OperationId::new(5_126).unwrap(),
            OperationId::new(5_127).unwrap(),
            OperationId::new(5_128).unwrap(),
        ];
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let staged = staged_widened_u8_exact_subtract_conditional(target);
            let target_function = &staged.optimized_target().target_operations().functions[0];
            let TerminalTargetOperation::ReturnIntegerConditionalControl {
                scalar_type,
                when_true,
                when_false,
                ..
            } = &target_function.operation
            else {
                panic!("fixture must lower to bounded integer conditional control")
            };
            assert_eq!(*scalar_type, u64_integer);
            assert_eq!(
                target_function.provenance.operations,
                expected_function_operations
            );
            for (
                arm,
                expected_wide,
                expected_widen_operation,
                expected_subtract_operation,
                expected_obligation,
                expected_left,
                expected_left_value,
                expected_right,
                expected_right_value,
            ) in [
                (
                    when_true,
                    ValueId::new(5_109).unwrap(),
                    OperationId::new(5_124).unwrap(),
                    OperationId::new(5_123).unwrap(),
                    ObligationId::new(5_131).unwrap(),
                    ValueId::new(5_106).unwrap(),
                    IntegerValue::Unsigned(255),
                    ValueId::new(5_107).unwrap(),
                    IntegerValue::Unsigned(0),
                ),
                (
                    when_false,
                    ValueId::new(5_113).unwrap(),
                    OperationId::new(5_128).unwrap(),
                    OperationId::new(5_127).unwrap(),
                    ObligationId::new(5_132).unwrap(),
                    ValueId::new(5_110).unwrap(),
                    IntegerValue::Unsigned(200),
                    ValueId::new(5_111).unwrap(),
                    IntegerValue::Unsigned(55),
                ),
            ] {
                let TerminalTargetIntegerControl::Return {
                    source_value,
                    expression,
                    ..
                } = arm.control.as_ref()
                else {
                    panic!("conditional arm must return its widened value")
                };
                assert_eq!(*source_value, expected_wide);
                let TerminalTargetIntegerExpression::IntegerWiden {
                    psi_operation: widen_operation,
                    source_type,
                    operand,
                } = expression
                else {
                    panic!("exact u8 subtraction must remain nested under its widening")
                };
                assert_eq!(*widen_operation, expected_widen_operation);
                assert_eq!(*source_type, u8_integer);
                let TerminalTargetIntegerExpression::ExactSubtract {
                    psi_operation: subtract_operation,
                    obligation,
                    left,
                    right,
                } = operand.as_ref()
                else {
                    panic!("proof-bearing exact subtraction must remain explicit")
                };
                assert_eq!(*subtract_operation, expected_subtract_operation);
                assert_eq!(*obligation, expected_obligation);
                assert_eq!(
                    left.as_ref(),
                    &TerminalTargetIntegerExpression::Immediate {
                        source_value: expected_left,
                        value: expected_left_value,
                    }
                );
                assert_eq!(
                    right.as_ref(),
                    &TerminalTargetIntegerExpression::Immediate {
                        source_value: expected_right,
                        value: expected_right_value,
                    }
                );
            }

            let legalized = staged.legalized();
            assert_eq!(legalized.receipt().target(), target);
            assert_eq!(legalized.receipt().function_count(), 1);
            assert_eq!(legalized.receipt().decomposition_count(), 2);
            assert_eq!(
                legalized.receipt().validator(),
                terminal_legalization_validator_identity()
            );
            assert_eq!(
                legalized.receipt().identity(),
                staged_widened_u8_exact_subtract_conditional(target)
                    .legalized()
                    .receipt()
                    .identity()
            );
            let function = &legalized.plan().functions[0];
            assert_eq!(
                function.recipe,
                TerminalLegalizationRecipe::ReturnU64WidenedU8ExactSubtractImmediateConditionalV1
            );
            assert_eq!(function.provenance.operations, expected_function_operations);
            assert_eq!(
                function.provenance.edges,
                vec![
                    EdgeId::new(5_141).unwrap(),
                    EdgeId::new(5_142).unwrap(),
                    EdgeId::new(5_143).unwrap(),
                    EdgeId::new(5_144).unwrap(),
                ]
            );
            assert_eq!(
                function.branch_true_fuel,
                vec![FuelSettlement {
                    site: PsiProvenance::Edge(EdgeId::new(5_141).unwrap()),
                    units: 1,
                }]
            );
            assert_eq!(
                function.branch_false_fuel,
                vec![FuelSettlement {
                    site: PsiProvenance::Edge(EdgeId::new(5_142).unwrap()),
                    units: 1,
                }]
            );
            assert_eq!(
                function.when_true.return_fuel,
                vec![FuelSettlement {
                    site: PsiProvenance::Edge(EdgeId::new(5_143).unwrap()),
                    units: 1,
                }]
            );
            assert_eq!(
                function.when_false.return_fuel,
                vec![FuelSettlement {
                    site: PsiProvenance::Edge(EdgeId::new(5_144).unwrap()),
                    units: 1,
                }]
            );
            let accepted = &staged
                .optimized_target()
                .optimized()
                .unit()
                .accepted_obligation_facts;
            assert_eq!(accepted.len(), 2);
            for (
                leaf,
                expected_temporaries,
                expected_values,
                expected_operations,
                expected_obligation,
                expected_block,
                expected_constants,
            ) in [
                (
                    &function.when_true,
                    [
                        TerminalLegalizedTemporaryId(0),
                        TerminalLegalizedTemporaryId(1),
                    ],
                    [
                        ValueId::new(5_106).unwrap(),
                        ValueId::new(5_107).unwrap(),
                        ValueId::new(5_108).unwrap(),
                        ValueId::new(5_109).unwrap(),
                    ],
                    [
                        OperationId::new(5_123).unwrap(),
                        OperationId::new(5_124).unwrap(),
                    ],
                    ObligationId::new(5_131).unwrap(),
                    BlockId::new(5_103).unwrap(),
                    [IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)],
                ),
                (
                    &function.when_false,
                    [
                        TerminalLegalizedTemporaryId(2),
                        TerminalLegalizedTemporaryId(3),
                    ],
                    [
                        ValueId::new(5_110).unwrap(),
                        ValueId::new(5_111).unwrap(),
                        ValueId::new(5_112).unwrap(),
                        ValueId::new(5_113).unwrap(),
                    ],
                    [
                        OperationId::new(5_127).unwrap(),
                        OperationId::new(5_128).unwrap(),
                    ],
                    ObligationId::new(5_132).unwrap(),
                    BlockId::new(5_104).unwrap(),
                    [IntegerValue::Unsigned(200), IntegerValue::Unsigned(55)],
                ),
            ] {
                assert_eq!(leaf.source_value, expected_values[3]);
                let TerminalLegalizedLeafValue::WidenedExactSubtract {
                    source_type,
                    target_type,
                    theorem,
                    obligation,
                    accepted_fact,
                    subtract_operation,
                    narrow_result,
                    subtract_definition_site,
                    subtract_fuel,
                    widen_operation,
                    widen_definition_site,
                    widen_fuel,
                    left_temporary,
                    right_temporary,
                    left,
                    right,
                } = &leaf.value
                else {
                    panic!("legalizer must publish its ordered proof-aware subtraction recipe")
                };
                assert_eq!(*source_type, u8_integer);
                assert_eq!(*target_type, u64_integer);
                assert_eq!(
                    *theorem,
                    TerminalLegalizationTheorem::UnsignedExactSubtractCommutesWithWidenV1
                );
                assert_eq!(*obligation, expected_obligation);
                assert_eq!(*subtract_operation, expected_operations[0]);
                assert_eq!(*narrow_result, expected_values[2]);
                assert_eq!(
                    *subtract_definition_site,
                    ValueDefinitionSite::Node {
                        block: expected_block,
                        node: 2,
                    }
                );
                assert_eq!(*widen_operation, expected_operations[1]);
                assert_eq!(
                    *widen_definition_site,
                    ValueDefinitionSite::Node {
                        block: expected_block,
                        node: 3,
                    }
                );
                assert_eq!(*left_temporary, expected_temporaries[0]);
                assert_eq!(*right_temporary, expected_temporaries[1]);
                assert_eq!(left.source_value, expected_values[0]);
                assert_eq!(right.source_value, expected_values[1]);
                assert_eq!(left.value, expected_constants[0]);
                assert_eq!(right.value, expected_constants[1]);
                let constant_operations = if expected_block == BlockId::new(5_103).unwrap() {
                    [
                        OperationId::new(5_121).unwrap(),
                        OperationId::new(5_122).unwrap(),
                    ]
                } else {
                    [
                        OperationId::new(5_125).unwrap(),
                        OperationId::new(5_126).unwrap(),
                    ]
                };
                assert_eq!(left.constant_operation, constant_operations[0]);
                assert_eq!(right.constant_operation, constant_operations[1]);
                assert_eq!(
                    left.definition_site,
                    ValueDefinitionSite::Node {
                        block: expected_block,
                        node: 0,
                    }
                );
                assert_eq!(
                    right.definition_site,
                    ValueDefinitionSite::Node {
                        block: expected_block,
                        node: 1,
                    }
                );
                assert_eq!(
                    left.fuel,
                    vec![FuelSettlement {
                        site: PsiProvenance::Operation(constant_operations[0]),
                        units: 1,
                    }]
                );
                assert_eq!(
                    right.fuel,
                    vec![FuelSettlement {
                        site: PsiProvenance::Operation(constant_operations[1]),
                        units: 1,
                    }]
                );
                assert_eq!(
                    subtract_fuel,
                    &vec![FuelSettlement {
                        site: PsiProvenance::Operation(expected_operations[0]),
                        units: 1,
                    }]
                );
                assert_eq!(
                    widen_fuel,
                    &vec![FuelSettlement {
                        site: PsiProvenance::Operation(expected_operations[1]),
                        units: 1,
                    }]
                );
                let fact = accepted
                    .iter()
                    .find(|fact| fact.identity == *accepted_fact)
                    .expect("legalized fact remains verifier-owned");
                assert_eq!(fact.operation, expected_operations[0]);
                assert_eq!(fact.obligation, expected_obligation);

                let narrow = source_type.exact_sub(left.value, right.value).unwrap();
                let widened = source_type.widen_value_to(*target_type, narrow).unwrap();
                let widened_left = source_type
                    .widen_value_to(*target_type, left.value)
                    .unwrap();
                let widened_right = source_type
                    .widen_value_to(*target_type, right.value)
                    .unwrap();
                assert_eq!(
                    target_type.exact_sub(widened_left, widened_right),
                    Some(widened)
                );
                assert_ne!(
                    target_type.exact_sub(widened_right, widened_left),
                    Some(widened),
                    "the theorem must not commute subtraction operands"
                );
            }

            let replayed = validate_terminal_legalized_operations(
                staged.optimized_target().target_operations(),
                staged.optimized_target().optimized().plan(),
                staged.optimized_target().optimized().unit(),
                legalized.plan().clone(),
            )
            .unwrap();
            assert_eq!(replayed.receipt(), legalized.receipt());
        }
    }

    #[test]
    fn widened_u8_exact_add_independent_replay_rejects_corrupted_bridge_custody() {
        let staged = staged_widened_u8_exact_add_conditional(NativeTarget::linux_x64());
        let original = staged.legalized().plan();
        let validate = |plan| {
            validate_terminal_legalized_operations(
                staged.optimized_target().target_operations(),
                staged.optimized_target().optimized().plan(),
                staged.optimized_target().optimized().unit(),
                plan,
            )
        };
        let false_fact = match original.functions[0].when_false.value {
            TerminalLegalizedLeafValue::WidenedExactAdd { accepted_fact, .. } => accepted_fact,
            _ => panic!("fixture must retain its false-arm proof fact"),
        };

        macro_rules! corrupt_true_leaf {
            (|$value:ident| $body:block) => {{
                let mut corrupted = original.clone();
                let $value = &mut corrupted.functions[0].when_true.value;
                $body
                assert_eq!(
                    validate(corrupted),
                    Err(TerminalLegalizationError::NonCanonicalLegalizedPlan)
                );
            }};
        }

        corrupt_true_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactAdd { source_type, .. } = value else {
                unreachable!()
            };
            *source_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
        });
        corrupt_true_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactAdd { target_type, .. } = value else {
                unreachable!()
            };
            *target_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        });
        corrupt_true_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactAdd { accepted_fact, .. } = value else {
                unreachable!()
            };
            *accepted_fact = false_fact;
        });
        corrupt_true_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactAdd { narrow_result, .. } = value else {
                unreachable!()
            };
            *narrow_result = ValueId::new(9_601).unwrap();
        });
        corrupt_true_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactAdd {
                add_operation,
                widen_operation,
                ..
            } = value
            else {
                unreachable!()
            };
            std::mem::swap(add_operation, widen_operation);
        });
        corrupt_true_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactAdd {
                add_definition_site,
                widen_definition_site,
                ..
            } = value
            else {
                unreachable!()
            };
            std::mem::swap(add_definition_site, widen_definition_site);
        });
        corrupt_true_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactAdd { add_fuel, .. } = value else {
                unreachable!()
            };
            add_fuel[0].units += 1;
        });
        corrupt_true_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactAdd { widen_fuel, .. } = value else {
                unreachable!()
            };
            widen_fuel[0].units += 1;
        });
        corrupt_true_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactAdd {
                left_temporary,
                right_temporary,
                ..
            } = value
            else {
                unreachable!()
            };
            *left_temporary = *right_temporary;
        });
        corrupt_true_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactAdd { left, right, .. } = value else {
                unreachable!()
            };
            std::mem::swap(&mut left.constant_operation, &mut right.constant_operation);
        });
    }

    #[test]
    fn widened_u8_exact_subtract_independent_replay_rejects_corrupted_order_and_custody() {
        let staged = staged_widened_u8_exact_subtract_conditional(NativeTarget::linux_x64());
        let original = staged.legalized().plan();
        let identity = terminal_legalized_operation_plan_identity(original);
        let validate = |plan| {
            validate_terminal_legalized_operations(
                staged.optimized_target().target_operations(),
                staged.optimized_target().optimized().plan(),
                staged.optimized_target().optimized().unit(),
                plan,
            )
        };
        let false_fact = match original.functions[0].when_false.value {
            TerminalLegalizedLeafValue::WidenedExactSubtract { accepted_fact, .. } => accepted_fact,
            _ => panic!("fixture must retain its false-arm proof fact"),
        };

        macro_rules! corrupt_true_subtract_leaf {
            (|$value:ident| $body:block) => {{
                let mut corrupted = original.clone();
                let $value = &mut corrupted.functions[0].when_true.value;
                $body
                assert_ne!(terminal_legalized_operation_plan_identity(&corrupted), identity);
                assert_eq!(
                    validate(corrupted),
                    Err(TerminalLegalizationError::NonCanonicalLegalizedPlan)
                );
            }};
        }

        corrupt_true_subtract_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactSubtract { source_type, .. } = value else {
                unreachable!()
            };
            *source_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
        });
        corrupt_true_subtract_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactSubtract { target_type, .. } = value else {
                unreachable!()
            };
            *target_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        });
        corrupt_true_subtract_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactSubtract { theorem, .. } = value else {
                unreachable!()
            };
            *theorem = TerminalLegalizationTheorem::UnsignedExactAddCommutesWithWidenV1;
        });
        corrupt_true_subtract_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactSubtract { accepted_fact, .. } = value
            else {
                unreachable!()
            };
            *accepted_fact = false_fact;
        });
        corrupt_true_subtract_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactSubtract { obligation, .. } = value else {
                unreachable!()
            };
            *obligation = ObligationId::new(9_611).unwrap();
        });
        corrupt_true_subtract_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactSubtract { narrow_result, .. } = value
            else {
                unreachable!()
            };
            *narrow_result = ValueId::new(9_612).unwrap();
        });
        corrupt_true_subtract_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactSubtract {
                subtract_operation,
                widen_operation,
                ..
            } = value
            else {
                unreachable!()
            };
            std::mem::swap(subtract_operation, widen_operation);
        });
        corrupt_true_subtract_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactSubtract {
                subtract_definition_site,
                widen_definition_site,
                ..
            } = value
            else {
                unreachable!()
            };
            std::mem::swap(subtract_definition_site, widen_definition_site);
        });
        corrupt_true_subtract_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactSubtract { subtract_fuel, .. } = value
            else {
                unreachable!()
            };
            subtract_fuel[0].units += 1;
        });
        corrupt_true_subtract_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactSubtract { widen_fuel, .. } = value else {
                unreachable!()
            };
            widen_fuel[0].units += 1;
        });
        corrupt_true_subtract_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactSubtract {
                left_temporary,
                right_temporary,
                ..
            } = value
            else {
                unreachable!()
            };
            *left_temporary = *right_temporary;
        });
        corrupt_true_subtract_leaf!(|value| {
            let TerminalLegalizedLeafValue::WidenedExactSubtract { left, right, .. } = value else {
                unreachable!()
            };
            std::mem::swap(left, right);
        });

        let mut corrupted = original.clone();
        corrupted.functions[0].recipe =
            TerminalLegalizationRecipe::ReturnU64WidenedU8ExactAddImmediateConditionalV1;
        assert_ne!(
            terminal_legalized_operation_plan_identity(&corrupted),
            identity
        );
        assert_eq!(
            validate(corrupted),
            Err(TerminalLegalizationError::NonCanonicalLegalizedPlan)
        );
    }

    #[test]
    fn widened_u8_exact_add_reaches_selected_effect_and_register_pipelines_on_both_architectures() {
        for (target, expected_homes) in [
            (
                NativeTarget::linux_x64(),
                ["rdi", "rax", "rbx", "rax", "rax", "rbx", "rax"],
            ),
            (
                NativeTarget::linux_arm64(),
                ["x0", "x0", "x1", "x0", "x0", "x1", "x0"],
            ),
        ] {
            let staged = staged_widened_u8_exact_add_conditional(target);
            let function = &staged.selected().plan().functions[0];
            assert_eq!(function.blocks.len(), 3);
            assert_eq!(function.virtual_registers.len(), 7);
            assert_eq!(staged.selected().receipt().instruction_count(), 10);
            assert!(
                function.blocks[0].instructions[0]
                    .provenance
                    .fuel
                    .is_empty()
            );
            let TerminalSelectedTerminator::ConditionalBranch {
                instruction,
                when_nonzero,
                when_zero,
            } = &function.blocks[0].terminator
            else {
                panic!("selected entry must branch")
            };
            assert!(instruction.provenance.fuel.is_empty());
            assert_eq!(
                when_nonzero.fuel,
                vec![FuelSettlement {
                    site: PsiProvenance::Edge(EdgeId::new(5_141).unwrap()),
                    units: 1,
                }]
            );
            assert_eq!(
                when_zero.fuel,
                vec![FuelSettlement {
                    site: PsiProvenance::Edge(EdgeId::new(5_142).unwrap()),
                    units: 1,
                }]
            );
            assert_eq!(
                function
                    .virtual_registers
                    .iter()
                    .map(|register| match register.origin {
                        TerminalVirtualRegisterOrigin::LegalizationTemporary {
                            temporary, ..
                        } => Some(temporary),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
                vec![
                    None,
                    Some(TerminalLegalizedTemporaryId(0)),
                    Some(TerminalLegalizedTemporaryId(1)),
                    None,
                    Some(TerminalLegalizedTemporaryId(2)),
                    Some(TerminalLegalizedTemporaryId(3)),
                    None,
                ]
            );
            for (block, expected) in function.blocks[1..].iter().zip([
                (
                    [IntegerValue::Unsigned(200), IntegerValue::Unsigned(55)],
                    [
                        OperationId::new(5_123).unwrap(),
                        OperationId::new(5_124).unwrap(),
                    ],
                    [
                        ValueId::new(5_106).unwrap(),
                        ValueId::new(5_107).unwrap(),
                        ValueId::new(5_108).unwrap(),
                        ValueId::new(5_109).unwrap(),
                    ],
                    ObligationId::new(5_131).unwrap(),
                ),
                (
                    [IntegerValue::Unsigned(254), IntegerValue::Unsigned(1)],
                    [
                        OperationId::new(5_127).unwrap(),
                        OperationId::new(5_128).unwrap(),
                    ],
                    [
                        ValueId::new(5_110).unwrap(),
                        ValueId::new(5_111).unwrap(),
                        ValueId::new(5_112).unwrap(),
                        ValueId::new(5_113).unwrap(),
                    ],
                    ObligationId::new(5_132).unwrap(),
                ),
            ]) {
                assert_eq!(block.instructions.len(), 3);
                for (materialize, expected_value) in block.instructions[..2].iter().zip(expected.0)
                {
                    assert_eq!(
                        materialize.kind,
                        TerminalSelectedInstructionKind::MaterializeI64 {
                            value: expected_value,
                        }
                    );
                }
                let add = &block.instructions[2];
                assert!(matches!(
                    add.kind,
                    TerminalSelectedInstructionKind::ExactAddI64 { obligation, .. }
                        if obligation == expected.3
                ));
                assert_eq!(
                    add.constraint,
                    staged.register_environment().selected_keys().add_i64
                );
                assert_eq!(
                    add.operands
                        .iter()
                        .map(|operand| operand.access)
                        .collect::<Vec<_>>(),
                    vec![
                        RegisterOperandAccess::Use,
                        RegisterOperandAccess::Use,
                        RegisterOperandAccess::Def,
                    ]
                );
                assert_eq!(add.provenance.operations, expected.1);
                assert_eq!(add.provenance.values, expected.2);
                assert_eq!(add.provenance.obligations, vec![expected.3]);
                assert_eq!(
                    add.provenance.fuel,
                    expected
                        .1
                        .into_iter()
                        .map(|operation| FuelSettlement {
                            site: PsiProvenance::Operation(operation),
                            units: 1,
                        })
                        .collect::<Vec<_>>()
                );
                assert!(
                    add.operands
                        .iter()
                        .all(|operand| operand.fixed_view.is_none())
                );
                assert!(add.operands.iter().all(|operand| operand.tied_to.is_none()));
                assert!(add.implicit_uses.is_empty());
                assert!(add.implicit_defs.is_empty());
                assert!(add.clobbers.is_empty());
                let TerminalSelectedTerminator::Return { instruction, .. } = &block.terminator
                else {
                    panic!("selected leaf must return")
                };
                assert_eq!(instruction.provenance.values, vec![expected.2[3]]);
                assert_eq!(instruction.provenance.fuel.len(), 1);
            }
            for (register, expected_source, expected_site) in [
                (
                    &function.virtual_registers[3],
                    ValueId::new(5_109).unwrap(),
                    ValueDefinitionSite::Node {
                        block: BlockId::new(5_103).unwrap(),
                        node: 3,
                    },
                ),
                (
                    &function.virtual_registers[6],
                    ValueId::new(5_113).unwrap(),
                    ValueDefinitionSite::Node {
                        block: BlockId::new(5_104).unwrap(),
                        node: 3,
                    },
                ),
            ] {
                assert_eq!(register.definition_site, expected_site);
                assert!(matches!(
                    register.origin,
                    TerminalVirtualRegisterOrigin::InstructionResult { source_value, .. }
                        if source_value == expected_source
                ));
            }

            let selected_identity = staged.selected().receipt().identity();
            let mut corrupted = staged.selected().plan().clone();
            let TerminalVirtualRegisterOrigin::LegalizationTemporary { temporary, .. } =
                &mut corrupted.functions[0].virtual_registers[1].origin
            else {
                panic!("first widened operand must retain its legal temporary")
            };
            temporary.0 += 10;
            assert_ne!(
                terminal_selected_instruction_plan_identity(&corrupted),
                selected_identity
            );
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::VirtualRegisterProjectionMismatch { .. })
            ));

            let effects = stage_optimized_machine_effects(&staged).unwrap();
            assert_eq!(effects.custody().instruction_count(), 10);
            let adds = effects
                .effects()
                .plan()
                .functions
                .iter()
                .flat_map(|function| &function.blocks)
                .flat_map(|block| &block.instructions)
                .filter(|instruction| {
                    matches!(
                        instruction.kind,
                        TerminalSelectedInstructionKind::ExactAddI64 { .. }
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(adds.len(), 2);
            for add in adds {
                assert_eq!(add.barrier, TerminalMachineBarrier::None);
                assert_eq!(add.alternatives.len(), 1);
                assert!(add.unit_clobbers.is_empty());
                assert_eq!(add.provenance.operations.len(), 2);
                assert_eq!(add.provenance.values.len(), 4);
                assert_eq!(add.provenance.obligations.len(), 1);
                assert_eq!(add.provenance.fuel.len(), 2);
            }

            let homes = stage_optimized_register_homes(
                stage_optimized_allocation_legality(
                    stage_optimized_live_ranges(stage_optimized_liveness(staged).unwrap()).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            let selected_stage = homes
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage();
            let model = selected_stage.register_environment().physical().model();
            assert_eq!(homes.custody().assignment_count(), 7);
            assert_eq!(
                homes.homes().plan().functions[0]
                    .assignments
                    .iter()
                    .map(|assignment| {
                        model
                            .views
                            .iter()
                            .find(|view| view.id == assignment.view)
                            .unwrap()
                            .name
                            .as_str()
                    })
                    .collect::<Vec<_>>(),
                expected_homes
            );
            let post = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
            assert_eq!(post.custody().instruction_count(), 10);
        }
    }

    #[test]
    fn widened_u8_exact_subtract_reaches_verified_register_and_machine_pipelines() {
        for (target, expected_homes, expected_alternative) in [
            (
                NativeTarget::linux_x64(),
                ["rdi", "rax", "rbx", "rax", "rax", "rbx", "rax"],
                1,
            ),
            (
                NativeTarget::linux_arm64(),
                ["x0", "x0", "x1", "x0", "x0", "x1", "x0"],
                0,
            ),
        ] {
            let staged = staged_widened_u8_exact_subtract_conditional(target);
            let function = &staged.selected().plan().functions[0];
            assert_eq!(function.blocks.len(), 3);
            assert_eq!(function.virtual_registers.len(), 7);
            assert_eq!(staged.selected().receipt().instruction_count(), 10);
            assert_eq!(
                function
                    .virtual_registers
                    .iter()
                    .map(|register| match register.origin {
                        TerminalVirtualRegisterOrigin::LegalizationTemporary {
                            temporary,
                            source_value,
                            ..
                        } => Some((temporary, source_value)),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
                vec![
                    None,
                    Some((
                        TerminalLegalizedTemporaryId(0),
                        ValueId::new(5_106).unwrap()
                    )),
                    Some((
                        TerminalLegalizedTemporaryId(1),
                        ValueId::new(5_107).unwrap()
                    )),
                    None,
                    Some((
                        TerminalLegalizedTemporaryId(2),
                        ValueId::new(5_110).unwrap()
                    )),
                    Some((
                        TerminalLegalizedTemporaryId(3),
                        ValueId::new(5_111).unwrap()
                    )),
                    None,
                ]
            );
            for (block, expected) in function.blocks[1..].iter().zip([
                (
                    [IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)],
                    [
                        OperationId::new(5_123).unwrap(),
                        OperationId::new(5_124).unwrap(),
                    ],
                    [
                        ValueId::new(5_106).unwrap(),
                        ValueId::new(5_107).unwrap(),
                        ValueId::new(5_108).unwrap(),
                        ValueId::new(5_109).unwrap(),
                    ],
                    ObligationId::new(5_131).unwrap(),
                ),
                (
                    [IntegerValue::Unsigned(200), IntegerValue::Unsigned(55)],
                    [
                        OperationId::new(5_127).unwrap(),
                        OperationId::new(5_128).unwrap(),
                    ],
                    [
                        ValueId::new(5_110).unwrap(),
                        ValueId::new(5_111).unwrap(),
                        ValueId::new(5_112).unwrap(),
                        ValueId::new(5_113).unwrap(),
                    ],
                    ObligationId::new(5_132).unwrap(),
                ),
            ]) {
                assert_eq!(block.instructions.len(), 3);
                assert_eq!(
                    block.instructions[0].kind,
                    TerminalSelectedInstructionKind::MaterializeI64 {
                        value: expected.0[0]
                    }
                );
                assert_eq!(
                    block.instructions[1].kind,
                    TerminalSelectedInstructionKind::MaterializeI64 {
                        value: expected.0[1]
                    }
                );
                let subtract = &block.instructions[2];
                assert!(matches!(
                    subtract.kind,
                    TerminalSelectedInstructionKind::ExactSubtractI64 { obligation, .. }
                        if obligation == expected.3
                ));
                assert_eq!(
                    subtract.constraint,
                    staged.register_environment().selected_keys().subtract_i64
                );
                assert_eq!(
                    subtract
                        .operands
                        .iter()
                        .map(|operand| (operand.virtual_register, operand.access))
                        .collect::<Vec<_>>(),
                    vec![
                        (
                            block.instructions[0].operands[0].virtual_register,
                            RegisterOperandAccess::Use
                        ),
                        (
                            block.instructions[1].operands[0].virtual_register,
                            RegisterOperandAccess::Use
                        ),
                        (
                            subtract.operands[2].virtual_register,
                            RegisterOperandAccess::Def,
                        ),
                    ]
                );
                assert_eq!(subtract.provenance.operations, expected.1);
                assert_eq!(subtract.provenance.values, expected.2);
                assert_eq!(subtract.provenance.obligations, vec![expected.3]);
                assert_eq!(
                    subtract.provenance.fuel,
                    expected
                        .1
                        .into_iter()
                        .map(|operation| FuelSettlement {
                            site: PsiProvenance::Operation(operation),
                            units: 1,
                        })
                        .collect::<Vec<_>>()
                );
                assert!(
                    subtract
                        .operands
                        .iter()
                        .all(|operand| operand.fixed_view.is_none())
                );
                assert!(
                    subtract
                        .operands
                        .iter()
                        .all(|operand| operand.tied_to.is_none())
                );
                if target.architecture == omega_target::Architecture::X86_64 {
                    assert!(!subtract.clobbers.is_empty());
                } else {
                    assert!(subtract.clobbers.is_empty());
                }
            }

            let selected_identity = staged.selected().receipt().identity();
            let mut swapped = staged.selected().plan().clone();
            swapped.functions[0].blocks[1].instructions[2]
                .operands
                .swap(0, 1);
            assert_ne!(
                terminal_selected_instruction_plan_identity(&swapped),
                selected_identity
            );
            assert!(matches!(
                validate_raw_selection(&staged, swapped),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
            ));

            let effects = stage_optimized_machine_effects(&staged).unwrap();
            assert_eq!(effects.custody().instruction_count(), 10);
            let subtracts = effects
                .effects()
                .plan()
                .functions
                .iter()
                .flat_map(|function| &function.blocks)
                .flat_map(|block| &block.instructions)
                .filter(|instruction| {
                    matches!(
                        instruction.kind,
                        TerminalSelectedInstructionKind::ExactSubtractI64 { .. }
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(subtracts.len(), 2);
            for subtract in subtracts {
                assert_eq!(subtract.barrier, TerminalMachineBarrier::None);
                assert_eq!(
                    subtract.alternatives.len(),
                    if target.architecture == omega_target::Architecture::X86_64 {
                        4
                    } else {
                        1
                    }
                );
                assert_eq!(subtract.provenance.operations.len(), 2);
                assert_eq!(subtract.provenance.values.len(), 4);
                assert_eq!(subtract.provenance.obligations.len(), 1);
                assert_eq!(subtract.provenance.fuel.len(), 2);
            }

            let liveness = stage_optimized_liveness(staged).unwrap();
            assert_eq!(liveness.custody().instruction_count(), 10);
            let live_function = &liveness.liveness().plan().functions[0];
            for (block, registers) in live_function.blocks[1..]
                .iter()
                .zip([[1_u32, 2, 3], [4, 5, 6]])
            {
                assert_eq!(block.instructions.len(), 4);
                assert_eq!(
                    block.instructions[2].virtual_uses,
                    registers[..2]
                        .iter()
                        .copied()
                        .map(TerminalVirtualRegisterId)
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    block.instructions[2].virtual_defs,
                    vec![TerminalVirtualRegisterId(registers[2])]
                );
                assert_eq!(
                    block.instructions[2].virtual_live_out,
                    vec![TerminalVirtualRegisterId(registers[2])]
                );
            }

            let ranges = stage_optimized_live_ranges(liveness).unwrap();
            assert_eq!(ranges.custody().virtual_register_count(), 7);
            assert_eq!(
                ranges.ranges().plan().functions[0]
                    .block_domains
                    .iter()
                    .map(|domain| (domain.block.0, domain.start.0, domain.end.0))
                    .collect::<Vec<_>>(),
                vec![(0, 0, 4), (1, 4, 12), (2, 12, 20)]
            );
            assert_eq!(ranges.ranges().plan().functions[0].interference.len(), 2);

            let legality = stage_optimized_allocation_legality(ranges).unwrap();
            assert_eq!(legality.custody().entry_transition_count(), 0);
            assert_eq!(legality.custody().function_count(), 1);
            let homes = stage_optimized_register_homes(legality).unwrap();
            assert_eq!(homes.custody().assignment_count(), 7);
            let selected_stage = homes
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage();
            let model = selected_stage.register_environment().physical().model();
            assert_eq!(
                homes.homes().plan().functions[0]
                    .assignments
                    .iter()
                    .map(|assignment| {
                        model
                            .views
                            .iter()
                            .find(|view| view.id == assignment.view)
                            .unwrap()
                            .name
                            .as_str()
                    })
                    .collect::<Vec<_>>(),
                expected_homes
            );
            let post = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
            assert_eq!(post.custody().instruction_count(), 10);
            let post_subtracts = post
                .machine()
                .plan()
                .functions
                .iter()
                .flat_map(|function| &function.blocks)
                .flat_map(|block| &block.instructions)
                .filter(|instruction| {
                    instruction.alternative.key.family
                        == omega_terminal_selected_instructions::TerminalMachineAlternativeFamily::ExactSubtractI64
                })
                .collect::<Vec<_>>();
            assert_eq!(post_subtracts.len(), 2);
            assert!(post_subtracts.iter().all(|instruction| {
                instruction.alternative.key.variant == expected_alternative
            }));
            assert_eq!(
                &validate_optimized_post_allocation_machine_plan_custody(&homes, &post).unwrap(),
                post.custody()
            );
        }
    }

    #[test]
    fn legalization_replay_rejects_foreign_proof_fact_and_leaf_operation_custody() {
        let staged = staged_exact_add_conditional(NativeTarget::linux_x64());
        let original = staged.legalized().plan();
        let validate = |plan| {
            validate_terminal_legalized_operations(
                staged.optimized_target().target_operations(),
                staged.optimized_target().optimized().plan(),
                staged.optimized_target().optimized().unit(),
                plan,
            )
        };

        let mut corrupted = original.clone();
        let false_fact = match corrupted.functions[0].when_false.value {
            omega_terminal_legalized_operations::TerminalLegalizedLeafValue::ExactAdd {
                accepted_fact,
                ..
            } => accepted_fact,
            _ => panic!("exact-add fixture must retain its admitted fact"),
        };
        let omega_terminal_legalized_operations::TerminalLegalizedLeafValue::ExactAdd {
            accepted_fact,
            ..
        } = &mut corrupted.functions[0].when_true.value
        else {
            panic!("exact-add fixture must retain its admitted fact")
        };
        *accepted_fact = false_fact;
        assert_eq!(
            validate(corrupted),
            Err(TerminalLegalizationError::NonCanonicalLegalizedPlan)
        );

        let mut corrupted = original.clone();
        let omega_terminal_legalized_operations::TerminalLegalizedLeafValue::ExactAdd {
            left,
            right,
            ..
        } = &mut corrupted.functions[0].when_true.value
        else {
            panic!("exact-add fixture must retain its inputs")
        };
        std::mem::swap(&mut left.constant_operation, &mut right.constant_operation);
        assert_eq!(
            validate(corrupted),
            Err(TerminalLegalizationError::NonCanonicalLegalizedPlan)
        );
    }

    #[test]
    fn exact_add_selection_retains_proof_policy_and_target_constraints() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let staged = staged_exact_add_conditional(target);
            assert_eq!(
                staged.legalized().plan().functions[0].recipe,
                TerminalLegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1
            );
            let plan = staged.selected().plan();
            let function = &plan.functions[0];
            assert_eq!(function.virtual_registers.len(), 7);
            assert_eq!(staged.selected().receipt().instruction_count(), 10);
            let accepted = &staged
                .optimized_target()
                .optimized()
                .unit()
                .accepted_obligation_facts;
            assert_eq!(accepted.len(), 2);
            for (block, expected_obligation) in function.blocks[1..].iter().zip([
                ObligationId::new(5_031).unwrap(),
                ObligationId::new(5_032).unwrap(),
            ]) {
                assert_eq!(block.instructions.len(), 3);
                let add = &block.instructions[2];
                let TerminalSelectedInstructionKind::ExactAddI64 {
                    obligation,
                    accepted_fact,
                } = add.kind
                else {
                    panic!("leaf arithmetic must retain exact-add semantics")
                };
                assert_eq!(obligation, expected_obligation);
                let fact = accepted
                    .iter()
                    .find(|fact| fact.identity == accepted_fact)
                    .expect("selected fact must remain verifier-owned");
                assert_eq!(fact.operation, add.provenance.operations[0]);
                assert_eq!(fact.obligation, obligation);
                assert_eq!(
                    add.constraint,
                    staged.register_environment().selected_keys().add_i64
                );
                assert_eq!(
                    add.operands
                        .iter()
                        .map(|operand| operand.access)
                        .collect::<Vec<_>>(),
                    vec![
                        RegisterOperandAccess::Use,
                        RegisterOperandAccess::Use,
                        RegisterOperandAccess::Def,
                    ]
                );
                assert!(
                    add.operands
                        .iter()
                        .all(|operand| operand.fixed_view.is_none())
                );
                assert!(add.operands.iter().all(|operand| operand.tied_to.is_none()));
                assert!(add.implicit_uses.is_empty());
                assert!(add.implicit_defs.is_empty());
                assert!(add.clobbers.is_empty());
                assert_eq!(add.provenance.operations.len(), 1);
                assert_eq!(add.provenance.values.len(), 3);
                assert_eq!(add.provenance.obligations, vec![obligation]);
                assert_eq!(add.provenance.fuel.len(), 1);
            }

            let original_identity = staged.selected().receipt().identity();
            let mut corrupted = plan.clone();
            let TerminalSelectedInstructionKind::ExactAddI64 { obligation, .. } =
                &mut corrupted.functions[0].blocks[1].instructions[2].kind
            else {
                unreachable!()
            };
            *obligation = ObligationId::new(9_501).unwrap();
            assert_ne!(
                terminal_selected_instruction_plan_identity(&corrupted),
                original_identity
            );
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
            ));

            let mut corrupted = plan.clone();
            let false_fact = match corrupted.functions[0].blocks[2].instructions[2].kind {
                TerminalSelectedInstructionKind::ExactAddI64 { accepted_fact, .. } => accepted_fact,
                _ => unreachable!(),
            };
            let TerminalSelectedInstructionKind::ExactAddI64 { accepted_fact, .. } =
                &mut corrupted.functions[0].blocks[1].instructions[2].kind
            else {
                unreachable!()
            };
            *accepted_fact = false_fact;
            assert_ne!(
                terminal_selected_instruction_plan_identity(&corrupted),
                original_identity
            );
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
            ));

            let mut corrupted = plan.clone();
            corrupted.functions[0].blocks[1].instructions[2]
                .provenance
                .obligations[0] = ObligationId::new(9_502).unwrap();
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
            ));

            let mut corrupted = plan.clone();
            corrupted.functions[0].blocks[1].instructions[2]
                .operands
                .swap(0, 1);
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
            ));

            let mut corrupted = plan.clone();
            corrupted.functions[0].blocks[1].instructions[2].constraint =
                staged.register_environment().selected_keys().copy_i64;
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                    | Err(SelectedInstructionError::ConstraintOperandMismatch { .. })
            ));

            let mut corrupted = plan.clone();
            corrupted.functions[0].blocks[1].instructions[2]
                .provenance
                .operations[0] = OperationId::new(9_503).unwrap();
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
            ));

            let mut corrupted = plan.clone();
            corrupted.functions[0].blocks[1].instructions[2]
                .provenance
                .fuel[0]
                .units += 1;
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                    | Err(SelectedInstructionError::ProvenancePartitionMismatch { .. })
            ));
        }
    }

    #[test]
    fn exact_subtract_retains_proof_target_effects_and_reaches_homes() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let staged = staged_exact_subtract_conditional(target);
            assert_eq!(
                staged.legalized().plan().functions[0].recipe,
                TerminalLegalizationRecipe::ReturnU64ExactSubtractImmediateConditionalV1
            );
            let plan = staged.selected().plan();
            assert_eq!(plan.functions[0].virtual_registers.len(), 7);
            assert_eq!(staged.selected().receipt().instruction_count(), 10);
            let accepted = &staged
                .optimized_target()
                .optimized()
                .unit()
                .accepted_obligation_facts;
            for (block, expected_obligation) in plan.functions[0].blocks[1..].iter().zip([
                ObligationId::new(5_031).unwrap(),
                ObligationId::new(5_032).unwrap(),
            ]) {
                let subtract = &block.instructions[2];
                let TerminalSelectedInstructionKind::ExactSubtractI64 {
                    obligation,
                    accepted_fact,
                } = subtract.kind
                else {
                    panic!("leaf arithmetic must retain exact-subtract semantics")
                };
                assert_eq!(obligation, expected_obligation);
                let fact = accepted
                    .iter()
                    .find(|fact| fact.identity == accepted_fact)
                    .expect("selected fact must remain verifier-owned");
                assert_eq!(fact.operation, subtract.provenance.operations[0]);
                assert_eq!(fact.obligation, obligation);
                assert_eq!(
                    subtract.constraint,
                    staged.register_environment().selected_keys().subtract_i64
                );
                assert_eq!(
                    subtract
                        .operands
                        .iter()
                        .map(|operand| operand.access)
                        .collect::<Vec<_>>(),
                    vec![
                        RegisterOperandAccess::Use,
                        RegisterOperandAccess::Use,
                        RegisterOperandAccess::Def,
                    ]
                );
                assert!(subtract.implicit_uses.is_empty());
                assert!(subtract.implicit_defs.is_empty());
                if target.architecture == omega_target::Architecture::X86_64 {
                    assert!(!subtract.clobbers.is_empty());
                } else {
                    assert!(subtract.clobbers.is_empty());
                }
                assert_eq!(subtract.provenance.obligations, vec![obligation]);
                assert_eq!(subtract.provenance.fuel.len(), 1);
            }

            let identity = staged.selected().receipt().identity();
            let mut corrupted = plan.clone();
            let TerminalSelectedInstructionKind::ExactSubtractI64 { obligation, .. } =
                &mut corrupted.functions[0].blocks[1].instructions[2].kind
            else {
                unreachable!()
            };
            *obligation = ObligationId::new(9_504).unwrap();
            assert_ne!(
                terminal_selected_instruction_plan_identity(&corrupted),
                identity
            );
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
            ));

            let mut corrupted = plan.clone();
            let TerminalSelectedInstructionKind::ExactSubtractI64 {
                obligation,
                accepted_fact,
            } = corrupted.functions[0].blocks[1].instructions[2].kind
            else {
                unreachable!()
            };
            corrupted.functions[0].blocks[1].instructions[2].kind =
                TerminalSelectedInstructionKind::ExactAddI64 {
                    obligation,
                    accepted_fact,
                };
            assert_ne!(
                terminal_selected_instruction_plan_identity(&corrupted),
                identity
            );
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
            ));

            let homes = stage_optimized_register_homes(
                stage_optimized_allocation_legality(
                    stage_optimized_live_ranges(
                        stage_optimized_liveness(staged).expect("subtract liveness"),
                    )
                    .expect("subtract ranges"),
                )
                .expect("subtract legality"),
            )
            .expect("subtract homes");
            assert_eq!(homes.custody().assignment_count(), 7);
            let post = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
            assert_eq!(post.custody().instruction_count(), 10);
            let decoded_post = omega_machine_optimizer::TerminalPostAllocationMachinePlan::decode(
                &post.machine().plan().encode(),
            )
            .unwrap();
            assert_eq!(&decoded_post, post.machine().plan());
            assert_eq!(
                validate_raw_post_allocation(&homes, &post, decoded_post.clone())
                    .unwrap()
                    .receipt(),
                post.machine().receipt()
            );
            assert_eq!(
                post.custody().source(),
                &StagedOptimizedPostAllocationMachineSourceCustodyReceipt::RegisterHomes(
                    homes.custody()
                )
            );
            assert_eq!(
                &validate_optimized_post_allocation_machine_plan_custody(&homes, &post).unwrap(),
                post.custody()
            );
            let selected_stage = homes
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage();
            let encodings = stage_optimized_layout_independent_selected_form_encoding(
                selected_stage.selected(),
                &post,
                selected_stage.register_environment().physical(),
            )
            .unwrap();
            assert_eq!(encodings.selected(), post.machine().receipt().selected());
            assert_eq!(encodings.machine(), post.machine().receipt().identity());
            assert_eq!(encodings.rows().len(), 10);
            assert_eq!(
                encodings
                    .rows()
                    .iter()
                    .filter(|row| matches!(
                        row.state,
                        TerminalSelectedFormEncodingState::DeferredControl { .. }
                    ))
                    .count(),
                1
            );
            assert!(encodings.rows().iter().all(|row| match &row.state {
                TerminalSelectedFormEncodingState::Encoded { bytes, .. } => !bytes.is_empty(),
                TerminalSelectedFormEncodingState::DeferredControl { .. } => true,
            }));
            let returns = encodings
                .rows()
                .iter()
                .filter(|row| {
                    row.alternative.family
                        == omega_terminal_selected_instructions::TerminalMachineAlternativeFamily::ReturnI64
                })
                .collect::<Vec<_>>();
            assert_eq!(returns.len(), 2);
            for returned in returns {
                let TerminalSelectedFormEncodingState::Encoded { bytes, footprint } =
                    &returned.state
                else {
                    panic!("returns have layout-independent target encodings")
                };
                assert_eq!(
                    bytes.as_slice(),
                    if target.architecture == omega_target::Architecture::X86_64 {
                        &[0xc3][..]
                    } else {
                        &[0xc0, 0x03, 0x5f, 0xd6][..]
                    }
                );
                assert!(footprint.register_reads.is_empty());
                assert!(footprint.register_writes.is_empty());
                assert!(footprint.encoded.external_operand_reads.is_empty());
                assert!(footprint.encoded.external_operand_writes.is_empty());
            }
            validate_optimized_layout_independent_selected_form_encoding(
                selected_stage.selected(),
                &post,
                selected_stage.register_environment().physical(),
                &encodings,
            )
            .unwrap();
            let layout = stage_optimized_resolved_selected_form_layout(
                selected_stage.selected(),
                &post,
                selected_stage.register_environment().physical(),
                &encodings,
            )
            .unwrap();
            assert_eq!(
                layout.policy(),
                TerminalSelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1
            );
            assert_eq!(layout.pre_layout(), encodings.identity());
            assert_eq!(layout.functions().len(), 1);
            let selected_function = &selected_stage.selected().plan().functions[0];
            let TerminalSelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } = &selected_function
                .blocks
                .iter()
                .find(|block| block.id == selected_function.entry_block)
                .unwrap()
                .terminator
            else {
                panic!("fixture entry is conditional")
            };
            let function_layout = &layout.functions()[0];
            assert_eq!(
                function_layout
                    .blocks
                    .iter()
                    .map(|block| block.block)
                    .collect::<Vec<_>>(),
                [
                    selected_function.entry_block,
                    when_zero.block,
                    when_nonzero.block
                ]
            );
            assert!(
                function_layout
                    .blocks
                    .windows(2)
                    .all(|pair| pair[0].offset + pair[0].byte_count == pair[1].offset)
            );
            let branch = function_layout
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .find_map(|row| row.branch.as_deref().map(|branch| (row, branch)))
                .expect("one resolved branch");
            assert_eq!(branch.1.when_zero_block, when_zero.block);
            assert_eq!(branch.1.when_nonzero_block, when_nonzero.block);
            assert_eq!(
                branch.0.offset + u64::try_from(branch.0.bytes.len()).unwrap(),
                branch.1.when_zero_offset
            );
            match target.architecture {
                omega_target::Architecture::X86_64 => {
                    assert_eq!(&branch.0.bytes[..2], [0x0f, 0x85]);
                    assert_eq!(
                        branch.1.byte_displacement,
                        i64::try_from(branch.1.when_nonzero_offset).unwrap()
                            - i64::try_from(branch.0.offset + 6).unwrap()
                    );
                }
                omega_target::Architecture::Aarch64 => {
                    assert_eq!(branch.0.bytes[0] & 0x1f, 1);
                    assert_eq!(
                        branch.1.byte_displacement,
                        i64::try_from(branch.1.when_nonzero_offset).unwrap()
                            - i64::try_from(branch.0.offset).unwrap()
                    );
                }
            }
            validate_optimized_resolved_selected_form_layout(
                selected_stage.selected(),
                &post,
                selected_stage.register_environment().physical(),
                &encodings,
                &layout,
            )
            .unwrap();
            let mut corrupted_layout = layout.clone();
            corrupted_layout.functions_mut()[0].blocks[0]
                .instructions
                .last_mut()
                .unwrap()
                .bytes[0] ^= 1;
            assert_eq!(
                validate_optimized_resolved_selected_form_layout(
                    selected_stage.selected(),
                    &post,
                    selected_stage.register_environment().physical(),
                    &encodings,
                    &corrupted_layout,
                ),
                Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)
            );
            let subtracts = post
                .machine()
                .plan()
                .functions
                .iter()
                .flat_map(|function| &function.blocks)
                .flat_map(|block| &block.instructions)
                .filter(|instruction| {
                    instruction.alternative.key.family
                        == omega_terminal_selected_instructions::TerminalMachineAlternativeFamily::ExactSubtractI64
                })
                .collect::<Vec<_>>();
            assert_eq!(subtracts.len(), 2);
            assert!(subtracts.iter().all(|instruction| {
                instruction.operands.len() == 3
                    && instruction
                        .unit_uses
                        .windows(2)
                        .all(|pair| pair[0] < pair[1])
                    && instruction
                        .unit_defs
                        .windows(2)
                        .all(|pair| pair[0] < pair[1])
                    && instruction
                        .operands
                        .iter()
                        .filter(|operand| operand.write_semantics.is_some())
                        .all(|operand| !operand.write_units.is_empty())
            }));
            let mut corrupted = decoded_post;
            let subtract = corrupted.functions[0]
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find(|instruction| {
                    instruction.alternative.key.family
                        == omega_terminal_selected_instructions::TerminalMachineAlternativeFamily::ExactSubtractI64
                })
                .unwrap();
            subtract.alternative.key.variant = u32::MAX;
            corrupted.identity =
                omega_machine_optimizer::terminal_post_allocation_machine_identity(&corrupted);
            assert!(matches!(
                validate_raw_post_allocation(&homes, &post, corrupted),
                Err(
                    omega_machine_optimizer::TerminalPostAllocationMachineError::InstructionMismatch {
                        ..
                    }
                )
            ));

            let mut corrupted = post.machine().plan().clone();
            corrupted.functions[0]
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .flat_map(|instruction| &mut instruction.operands)
                .find(|operand| operand.write_semantics.is_some())
                .unwrap()
                .write_units
                .clear();
            corrupted.identity =
                omega_machine_optimizer::terminal_post_allocation_machine_identity(&corrupted);
            assert!(matches!(
                validate_raw_post_allocation(&homes, &post, corrupted),
                Err(
                    omega_machine_optimizer::TerminalPostAllocationMachineError::InstructionMismatch {
                        ..
                    }
                )
            ));

            let mut corrupted = post.machine().plan().clone();
            corrupted.effects =
                omega_machine_optimizer::TerminalPreAllocationMachineEffectIdentity::from_bytes(
                    [0x5a; 32],
                );
            corrupted.identity =
                omega_machine_optimizer::terminal_post_allocation_machine_identity(&corrupted);
            assert_eq!(
                validate_raw_post_allocation(&homes, &post, corrupted),
                Err(
                    omega_machine_optimizer::TerminalPostAllocationMachineError::EffectRootMismatch
                )
            );
        }
    }

    #[test]
    fn machine_effect_sidecar_reconstructs_subtraction_and_control_barriers() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let selected = staged_exact_subtract_conditional(target);
            let staged = stage_optimized_machine_effects(&selected).unwrap();
            assert_eq!(staged.custody().instruction_count(), 10);
            assert_eq!(
                staged.custody().source(),
                &StagedOptimizedMachineEffectSourceCustodyReceipt::Selected(selected.custody())
            );
            assert_eq!(
                &validate_optimized_machine_effect_custody(&selected, staged.effects()).unwrap(),
                staged.custody()
            );
            let instructions = staged
                .effects()
                .plan()
                .functions
                .iter()
                .flat_map(|function| &function.blocks)
                .flat_map(|block| &block.instructions)
                .collect::<Vec<_>>();
            assert_eq!(
                instructions
                    .iter()
                    .filter(|instruction| {
                        instruction.barrier
                        == omega_terminal_selected_instructions::TerminalMachineBarrier::ControlFlow
                    })
                    .count(),
                3
            );
            let subtracts = instructions
                .iter()
                .filter(|instruction| {
                    matches!(
                        instruction.kind,
                        TerminalSelectedInstructionKind::ExactSubtractI64 { .. }
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(subtracts.len(), 2);
            for subtract in subtracts {
                assert_eq!(
                    subtract.alternatives.len(),
                    if target.architecture == omega_target::Architecture::X86_64 {
                        4
                    } else {
                        1
                    }
                );
                assert_eq!(
                    subtract.unit_clobbers.is_empty(),
                    target.architecture != omega_target::Architecture::X86_64
                );
                assert_eq!(subtract.provenance.obligations.len(), 1);
                assert_eq!(subtract.provenance.fuel.len(), 1);
            }

            let mut corrupted = staged.effects().plan().clone();
            corrupted.functions[0].blocks[1].instructions[2]
                .alternatives
                .clear();
            assert!(matches!(
                omega_machine_optimizer::validate_terminal_pre_allocation_machine_effects(
                    selected.selected(),
                    selected.register_environment().identity(),
                    selected.register_environment().physical(),
                    selected.register_environment().constraints(),
                    selected.register_environment().reservations(),
                    selected.register_environment().allocation_constraint_keys(),
                    &match target.architecture {
                        omega_target::Architecture::X86_64 => {
                            omega_terminal_isa_x86_64::validate_x86_64_terminal_machine_effect_catalog(
                                target,
                                selected.register_environment().constraints(),
                                omega_terminal_isa_x86_64::x86_64_terminal_machine_effect_catalog(
                                    target,
                                    selected.register_environment().constraints(),
                                )
                                .unwrap(),
                            )
                            .unwrap()
                        }
                        omega_target::Architecture::Aarch64 => {
                            omega_terminal_isa_aarch64::validate_aarch64_terminal_machine_effect_catalog(
                                target,
                                selected.register_environment().constraints(),
                                omega_terminal_isa_aarch64::aarch64_terminal_machine_effect_catalog(
                                    target,
                                    selected.register_environment().constraints(),
                                )
                                .unwrap(),
                            )
                            .unwrap()
                        }
                    },
                    corrupted,
                ),
                Err(
                    omega_machine_optimizer::TerminalMachineEffectError::InstructionMismatch { .. }
                )
            ));
        }
    }

    #[test]
    fn exact_add_pressure_reaches_deterministic_homes_on_both_architectures() {
        for (target, expected_homes) in [
            (
                NativeTarget::linux_x64(),
                ["rdi", "rax", "rbx", "rax", "rax", "rbx", "rax"],
            ),
            (
                NativeTarget::linux_arm64(),
                ["x0", "x0", "x1", "x0", "x0", "x1", "x0"],
            ),
        ] {
            let legality = stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_exact_add_conditional(target)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            let ranges = legality.live_range_stage();
            let selected = ranges.liveness_stage().selected_stage();
            let environment = selected.register_environment();
            let choices = choose_terminal_spill_victims(
                legality.legality(),
                ranges.ranges(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                OptimizationWorkBudget::new(100, 100, 1_000, 100, 1).unwrap(),
            )
            .unwrap();
            assert!(
                choices
                    .plan()
                    .functions
                    .iter()
                    .all(|function| function.choice.is_none())
            );
            let recovery = classify_terminal_pressure_recovery(
                selected.selected(),
                ranges.ranges(),
                legality.legality(),
                &choices,
                TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                OptimizationWorkBudget::new(100, 100, 1_000, 100, 1).unwrap(),
            )
            .unwrap();
            assert!(
                recovery
                    .plan()
                    .functions
                    .iter()
                    .all(|function| function.classification.is_none())
            );
            assert_eq!(recovery.receipt().selected(), selected.custody().selected());
            assert_eq!(recovery.receipt().ranges(), ranges.custody().ranges());
            assert_eq!(recovery.receipt().legality(), legality.custody().legality());
            assert_eq!(
                recovery.receipt().spill_choices(),
                choices.receipt().identity()
            );
            let staged = stage_optimized_register_homes(legality).unwrap();
            let post = stage_optimized_post_allocation_machine_plan(&staged).unwrap();
            assert_eq!(
                post.machine().receipt().selected(),
                staged.custody().selected()
            );
            assert!(post.machine().plan().functions.iter().all(|function| {
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .all(|instruction| instruction.alternative.key.variant == 0)
            }));
            let legality_stage = staged.legality_stage();
            let ranges_stage = legality_stage.live_range_stage();
            let liveness_stage = ranges_stage.liveness_stage();
            let liveness = &liveness_stage.liveness().plan().functions[0];
            for (block, registers) in liveness.blocks[1..].iter().zip([[1_u32, 2, 3], [4, 5, 6]]) {
                assert_eq!(block.instructions.len(), 4);
                assert_eq!(
                    block.instructions[2].virtual_uses,
                    registers[..2]
                        .iter()
                        .copied()
                        .map(TerminalVirtualRegisterId)
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    block.instructions[2].virtual_defs,
                    vec![TerminalVirtualRegisterId(registers[2])]
                );
                assert_eq!(
                    block.instructions[2].virtual_live_out,
                    vec![TerminalVirtualRegisterId(registers[2])]
                );
            }

            let ranges = &ranges_stage.ranges().plan().functions[0];
            assert_eq!(
                ranges
                    .block_domains
                    .iter()
                    .map(|domain| (domain.block.0, domain.start.0, domain.end.0))
                    .collect::<Vec<_>>(),
                vec![(0, 0, 4), (1, 4, 12), (2, 12, 20)]
            );
            assert_eq!(
                ranges.interference,
                vec![
                    TerminalVirtualInterference {
                        lower: TerminalVirtualRegisterId(1),
                        higher: TerminalVirtualRegisterId(2),
                    },
                    TerminalVirtualInterference {
                        lower: TerminalVirtualRegisterId(4),
                        higher: TerminalVirtualRegisterId(5),
                    },
                ]
            );
            assert!(
                ranges
                    .virtual_registers
                    .iter()
                    .all(|register| register.edge_connectors.is_empty())
            );
            assert_eq!(legality_stage.custody().entry_transition_count(), 0);

            let environment = liveness_stage.selected_stage().register_environment();
            let model = environment.physical().model();
            let homes = &staged.homes().plan().functions[0];
            assert_eq!(homes.assignments.len(), 7);
            assert_eq!(
                homes
                    .assignments
                    .iter()
                    .map(|assignment| {
                        model
                            .views
                            .iter()
                            .find(|view| view.id == assignment.view)
                            .unwrap()
                            .name
                            .as_str()
                    })
                    .collect::<Vec<_>>(),
                expected_homes
            );
            assert_eq!(homes.assignments[1].view, homes.assignments[4].view);
            assert_eq!(homes.assignments[2].view, homes.assignments[5].view);
            assert_ne!(homes.assignments[1].view, homes.assignments[2].view);
        }
    }

    #[test]
    fn active_resident_multi_use_rematerialization_reaches_fresh_homes_on_both_architectures() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let source = staged_active_resident_two_view_legality(target);
            assert_eq!(
                source
                    .live_range_stage()
                    .liveness_stage()
                    .selected_stage()
                    .legalized()
                    .plan()
                    .functions[0]
                    .recipe,
                TerminalLegalizationRecipe::ReturnU64ActiveResidentExactAddChainConditionalV1
            );
            let source_selected = source
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected()
                .plan()
                .clone();
            let source_resident = source_selected.functions[0].blocks[1].instructions[0].clone();
            assert_eq!(source_resident.id.0, 2);
            assert!(matches!(
                source_resident.kind,
                TerminalSelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(3)
                }
            ));

            let staged = stage_optimized_active_resident_rematerialization(
                source,
                TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
                selected_lowering_budget(),
            )
            .unwrap();
            assert_eq!(
                validate_optimized_active_resident_rematerialization(&staged).unwrap(),
                staged.custody()
            );
            let choice = staged.choices().plan().functions[0]
                .choice
                .as_ref()
                .unwrap();
            assert_eq!(choice.incoming, TerminalVirtualRegisterId(3));
            assert_eq!(choice.selected_victim, TerminalVirtualRegisterId(1));
            let classification = staged.classifications().plan().functions[0]
                .classification
                .as_ref()
                .unwrap();
            assert_eq!(classification.victim, TerminalVirtualRegisterId(1));
            assert!(matches!(
                classification.role,
                TerminalRecoveryVictimRole::ActiveResident { .. }
            ));
            let TerminalRecoveryClassification::ImmediateU64RematerializationCandidate {
                defining_instruction,
                source_value,
                value,
                provenance,
                future_uses,
            } = &classification.classification
            else {
                panic!("active resident must retain literal eligibility")
            };
            assert_eq!(*defining_instruction, source_resident.id);
            assert_eq!(*source_value, ValueId::new(5_206).unwrap());
            assert_eq!(*value, IntegerValue::Unsigned(3));
            assert_eq!(provenance, &source_resident.provenance);
            assert_eq!(future_uses.len(), 2);

            let action = staged.rematerialization().plan().functions[0]
                .action
                .as_ref()
                .unwrap();
            assert_eq!(action.victim, TerminalVirtualRegisterId(1));
            assert_eq!(action.original_materialize, source_resident.id);
            assert_eq!(action.rewrites.len(), 2);
            assert_eq!(
                staged.rematerialization().receipt().rewritten_use_count(),
                2
            );
            assert_eq!(staged.rematerialization().receipt().applied_count(), 1);
            let transformed = staged.rematerialization().transformed();
            assert_eq!(
                transformed.functions[0].blocks[1].instructions[0],
                source_resident
            );
            let fresh = transformed.functions[0].blocks[1]
                .instructions
                .iter()
                .find(|instruction| instruction.id == action.fresh_materialize)
                .unwrap();
            assert!(fresh.provenance.operations.is_empty());
            assert!(fresh.provenance.edges.is_empty());
            assert!(fresh.provenance.obligations.is_empty());
            assert!(fresh.provenance.fuel.is_empty());
            assert_eq!(fresh.provenance.values, vec![ValueId::new(5_206).unwrap()]);
            let rewritten_uses = transformed.functions[0].blocks[1]
                .instructions
                .iter()
                .flat_map(|instruction| &instruction.operands)
                .filter(|operand| operand.virtual_register == action.result_virtual_register)
                .count();
            assert_eq!(rewritten_uses, 3);
            assert_ne!(
                staged.liveness().receipt().identity(),
                staged.source().custody().liveness()
            );
            assert_ne!(
                staged.ranges().receipt().identity(),
                staged.source().custody().ranges()
            );
            assert_ne!(
                staged.legality().receipt().identity(),
                staged.source().custody().legality()
            );
            assert_eq!(staged.legality().receipt().entry_transition_count(), 0);
            assert_eq!(
                staged.homes().receipt().ranges(),
                staged.ranges().receipt().identity()
            );
            assert_eq!(
                staged.homes().receipt().legality(),
                staged.legality().receipt().identity()
            );
            assert_eq!(staged.homes().receipt().assignment_count(), 9);
            assert_eq!(
                staged
                    .post_allocation_manifest()
                    .record()
                    .selected_transformations,
                vec![
                    PostAllocationSelectedTransformation::PressureRematerialization(
                        staged.rematerialization().receipt().identity()
                    )
                ]
            );
            assert_eq!(
                staged.post_allocation_manifest().record().selected,
                staged.rematerialization().receipt().transformed_selected()
            );
        }
    }

    #[test]
    fn active_resident_rematerialization_reaches_machine_custody_on_both_architectures() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let source = stage_optimized_active_resident_rematerialization(
                staged_active_resident_two_view_legality(target),
                TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
                selected_lowering_budget(),
            )
            .unwrap();
            let source_selected = source
                .source()
                .live_range_stage()
                .liveness_stage()
                .selected_stage();
            let transformed_selected = source.rematerialization().receipt().transformed_selected();

            let effects =
                stage_optimized_machine_effects_after_active_resident_rematerialization(&source)
                    .unwrap();
            assert_eq!(effects.effects().receipt().selected(), transformed_selected);
            assert_eq!(
                effects.effects().plan().optimization_unit,
                source.custody().source().optimization_unit()
            );
            assert_eq!(
                effects.effects().plan().fuel_schedule,
                source.custody().source().fuel_schedule()
            );
            assert_eq!(effects.effects().plan().target, target);
            assert_eq!(
                effects.effects().receipt().register_environment(),
                source_selected.register_environment().identity()
            );
            assert_eq!(
                effects.custody().source(),
                &StagedOptimizedMachineEffectSourceCustodyReceipt::ActiveResidentRematerialization(
                    source.custody()
                )
            );
            assert_eq!(
                &validate_optimized_machine_effect_custody_after_active_resident_rematerialization(
                    &source,
                    effects.effects(),
                )
                .unwrap(),
                effects.custody()
            );

            let post =
                stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization(
                    &source,
                )
                .unwrap();
            assert_eq!(post.machine().receipt().selected(), transformed_selected);
            assert_eq!(
                post.machine().receipt().effects(),
                post.effects().effects().receipt().identity()
            );
            assert_eq!(
                post.machine().receipt().homes(),
                source.homes().receipt().identity()
            );
            assert_eq!(
                post.machine().receipt().post_allocation_manifest(),
                source.post_allocation_manifest().record().identity
            );
            assert_eq!(
                post.machine().receipt().register_environment(),
                source_selected.register_environment().identity()
            );
            assert_eq!(
                post.custody().source(),
                &StagedOptimizedPostAllocationMachineSourceCustodyReceipt::ActiveResidentRematerialization(
                    source.custody()
                )
            );
            assert_eq!(
                &validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody(
                    &source,
                    &post,
                )
                .unwrap(),
                post.custody()
            );

            assert_eq!(
                omega_machine_optimizer::validate_terminal_post_allocation_machine_plan(
                    source_selected.selected(),
                    post.effects().effects(),
                    source.ranges(),
                    source.legality(),
                    source.homes(),
                    source.post_allocation_manifest(),
                    source_selected.register_environment().identity(),
                    source_selected.register_environment().physical(),
                    source_selected.register_environment().constraints(),
                    post.machine().plan().clone(),
                ),
                Err(omega_machine_optimizer::TerminalPostAllocationMachineError::SelectedRootMismatch)
            );
        }

        let mut corrupted = stage_optimized_active_resident_rematerialization(
            staged_active_resident_two_view_legality(NativeTarget::linux_x64()),
            TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            selected_lowering_budget(),
        )
        .unwrap();
        crate::active_resident_rematerialization::corrupt_active_resident_rematerialization_custody_for_test(
            &mut corrupted,
        );
        assert!(matches!(
            stage_optimized_machine_effects_after_active_resident_rematerialization(&corrupted),
            Err(
                OptimizedMachineEffectPipelineError::ActiveResidentRematerialization(
                    OptimizedActiveResidentRematerializationError::ReceiptMismatch
                )
            )
        ));
        assert!(matches!(
            stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization(
                &corrupted,
            ),
            Err(
                OptimizedPostAllocationMachinePipelineError::ActiveResidentRematerialization(
                    OptimizedActiveResidentRematerializationError::ReceiptMismatch
                )
            )
        ));

        let x86 = stage_optimized_active_resident_rematerialization(
            staged_active_resident_two_view_legality(NativeTarget::linux_x64()),
            TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            selected_lowering_budget(),
        )
        .unwrap();
        let arm = stage_optimized_active_resident_rematerialization(
            staged_active_resident_two_view_legality(NativeTarget::linux_arm64()),
            TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            selected_lowering_budget(),
        )
        .unwrap();
        let x86_post =
            stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization(
                &x86,
            )
            .unwrap();
        assert!(
            validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody(
                &arm,
                &x86_post,
            )
            .is_err()
        );
    }

    #[test]
    fn active_resident_rematerialization_reaches_layout_independent_encoding_on_both_architectures()
    {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let (source, machine) = staged_active_resident_rematerialization_and_machine(target);
            let transformed_selected = source.rematerialization().receipt().transformed_selected();
            let machine_root = machine.machine().receipt().identity();
            let machine_row_count = machine.custody().instruction_count();
            let rematerialization = source.custody();
            let fresh_materialize = source.rematerialization().plan().functions[0]
                .action
                .as_ref()
                .unwrap()
                .fresh_materialize;

            let staged = stage_optimized_active_resident_rematerialization_selected_form_encoding(
                source, machine,
            )
            .unwrap();
            assert_eq!(staged.encoding().selected(), transformed_selected);
            assert_eq!(staged.encoding().machine(), machine_root);
            assert_eq!(staged.custody().rematerialization(), rematerialization);
            assert_eq!(staged.custody().machine(), staged.machine().custody());
            assert_eq!(
                staged.custody().transformed_selected(),
                transformed_selected
            );
            assert_eq!(staged.custody().encoding(), staged.encoding().identity());
            assert_eq!(staged.custody().row_count(), machine_row_count);
            assert_eq!(
                staged.custody().encoded_count() + staged.custody().deferred_count(),
                machine_row_count
            );
            assert_eq!(staged.custody().deferred_count(), 1);
            assert!(staged.encoding().rows().iter().all(|row| match &row.state {
                TerminalSelectedFormEncodingState::Encoded { bytes, .. } => !bytes.is_empty(),
                TerminalSelectedFormEncodingState::DeferredControl { .. } => true,
            }));
            let fresh_row = staged
                .encoding()
                .rows()
                .iter()
                .find(|row| row.instruction == fresh_materialize)
                .expect("fresh rematerialization must reach the encoder roster");
            assert_eq!(
                fresh_row.alternative.family,
                omega_terminal_selected_instructions::TerminalMachineAlternativeFamily::MaterializeI64
            );
            assert!(matches!(
                &fresh_row.state,
                TerminalSelectedFormEncodingState::Encoded { bytes, .. } if !bytes.is_empty()
            ));
            assert_eq!(
                validate_optimized_active_resident_rematerialization_selected_form_encoding(
                    &staged,
                )
                .unwrap(),
                staged.custody().clone()
            );
        }
    }

    #[test]
    fn active_resident_rematerialization_encoding_rejects_detached_or_corrupt_custody() {
        let (mut corrupt_source, machine) =
            staged_active_resident_rematerialization_and_machine(NativeTarget::linux_x64());
        crate::active_resident_rematerialization::corrupt_active_resident_rematerialization_custody_for_test(
            &mut corrupt_source,
        );
        assert!(matches!(
            stage_optimized_active_resident_rematerialization_selected_form_encoding(
                corrupt_source,
                machine,
            ),
            Err(
                OptimizedActiveResidentRematerializationSelectedFormEncodingError::Rematerialization(
                    OptimizedActiveResidentRematerializationError::ReceiptMismatch
                )
            )
        ));

        let (x86_source, _) =
            staged_active_resident_rematerialization_and_machine(NativeTarget::linux_x64());
        let (_, arm_machine) =
            staged_active_resident_rematerialization_and_machine(NativeTarget::linux_arm64());
        assert!(matches!(
            stage_optimized_active_resident_rematerialization_selected_form_encoding(
                x86_source,
                arm_machine,
            ),
            Err(OptimizedActiveResidentRematerializationSelectedFormEncodingError::Machine(_))
        ));

        let (source, machine) =
            staged_active_resident_rematerialization_and_machine(NativeTarget::linux_x64());
        let mut staged = stage_optimized_active_resident_rematerialization_selected_form_encoding(
            source, machine,
        )
        .unwrap();
        crate::active_resident_selected_form_encoding::corrupt_active_resident_selected_form_encoding_byte_for_test(
            &mut staged,
        );
        assert_eq!(
            validate_optimized_active_resident_rematerialization_selected_form_encoding(&staged),
            Err(
                OptimizedActiveResidentRematerializationSelectedFormEncodingError::Encoding(
                    OptimizedSelectedFormEncodingError::ArtifactMismatch
                )
            )
        );
    }

    #[test]
    fn active_resident_rematerialization_reaches_resolved_layout_on_both_architectures() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let (source, machine) = staged_active_resident_rematerialization_and_machine(target);
            let fresh_materialize = source.rematerialization().plan().functions[0]
                .action
                .as_ref()
                .unwrap()
                .fresh_materialize;
            let physical = source
                .source()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment()
                .physical()
                .identity();
            let pre_layout =
                stage_optimized_active_resident_rematerialization_selected_form_encoding(
                    source, machine,
                )
                .unwrap();
            let pre_layout_custody = pre_layout.custody().clone();
            let selected = pre_layout.encoding().selected();
            let machine = pre_layout.encoding().machine();
            let pre_layout_encoding = pre_layout.encoding().identity();

            let staged =
                stage_optimized_active_resident_rematerialization_resolved_selected_form_layout(
                    pre_layout,
                )
                .unwrap();
            let layout = staged.layout();
            let custody = staged.custody();
            assert_eq!(custody.pre_layout_custody(), &pre_layout_custody);
            assert_eq!(custody.selected(), selected);
            assert_eq!(custody.machine(), machine);
            assert_eq!(custody.pre_layout(), pre_layout_encoding);
            assert_eq!(custody.physical(), physical);
            assert_eq!(custody.layout(), layout.identity());
            assert_eq!(custody.target(), target);
            assert_eq!(
                custody.policy(),
                TerminalSelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1
            );
            assert_eq!(custody.function_count(), 1);
            assert_eq!(custody.block_count(), 3);
            assert_eq!(
                custody.instruction_count(),
                custody.pre_layout_custody().row_count()
            );
            assert_eq!(
                custody.instruction_count(),
                layout
                    .functions()
                    .iter()
                    .flat_map(|function| &function.blocks)
                    .map(|block| block.instructions.len())
                    .sum()
            );
            assert_eq!(
                custody.byte_count(),
                layout
                    .functions()
                    .iter()
                    .map(|function| function.byte_count)
                    .sum()
            );
            assert_eq!(custody.resolved_branch_count(), 1);
            let rows = layout
                .functions()
                .iter()
                .flat_map(|function| &function.blocks)
                .flat_map(|block| &block.instructions)
                .collect::<Vec<_>>();
            let fresh_row = rows
                .iter()
                .find(|row| row.instruction == fresh_materialize)
                .expect("fresh rematerialization must survive resolved layout");
            assert_eq!(
                fresh_row.alternative.family,
                omega_terminal_selected_instructions::TerminalMachineAlternativeFamily::MaterializeI64
            );
            assert!(!fresh_row.bytes.is_empty());
            assert_eq!(
                rows.iter().filter(|row| row.branch.is_some()).count(),
                custody.resolved_branch_count()
            );
            assert_eq!(
                validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(
                    &staged,
                )
                .unwrap(),
                custody.clone()
            );
        }
    }

    #[test]
    fn active_resident_resolved_layout_rejects_pre_layout_layout_and_receipt_mutation() {
        let mut corrupt_pre_layout =
            staged_active_resident_resolved_layout(NativeTarget::linux_x64());
        crate::active_resident_resolved_selected_form_layout::corrupt_active_resident_resolved_layout_pre_layout_byte_for_test(
            &mut corrupt_pre_layout,
        );
        assert_eq!(
            validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(
                &corrupt_pre_layout,
            ),
            Err(
                OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::PreLayout(
                    OptimizedActiveResidentRematerializationSelectedFormEncodingError::Encoding(
                        OptimizedSelectedFormEncodingError::ArtifactMismatch,
                    ),
                ),
            )
        );

        let mut corrupt_layout = staged_active_resident_resolved_layout(NativeTarget::linux_x64());
        crate::active_resident_resolved_selected_form_layout::corrupt_active_resident_resolved_layout_byte_for_test(
            &mut corrupt_layout,
        );
        assert_eq!(
            validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(
                &corrupt_layout,
            ),
            Err(
                OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::Layout(
                    OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch,
                ),
            )
        );

        let mut corrupt_receipt = staged_active_resident_resolved_layout(NativeTarget::linux_x64());
        crate::active_resident_resolved_selected_form_layout::corrupt_active_resident_resolved_layout_receipt_for_test(
            &mut corrupt_receipt,
        );
        assert_eq!(
            validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(
                &corrupt_receipt,
            ),
            Err(
                OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::ReceiptMismatch,
            )
        );
    }

    #[test]
    fn active_resident_rematerialization_reaches_function_relative_exit_on_both_architectures() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let staged = staged_active_resident_function_relative_realization(target);
            let source = staged.source();
            let rematerialization = source.pre_layout().source();
            let physical = rematerialization
                .source()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment()
                .physical();
            let admitted_names = match target.architecture {
                omega_target::Architecture::X86_64 => ["rax", "rcx"],
                omega_target::Architecture::Aarch64 => ["x0", "x1"],
            };
            let admitted_views = admitted_names
                .into_iter()
                .map(|name| physical.model().view_named(name).unwrap().id)
                .collect::<BTreeSet<_>>();
            let TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views } =
                &rematerialization
                    .source()
                    .allocator_availability()
                    .plan()
                    .policy
            else {
                panic!("pressure fixture must retain an explicit caller-saved allowlist")
            };
            assert_eq!(
                views.iter().copied().collect::<BTreeSet<_>>(),
                admitted_views
            );
            let action = rematerialization.rematerialization().plan().functions[0]
                .action
                .as_ref()
                .expect("the explicit active-resident staging route must rematerialize");
            let fresh = action.fresh_materialize;
            let transformed_selected = rematerialization
                .rematerialization()
                .receipt()
                .transformed_selected();
            let fresh_layout_row = source
                .layout()
                .functions()
                .iter()
                .flat_map(|function| &function.blocks)
                .flat_map(|block| &block.instructions)
                .find(|instruction| instruction.instruction == fresh)
                .expect("fresh rematerialization must survive function-relative layout");
            assert_eq!(
                fresh_layout_row.alternative.family,
                omega_terminal_selected_instructions::TerminalMachineAlternativeFamily::MaterializeI64
            );
            assert!(!fresh_layout_row.bytes.is_empty());

            let manifest = staged.manifest().record();
            let empty = OptimizationSelections::default().identity();
            assert_eq!(
                manifest.selections,
                OptimizationSelections::new([
                    Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
                ])
                .unwrap()
                .identity()
            );
            assert_eq!(manifest.selected_lowering_selections, empty);
            assert_eq!(manifest.selected_lowering_completion, None);
            assert_eq!(manifest.allocation_recovery_selections, manifest.selections);
            assert_eq!(manifest.post_allocation_machine_selections, empty);
            assert_eq!(manifest.function_relative_layout_selections, empty);
            assert_eq!(
                manifest.pre_physical_manifest,
                rematerialization.custody().source().manifest()
            );
            assert_eq!(
                manifest.post_allocation_manifest,
                rematerialization
                    .post_allocation_manifest()
                    .record()
                    .identity
            );
            assert_eq!(manifest.selected, transformed_selected);
            assert_eq!(manifest.baseline_pre_layout, manifest.pre_layout);
            assert_eq!(manifest.baseline_resolved_layout, manifest.resolved_layout);
            assert_eq!(manifest.x86_branch_relaxation, None);
            assert_eq!(manifest.aarch64_cbnz_fusion, None);
            assert_eq!(manifest.target, target);
            assert_eq!(
                rematerialization
                    .post_allocation_manifest()
                    .record()
                    .selected_transformations,
                [
                    PostAllocationSelectedTransformation::PressureRematerialization(
                        rematerialization.rematerialization().receipt().identity(),
                    )
                ]
            );
            assert_eq!(
                staged.exit_contract().contract().selected,
                transformed_selected
            );
            assert_eq!(
                staged.exit_contract().contract().resolved_layout,
                source.layout().identity()
            );
            assert!(matches!(
                staged.exit_contract().contract().layout_custody,
                TerminalWholeFunctionExitLayoutCustody::BaselineNearLayoutV1
            ));
            assert!(
                staged
                    .exit_contract()
                    .contract()
                    .functions
                    .iter()
                    .all(|function| function.modified_callee_saved_units.is_empty())
            );
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&manifest.encode()),
                Ok(manifest.clone())
            );
            assert_eq!(
                validate_optimized_active_resident_rematerialization_function_relative_realization(
                    &staged,
                )
                .unwrap(),
                staged.custody().clone()
            );
            assert_eq!(staged.custody().source(), source.custody());
            assert_eq!(
                staged.custody().exit_contract(),
                staged.exit_contract().identity()
            );
            assert_eq!(staged.custody().realization(), manifest.identity);
        }
    }

    #[test]
    fn active_resident_function_relative_realization_rejects_corrupt_or_detached_custody() {
        let target = NativeTarget::linux_x64();

        let mut source_corruption = staged_active_resident_function_relative_realization(target);
        crate::active_resident_function_relative_realization::corrupt_active_resident_function_relative_source_for_test(
            &mut source_corruption,
        );
        assert!(matches!(
            validate_optimized_active_resident_rematerialization_function_relative_realization(
                &source_corruption,
            ),
            Err(
                OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::Source(
                    OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::Layout(
                        OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch,
                    ),
                ),
            )
        ));

        let mut exit_corruption = staged_active_resident_function_relative_realization(target);
        crate::active_resident_function_relative_realization::corrupt_active_resident_function_relative_exit_for_test(
            &mut exit_corruption,
        );
        assert_eq!(
            validate_optimized_active_resident_rematerialization_function_relative_realization(
                &exit_corruption,
            ),
            Err(
                OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::ExitContract(
                    TerminalWholeFunctionExitContractError::ArtifactMismatch,
                ),
            )
        );

        let mut manifest_corruption = staged_active_resident_function_relative_realization(target);
        crate::active_resident_function_relative_realization::corrupt_active_resident_function_relative_manifest_for_test(
            &mut manifest_corruption,
        );
        assert_eq!(
            validate_optimized_active_resident_rematerialization_function_relative_realization(
                &manifest_corruption,
            ),
            Err(
                OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::RootMismatch,
            )
        );

        let mut receipt_corruption = staged_active_resident_function_relative_realization(target);
        crate::active_resident_function_relative_realization::corrupt_active_resident_function_relative_receipt_for_test(
            &mut receipt_corruption,
        );
        assert_eq!(
            validate_optimized_active_resident_rematerialization_function_relative_realization(
                &receipt_corruption,
            ),
            Err(
                OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::ReceiptMismatch,
            )
        );

        let mut detached = staged_active_resident_function_relative_realization(target);
        let foreign =
            staged_active_resident_function_relative_realization(NativeTarget::linux_arm64());
        crate::active_resident_function_relative_realization::replace_active_resident_function_relative_exit_for_test(
            &mut detached,
            &foreign,
        );
        assert_eq!(
            validate_optimized_active_resident_rematerialization_function_relative_realization(
                &detached,
            ),
            Err(
                OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::ExitContract(
                    TerminalWholeFunctionExitContractError::ArtifactMismatch,
                ),
            )
        );
    }

    #[test]
    fn active_resident_function_relative_realization_rejects_unexecuted_later_phase_selections() {
        for later in [
            Optimization::SelectedIncomingU12ExactAddImmediate,
            Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
            Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
            Optimization::X86RelaxConditionalBranchesToRel8V1,
        ] {
            let selections = OptimizationSelections::new([
                Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
                later,
            ])
            .unwrap();
            let source = staged_active_resident_resolved_layout_with_selections(
                NativeTarget::linux_x64(),
                selections,
            );
            assert!(matches!(
                stage_optimized_active_resident_rematerialization_function_relative_realization(
                    source,
                ),
                Err(
                    OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::LaterPhaseSelected,
                )
            ));
        }
    }

    #[test]
    fn active_resident_stage_declines_default_single_use_and_exhausted_budget() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let default = stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_active_resident_exact_add_chain(target))
                        .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            assert!(matches!(
                stage_optimized_active_resident_rematerialization(
                    default,
                    TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                    TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                    TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
                    selected_lowering_budget(),
                ),
                Err(OptimizedActiveResidentRematerializationError::Rematerialization(
                    TerminalPressureRematerializationError::NoAction
                ))
            ));
            assert!(matches!(
                stage_optimized_active_resident_rematerialization(
                    staged_active_resident_two_view_legality(target),
                    TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                    TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                    TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
                    selected_lowering_budget(),
                ),
                Err(OptimizedActiveResidentRematerializationError::UnsupportedPolicy)
            ));
            assert!(matches!(
                stage_optimized_active_resident_rematerialization(
                    staged_active_resident_two_view_legality(target),
                    TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                    TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                    TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
                    OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap(),
                ),
                Err(OptimizedActiveResidentRematerializationError::SpillChoice(
                    omega_regalloc::TerminalSpillChoiceError::BudgetExceeded { .. }
                ))
            ));
        }
    }

    #[test]
    fn active_resident_stage_rejects_corrupted_vertical_custody() {
        let mut staged = stage_optimized_active_resident_rematerialization(
            staged_active_resident_two_view_legality(NativeTarget::linux_x64()),
            TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            selected_lowering_budget(),
        )
        .unwrap();
        crate::active_resident_rematerialization::corrupt_active_resident_rematerialization_custody_for_test(
            &mut staged,
        );
        assert_eq!(
            validate_optimized_active_resident_rematerialization(&staged),
            Err(OptimizedActiveResidentRematerializationError::ReceiptMismatch)
        );
    }

    #[test]
    fn explicit_one_view_availability_reaches_real_pressure_and_recovery_on_both_architectures() {
        for (target, sole_view_name) in [
            (NativeTarget::linux_x64(), "rdi"),
            (NativeTarget::linux_arm64(), "x0"),
        ] {
            let ranges = stage_optimized_live_ranges(
                stage_optimized_liveness(staged_exact_add_conditional(target)).unwrap(),
            )
            .unwrap();
            let environment = ranges
                .liveness_stage()
                .selected_stage()
                .register_environment();
            let sole_view = environment
                .physical()
                .model()
                .view_named(sole_view_name)
                .unwrap()
                .id;
            let fixed_return = (target == NativeTarget::linux_x64())
                .then(|| environment.physical().model().view_named("rax").unwrap().id);
            assert!(matches!(
                materialize_terminal_allocator_availability(
                    environment.identity(),
                    environment.target(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                        views: vec![sole_view, sole_view],
                    },
                ),
                Err(TerminalAllocatorAvailabilityError::NonCanonicalAllowlist)
            ));
            assert!(matches!(
                materialize_terminal_allocator_availability(
                    environment.identity(),
                    environment.target(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                        views: vec![RegisterViewId(u16::MAX)],
                    },
                ),
                Err(TerminalAllocatorAvailabilityError::UnknownView { .. })
            ));
            let reserved_view =
                environment
                    .physical()
                    .model()
                    .views
                    .iter()
                    .find(|view| {
                        view.allocatable
                            && view.units.iter().chain(&view.write_units).any(|unit| {
                                environment.reservations().reserved_units().contains(unit)
                            })
                    })
                    .unwrap();
            assert!(matches!(
                materialize_terminal_allocator_availability(
                    environment.identity(),
                    environment.target(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                        views: vec![reserved_view.id],
                    },
                ),
                Err(TerminalAllocatorAvailabilityError::ViewNotEnvironmentAllocatable { .. })
            ));
            let availability = materialize_terminal_allocator_availability(
                environment.identity(),
                environment.target(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                    views: vec![sole_view],
                },
            )
            .unwrap();
            let mut noncanonical = availability.plan().clone();
            let retained_row = noncanonical
                .classes
                .iter_mut()
                .find(|row| !row.unconstrained_views.is_empty())
                .unwrap();
            retained_row.unconstrained_views.push(sole_view);
            assert_eq!(
                validate_terminal_allocator_availability(
                    environment.identity(),
                    environment.target(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    noncanonical,
                ),
                Err(TerminalAllocatorAvailabilityError::NonCanonicalPlan)
            );
            let encoded = availability.plan().encode();
            assert_eq!(
                omega_regalloc::TerminalAllocatorAvailabilityPlan::decode(&encoded).unwrap(),
                *availability.plan()
            );
            let legality =
                stage_optimized_allocation_legality_with_availability(ranges, availability)
                    .unwrap();
            if let Some(fixed_return) = fixed_return {
                assert_ne!(fixed_return, sole_view);
                assert!(
                    legality
                        .legality()
                        .plan()
                        .functions
                        .iter()
                        .flat_map(|function| &function.virtual_registers)
                        .flat_map(|register| &register.points)
                        .any(|point| point.candidates == vec![fixed_return])
                );
            }
            assert_eq!(
                legality.custody().allocator_availability(),
                legality.allocator_availability().receipt().identity()
            );
            let ranges = legality.live_range_stage();
            let selected = ranges.liveness_stage().selected_stage();
            let environment = selected.register_environment();
            let choices = choose_terminal_spill_victims(
                legality.legality(),
                ranges.ranges(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                OptimizationWorkBudget::new(100, 100, 1_000, 100, 1).unwrap(),
            )
            .unwrap();
            let choice = choices.plan().functions[0].choice.as_ref().unwrap();
            assert_eq!(choice.incoming, TerminalVirtualRegisterId(2));
            assert_eq!(choice.selected_victim, choice.incoming);
            assert_eq!(choice.incoming_common_candidates, vec![sole_view]);

            let recovery = classify_terminal_pressure_recovery(
                selected.selected(),
                ranges.ranges(),
                legality.legality(),
                &choices,
                TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                OptimizationWorkBudget::new(100, 100, 1_000, 100, 1).unwrap(),
            )
            .unwrap();
            let row = recovery.plan().functions[0]
                .classification
                .as_ref()
                .unwrap();
            assert_eq!(row.victim, TerminalVirtualRegisterId(2));
            assert_eq!(row.role, TerminalRecoveryVictimRole::Incoming);
            assert!(matches!(
                row.classification,
                TerminalRecoveryClassification::ImmediateU64RematerializationCandidate {
                    value: IntegerValue::Unsigned(8),
                    ..
                }
            ));
        }
    }

    #[test]
    fn two_explicit_u12_exact_add_folds_close_one_view_pressure_on_both_architectures() {
        for (target, sole_view_name) in [
            (NativeTarget::linux_x64(), "rax"),
            (NativeTarget::linux_arm64(), "x0"),
        ] {
            let ranges = stage_optimized_live_ranges(
                stage_optimized_liveness(staged_exact_add_conditional(target)).unwrap(),
            )
            .unwrap();
            let environment = ranges
                .liveness_stage()
                .selected_stage()
                .register_environment();
            let sole_view = environment
                .physical()
                .model()
                .view_named(sole_view_name)
                .unwrap()
                .id;
            let availability = materialize_terminal_allocator_availability(
                environment.identity(),
                environment.target(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                    views: vec![sole_view],
                },
            )
            .unwrap();
            let legality =
                stage_optimized_allocation_legality_with_availability(ranges, availability)
                    .unwrap();
            let ranges = legality.live_range_stage();
            let selected = ranges.liveness_stage().selected_stage();
            let environment = selected.register_environment();
            let choices = choose_terminal_spill_victims(
                legality.legality(),
                ranges.ranges(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                budget(),
            )
            .unwrap();
            let recovery = classify_terminal_pressure_recovery(
                selected.selected(),
                ranges.ranges(),
                legality.legality(),
                &choices,
                TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                budget(),
            )
            .unwrap();
            let fold_one = fold_terminal_selected_incoming_literal(
                selected.selected(),
                ranges.ranges(),
                legality.legality(),
                &choices,
                &recovery,
                legality.allocator_availability(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
                budget(),
            )
            .unwrap();
            assert_eq!(fold_one.receipt().applied_count(), 1);
            assert_eq!(
                TerminalLiteralFoldPlan::decode(&fold_one.plan().encode()).unwrap(),
                *fold_one.plan()
            );
            let mut corrupted_recipe = fold_one.plan().clone();
            corrupted_recipe.functions[0]
                .action
                .as_mut()
                .unwrap()
                .immediate += 1;
            assert!(matches!(
                validate_terminal_literal_fold(
                    selected.selected(),
                    ranges.ranges(),
                    legality.legality(),
                    &choices,
                    &recovery,
                    legality.allocator_availability(),
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    corrupted_recipe,
                ),
                Err(omega_regalloc::TerminalLiteralFoldError::DecisionMismatch { .. })
            ));
            let foreign_target = match target.architecture {
                omega_target::Architecture::X86_64 => NativeTarget::linux_arm64(),
                omega_target::Architecture::Aarch64 => NativeTarget::linux_x64(),
            };
            let foreign_environment = baseline_target_register_environment(foreign_target).unwrap();
            assert_eq!(
                validate_terminal_literal_fold(
                    selected.selected(),
                    ranges.ranges(),
                    legality.legality(),
                    &choices,
                    &recovery,
                    legality.allocator_availability(),
                    environment.identity(),
                    foreign_environment.physical(),
                    foreign_environment.constraints(),
                    foreign_environment.reservations(),
                    environment.allocation_constraint_keys(),
                    fold_one.plan().clone(),
                ),
                Err(omega_regalloc::TerminalLiteralFoldError::RootMismatch)
            );
            let folded_add = &fold_one.transformed().functions[0].blocks[1].instructions[1];
            assert!(matches!(
                folded_add.kind,
                TerminalSelectedInstructionKind::ExactAddI64Immediate {
                    immediate: IntegerValue::Unsigned(8),
                    ..
                }
            ));
            assert_eq!(folded_add.provenance.operations.len(), 2);
            assert_eq!(folded_add.provenance.obligations.len(), 1);

            let liveness_one = analyze_terminal_liveness(&fold_one).unwrap();
            let ranges_one = analyze_terminal_live_ranges(&fold_one, &liveness_one).unwrap();
            let legality_one = analyze_terminal_allocation_legality(
                &ranges_one,
                legality.allocator_availability(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
            )
            .unwrap();
            let choices_one = choose_terminal_spill_victims(
                &legality_one,
                &ranges_one,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                budget(),
            )
            .unwrap();
            assert_eq!(
                choices_one.plan().functions[0]
                    .choice
                    .as_ref()
                    .unwrap()
                    .incoming,
                TerminalVirtualRegisterId(4)
            );
            let recovery_one = classify_terminal_pressure_recovery(
                &fold_one,
                &ranges_one,
                &legality_one,
                &choices_one,
                TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                budget(),
            )
            .unwrap();
            let fold_two = fold_terminal_selected_incoming_literal(
                &fold_one,
                &ranges_one,
                &legality_one,
                &choices_one,
                &recovery_one,
                legality.allocator_availability(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
                budget(),
            )
            .unwrap();
            assert_eq!(fold_two.receipt().applied_count(), 1);

            let liveness_two = analyze_terminal_liveness(&fold_two).unwrap();
            let ranges_two = analyze_terminal_live_ranges(&fold_two, &liveness_two).unwrap();
            let legality_two = analyze_terminal_allocation_legality(
                &ranges_two,
                legality.allocator_availability(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
            )
            .unwrap();
            let choices_two = choose_terminal_spill_victims(
                &legality_two,
                &ranges_two,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                budget(),
            )
            .unwrap();
            assert!(
                choices_two
                    .plan()
                    .functions
                    .iter()
                    .all(|function| function.choice.is_none())
            );
            let homes = omega_regalloc::assign_terminal_register_homes(
                &legality_two,
                &ranges_two,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
            )
            .unwrap();
            assert_eq!(
                homes.plan().functions[0].assignments.len(),
                fold_two.transformed().functions[0].virtual_registers.len()
            );

            let staged_folds = stage_first_optimized_literal_fold(
                legality,
                TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
                budget(),
            )
            .unwrap();
            assert_eq!(staged_folds.steps().len(), 1);
            let staged_environment = staged_folds
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment();
            assert!(matches!(
                omega_regalloc::assign_terminal_register_homes(
                    staged_folds.final_step().legality(),
                    staged_folds.final_step().ranges(),
                    staged_environment.identity(),
                    staged_environment.physical(),
                    staged_environment.constraints(),
                    staged_environment.reservations(),
                    staged_environment.allocation_constraint_keys(),
                ),
                Err(TerminalRegisterHomeError::NoCompatibleHome { .. })
            ));
            let staged_folds = stage_next_optimized_literal_fold(
                staged_folds,
                TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
                budget(),
            )
            .unwrap();
            assert_eq!(staged_folds.steps().len(), 2);
            assert_eq!(staged_folds.custody().transformations().len(), 2);
            let [first_iteration, second_iteration] = staged_folds.custody().iterations() else {
                panic!("two explicit fold calls must retain two iteration receipts");
            };
            assert_eq!(
                first_iteration.transformed_selected(),
                second_iteration.source_selected()
            );
            assert_eq!(
                first_iteration.fresh_ranges(),
                second_iteration.source_ranges()
            );
            assert_eq!(
                first_iteration.fresh_legality(),
                second_iteration.source_legality()
            );
            assert_eq!(
                second_iteration.fold_policy(),
                TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1
            );
            assert_eq!(
                validate_optimized_literal_fold_custody(&staged_folds).unwrap(),
                *staged_folds.custody()
            );
            assert_eq!(
                staged_folds.custody().final_selected(),
                staged_folds
                    .final_step()
                    .fold()
                    .receipt()
                    .transformed_selected()
            );
            let machine_effects =
                stage_optimized_machine_effects_after_literal_folds(&staged_folds).unwrap();
            assert_eq!(
                machine_effects.effects().receipt().selected(),
                staged_folds.custody().final_selected()
            );
            assert_eq!(
                machine_effects.custody().source(),
                &StagedOptimizedMachineEffectSourceCustodyReceipt::LiteralFolds(
                    staged_folds.custody().clone()
                )
            );
            assert_eq!(
                &validate_optimized_machine_effect_custody_after_literal_folds(
                    &staged_folds,
                    machine_effects.effects(),
                )
                .unwrap(),
                machine_effects.custody()
            );
            let expected_transformations = staged_folds
                .custody()
                .transformations()
                .iter()
                .copied()
                .map(PostAllocationSelectedTransformation::LiteralFold)
                .collect::<Vec<_>>();
            let staged_homes =
                stage_optimized_register_homes_after_literal_folds(staged_folds).unwrap();
            let post =
                stage_optimized_post_allocation_machine_plan_after_literal_folds(&staged_homes)
                    .unwrap();
            assert_eq!(
                post.machine().receipt().selected(),
                staged_homes.fold_stage().custody().final_selected()
            );
            assert_eq!(
                &validate_optimized_post_allocation_machine_plan_after_literal_fold_custody(
                    &staged_homes,
                    &post,
                )
                .unwrap(),
                post.custody()
            );
            assert_eq!(
                staged_homes
                    .post_allocation_manifest()
                    .record()
                    .selected_transformations,
                expected_transformations
            );
            assert_eq!(
                staged_homes.post_allocation_manifest().record().selected,
                staged_homes.fold_stage().custody().final_selected()
            );
            assert_eq!(
                validate_optimized_register_home_after_literal_fold_custody(&staged_homes).unwrap(),
                *staged_homes.custody()
            );
        }
    }

    #[test]
    fn named_selected_lowering_suite_reaches_a_verified_fixed_point_on_both_architectures() {
        for (target, sole_view_name) in [
            (NativeTarget::linux_x64(), "rax"),
            (NativeTarget::linux_arm64(), "x0"),
        ] {
            let selections = OptimizationSelections::new([
                Optimization::CopyPropagation,
                Optimization::SelectedIncomingU12ExactAddImmediate,
            ])
            .unwrap();
            let ranges = stage_optimized_live_ranges(
                stage_optimized_liveness(staged_exact_add_conditional_with_selections(
                    target,
                    selections.clone(),
                    selected_lowering_budget(),
                ))
                .unwrap(),
            )
            .unwrap();
            let environment = ranges
                .liveness_stage()
                .selected_stage()
                .register_environment();
            let sole_view = environment
                .physical()
                .model()
                .view_named(sole_view_name)
                .unwrap()
                .id;
            let availability = materialize_terminal_allocator_availability(
                environment.identity(),
                environment.target(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                    views: vec![sole_view],
                },
            )
            .unwrap();
            let legality =
                stage_optimized_allocation_legality_with_availability(ranges, availability)
                    .unwrap();
            let run = run_selected_lowering_optimizations(legality).unwrap();

            assert_eq!(run.selections(), &selections);
            assert_eq!(run.custody().selections(), selections.identity());
            assert_eq!(
                run.selected_lowering_selections().as_slice(),
                &[Optimization::SelectedIncomingU12ExactAddImmediate]
            );
            assert_eq!(
                run.custody().selected_lowering_selections(),
                run.selected_lowering_selections().identity()
            );
            assert_eq!(run.steps().len(), 2);
            assert_eq!(run.custody().action_count(), 2);
            assert_eq!(
                run.custody()
                    .initial_virtual_register_count()
                    .checked_sub(run.custody().action_count()),
                Some(run.custody().final_virtual_register_count())
            );
            assert_eq!(run.custody().iterations().len(), 2);
            assert_eq!(run.terminal_attempt().fold().receipt().applied_count(), 0);
            assert_eq!(
                run.terminal_attempt().fold().receipt().source_selected(),
                run.terminal_attempt()
                    .fold()
                    .receipt()
                    .transformed_selected()
            );
            assert!(run.custody().usage().within(run.custody().budget()));
            assert_eq!(
                validate_selected_lowering_optimization_custody(&run).unwrap(),
                *run.custody()
            );
            let completion = run.custody().identity();
            let final_selected = run.custody().final_selected();
            let expected_transformations = run
                .custody()
                .iterations()
                .iter()
                .map(|iteration| {
                    PostAllocationSelectedTransformation::LiteralFold(iteration.fold())
                })
                .collect::<Vec<_>>();
            let homes = stage_optimized_register_homes_after_selected_lowering(run).unwrap();
            assert_eq!(
                homes
                    .post_allocation_manifest()
                    .record()
                    .selected_lowering_completion,
                Some(completion)
            );
            assert_eq!(
                homes
                    .post_allocation_manifest()
                    .record()
                    .selected_transformations,
                expected_transformations
            );
            assert_eq!(
                validate_optimized_register_home_after_selected_lowering_custody(&homes).unwrap(),
                *homes.custody()
            );
            let realization = stage_selected_lowering_function_relative_realization(homes).unwrap();
            let post = realization.machine();
            assert_eq!(post.machine().receipt().selected(), final_selected);
            assert_eq!(
                validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
                    realization.homes(),
                    post,
                )
                .unwrap(),
                *post.custody()
            );
            assert_eq!(
                validate_selected_lowering_function_relative_realization_custody(&realization)
                    .unwrap(),
                *realization.custody()
            );
            let manifest = realization.manifest().record();
            assert_eq!(manifest.selections, selections.identity());
            assert_eq!(
                manifest.selected_lowering_selections,
                selections
                    .for_phase(
                        omega_optimization_core::OptimizationExecutionPhase::SelectedLowering,
                    )
                    .identity()
            );
            assert_eq!(manifest.selected_lowering_completion, Some(completion));
            assert_eq!(
                manifest.function_relative_layout_selections,
                selections
                    .for_phase(
                        omega_optimization_core::OptimizationExecutionPhase::FunctionRelativeLayout,
                    )
                    .identity()
            );
            assert_eq!(manifest.selected, final_selected);
            assert_eq!(manifest.pre_layout, realization.encoding().identity());
            assert_eq!(
                manifest.baseline_resolved_layout,
                realization.layout().identity()
            );
            assert_eq!(manifest.resolved_layout, realization.layout().identity());
            assert_eq!(manifest.x86_branch_relaxation, None);
            assert_eq!(
                manifest.whole_function_exit_contract,
                realization.exit_contract().identity()
            );
            assert_eq!(
                realization.exit_contract().contract().functions[0]
                    .returns
                    .len(),
                2
            );
            assert_eq!(manifest.statistics.functions, 1);
            assert_eq!(manifest.statistics.blocks, 3);
            assert_eq!(manifest.statistics.resolved_conditional_branches, 1);
            assert_eq!(
                manifest.statistics.bytes,
                realization
                    .layout()
                    .functions()
                    .iter()
                    .map(|function| function.byte_count)
                    .sum()
            );
        }
    }

    #[test]
    fn named_exact_subtract_immediate_suite_closes_pressure_and_rejects_policy_substitution() {
        for (target, sole_view_name) in [
            (NativeTarget::linux_x64(), "rax"),
            (NativeTarget::linux_arm64(), "x0"),
        ] {
            let selections = OptimizationSelections::new([
                Optimization::CopyPropagation,
                Optimization::SelectedIncomingU12ExactSubtractImmediate,
            ])
            .unwrap();
            let ranges = stage_optimized_live_ranges(
                stage_optimized_liveness(staged_exact_subtract_conditional_with_selections(
                    target,
                    selections.clone(),
                    selected_lowering_budget(),
                ))
                .unwrap(),
            )
            .unwrap();
            let environment = ranges
                .liveness_stage()
                .selected_stage()
                .register_environment();
            let sole_view = environment
                .physical()
                .model()
                .view_named(sole_view_name)
                .unwrap()
                .id;
            let availability = materialize_terminal_allocator_availability(
                environment.identity(),
                environment.target(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                    views: vec![sole_view],
                },
            )
            .unwrap();
            let legality =
                stage_optimized_allocation_legality_with_availability(ranges, availability)
                    .unwrap();
            let run = run_selected_lowering_optimizations(legality).unwrap();

            assert_eq!(
                run.selected_lowering_selections().as_slice(),
                &[Optimization::SelectedIncomingU12ExactSubtractImmediate]
            );
            assert_eq!(
                run.custody().schedule(),
                SelectedLoweringOptimizationSchedule::SelectedIncomingU12ExactSubtractImmediateToNoChangeV1
            );
            assert_eq!(run.steps().len(), 2);
            assert_eq!(run.custody().action_count(), 2);
            assert_eq!(run.terminal_attempt().fold().receipt().applied_count(), 0);
            assert_eq!(
                validate_selected_lowering_optimization_custody(&run).unwrap(),
                *run.custody()
            );

            let final_plan = run.terminal_attempt().fold().transformed();
            let folded = final_plan.functions[0]
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter_map(|instruction| match instruction.kind {
                    TerminalSelectedInstructionKind::ExactSubtractI64Immediate {
                        immediate,
                        ..
                    } => Some((immediate, &instruction.provenance)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                folded
                    .iter()
                    .map(|(immediate, _)| *immediate)
                    .collect::<Vec<_>>(),
                vec![IntegerValue::Unsigned(5), IntegerValue::Unsigned(8)]
            );
            for (_, provenance) in folded {
                assert_eq!(provenance.operations.len(), 2);
                assert_eq!(provenance.fuel.len(), 2);
                assert_eq!(provenance.obligations.len(), 1);
            }

            let source = run.source_legality_stage();
            let selected = source.live_range_stage().liveness_stage().selected_stage();
            let environment = selected.register_environment();
            let first = &run.steps()[0];
            let mut substituted_policy = first.fold().plan().clone();
            substituted_policy.policy =
                TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1;
            assert!(
                validate_terminal_literal_fold(
                    selected.selected(),
                    source.live_range_stage().ranges(),
                    source.legality(),
                    first.choices(),
                    first.recovery(),
                    source.allocator_availability(),
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    substituted_policy,
                )
                .is_err()
            );

            let homes = stage_optimized_register_homes_after_selected_lowering(run).unwrap();
            assert_eq!(
                validate_optimized_register_home_after_selected_lowering_custody(&homes).unwrap(),
                *homes.custody()
            );
        }
    }

    #[test]
    fn combined_exact_immediate_selection_executes_each_named_shape() {
        for subtract in [false, true] {
            let target = NativeTarget::linux_x64();
            let selections = OptimizationSelections::new([
                Optimization::SelectedIncomingU12ExactAddImmediate,
                Optimization::SelectedIncomingU12ExactSubtractImmediate,
            ])
            .unwrap();
            let selected = if subtract {
                staged_exact_subtract_conditional_with_selections(
                    target,
                    selections.clone(),
                    selected_lowering_budget(),
                )
            } else {
                staged_exact_add_conditional_with_selections(
                    target,
                    selections.clone(),
                    selected_lowering_budget(),
                )
            };
            let ranges =
                stage_optimized_live_ranges(stage_optimized_liveness(selected).unwrap()).unwrap();
            let environment = ranges
                .liveness_stage()
                .selected_stage()
                .register_environment();
            let sole_view = environment.physical().model().view_named("rax").unwrap().id;
            let availability = materialize_terminal_allocator_availability(
                environment.identity(),
                environment.target(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                    views: vec![sole_view],
                },
            )
            .unwrap();
            let run = run_selected_lowering_optimizations(
                stage_optimized_allocation_legality_with_availability(ranges, availability)
                    .unwrap(),
            )
            .unwrap();

            assert_eq!(
                run.custody().schedule(),
                SelectedLoweringOptimizationSchedule::SelectedIncomingU12ExactAddAndSubtractImmediateToNoChangeV1
            );
            assert_eq!(run.custody().action_count(), 2);
            let final_plan = run.terminal_attempt().fold().transformed();
            let matching_immediate_count = final_plan.functions[0]
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter(|instruction| {
                    if subtract {
                        matches!(
                            instruction.kind,
                            TerminalSelectedInstructionKind::ExactSubtractI64Immediate { .. }
                        )
                    } else {
                        matches!(
                            instruction.kind,
                            TerminalSelectedInstructionKind::ExactAddI64Immediate { .. }
                        )
                    }
                })
                .count();
            assert_eq!(matching_immediate_count, 2);
            assert_eq!(
                validate_selected_lowering_optimization_custody(&run).unwrap(),
                *run.custody()
            );
        }
    }

    #[test]
    fn named_selected_lowering_suite_retains_verified_no_change_completion() {
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            let selections = OptimizationSelections::new([
                Optimization::CopyPropagation,
                Optimization::SelectedIncomingU12ExactAddImmediate,
            ])
            .unwrap();
            let legality = stage_optimized_allocation_legality_for_frameless_leaf(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_exact_add_conditional_with_selections(
                        target,
                        selections.clone(),
                        selected_lowering_budget(),
                    ))
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            let source_selected = legality
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected()
                .receipt()
                .identity();
            let source_ranges = legality.live_range_stage().ranges().receipt().identity();
            let source_legality = legality.legality().receipt().identity();
            let run = run_selected_lowering_optimizations(legality).unwrap();

            assert!(run.steps().is_empty());
            assert_eq!(run.custody().action_count(), 0);
            assert_eq!(run.custody().final_selected(), source_selected);
            assert_eq!(run.custody().final_ranges(), source_ranges);
            assert_eq!(run.custody().final_legality(), source_legality);
            assert_eq!(run.terminal_attempt().fold().receipt().applied_count(), 0);
            assert_eq!(
                validate_selected_lowering_optimization_custody(&run).unwrap(),
                *run.custody()
            );
            let completion = run.custody().identity();
            let homes = stage_optimized_register_homes_after_selected_lowering(run).unwrap();
            let manifest = homes.post_allocation_manifest().record();
            assert_eq!(manifest.selected_lowering_completion, Some(completion));
            assert!(manifest.selected_transformations.is_empty());
            assert_eq!(manifest.selected, source_selected);
            let realization = stage_selected_lowering_function_relative_realization(homes).unwrap();
            let post = realization.machine();
            assert_eq!(post.machine().receipt().selected(), source_selected);
            assert_eq!(
                validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
                    realization.homes(),
                    post,
                )
                .unwrap(),
                *post.custody()
            );
            assert_eq!(
                validate_selected_lowering_function_relative_realization_custody(&realization)
                    .unwrap(),
                *realization.custody()
            );
            let manifest = realization.manifest().record();
            assert_eq!(manifest.selected_lowering_completion, Some(completion));
            assert_eq!(manifest.selected, source_selected);
            assert_eq!(
                manifest.whole_function_exit_contract,
                realization.exit_contract().identity()
            );
            assert_eq!(realization.exit_contract().contract().functions.len(), 1);
            assert_eq!(
                realization.exit_contract().contract().functions[0]
                    .returns
                    .len(),
                2
            );
            assert!(
                realization.exit_contract().contract().functions[0]
                    .modified_callee_saved_units
                    .is_empty()
            );
            let exit = realization.exit_contract().contract();
            assert_eq!(
                exit.hardening,
                TerminalWholeFunctionHardeningPolicy::NoAdditionalEntryExitHardeningV1
            );
            match (target.architecture, target.object_format) {
                (omega_target::Architecture::X86_64, omega_target::ObjectFormat::Elf) => {
                    assert_eq!(
                        exit.policy,
                        TerminalWholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1
                    );
                    assert_eq!(
                        exit.entry_assumption,
                        TerminalWholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1
                    );
                    assert!(exit.functions[0].returns.iter().all(|returned| matches!(
                        returned.mechanism,
                        TerminalWholeFunctionReturnMechanism::X86ActivationStackReturnV1 {
                            read_bytes: 8,
                            pop_bytes: 8,
                            ..
                        }
                    )));
                }
                (omega_target::Architecture::X86_64, omega_target::ObjectFormat::Coff) => {
                    assert_eq!(
                        exit.policy,
                        TerminalWholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1
                    );
                    assert_eq!(
                        exit.entry_assumption,
                        TerminalWholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1
                    );
                }
                (omega_target::Architecture::Aarch64, omega_target::ObjectFormat::Elf) => {
                    assert_eq!(
                        exit.policy,
                        TerminalWholeFunctionExitPolicy::Aapcs64FramelessLeafV1
                    );
                    assert!(matches!(
                        exit.entry_assumption,
                        TerminalWholeFunctionEntryAssumption::CallerLinkRegisterV1 { .. }
                    ));
                    assert!(exit.functions[0].returns.iter().all(|returned| matches!(
                        returned.mechanism,
                        TerminalWholeFunctionReturnMechanism::Aarch64LinkRegisterReturnV1 { .. }
                    )));
                }
                (omega_target::Architecture::Aarch64, omega_target::ObjectFormat::MachO) => {
                    assert_eq!(
                        exit.policy,
                        TerminalWholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1
                    );
                    assert!(matches!(
                        exit.entry_assumption,
                        TerminalWholeFunctionEntryAssumption::CallerLinkRegisterV1 { .. }
                    ));
                }
                _ => unreachable!(),
            }
            assert_eq!(manifest.statistics.functions, 1);
            assert_eq!(manifest.statistics.blocks, 3);
            assert_eq!(manifest.statistics.resolved_conditional_branches, 1);
            assert_eq!(
                manifest.publication,
                FunctionRelativeOptimizationUnavailableData::Unavailable
            );
            let encoded = manifest.encode();
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&encoded),
                Ok(manifest.clone())
            );
            let mut without_selected_lowering = manifest.clone();
            without_selected_lowering.selected_lowering_completion = None;
            without_selected_lowering.identity = without_selected_lowering.recomputed_identity();
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(
                    &without_selected_lowering.encode()
                ),
                Ok(without_selected_lowering)
            );
            if target.architecture == omega_target::Architecture::X86_64 {
                let mut with_no_change_relaxation = manifest.clone();
                with_no_change_relaxation.x86_branch_relaxation =
                    Some(TerminalX86BranchRelaxationIdentity::from_bytes([0x4f; 32]));
                with_no_change_relaxation.identity =
                    with_no_change_relaxation.recomputed_identity();
                assert_eq!(
                    FunctionRelativeOptimizationRealizationManifest::decode(
                        &with_no_change_relaxation.encode()
                    ),
                    Ok(with_no_change_relaxation)
                );
            }
            assert!(manifest.render_text().contains("publication: unavailable"));
            let mut identity_tamper = encoded.clone();
            identity_tamper[12] ^= 1;
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&identity_tamper),
                Err(FunctionRelativeOptimizationRealizationManifestDecodeError::IdentityMismatch)
            );
            let mut wrong_magic = encoded.clone();
            wrong_magic[0] ^= 1;
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&wrong_magic),
                Err(FunctionRelativeOptimizationRealizationManifestDecodeError::WrongMagic)
            );
            let mut wrong_version = encoded.clone();
            wrong_version[8..12].copy_from_slice(&6_u32.to_le_bytes());
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&wrong_version),
                Err(
                    FunctionRelativeOptimizationRealizationManifestDecodeError::UnsupportedVersion(
                        6
                    )
                )
            );
            let mut legacy_version = encoded.clone();
            legacy_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&legacy_version),
                Err(
                    FunctionRelativeOptimizationRealizationManifestDecodeError::UnsupportedVersion(
                        2
                    )
                )
            );
            let mut trailing = encoded.clone();
            trailing.push(0);
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&trailing),
                Err(FunctionRelativeOptimizationRealizationManifestDecodeError::TrailingBytes)
            );
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(
                    &encoded[..encoded.len() - 1]
                ),
                Err(FunctionRelativeOptimizationRealizationManifestDecodeError::Truncated)
            );
            let content_offset = 8 + 4 + 32;
            let selected_lowering_completion_status_offset = content_offset + 1 + 2 * 32;
            let x86_branch_relaxation_status_offset =
                selected_lowering_completion_status_offset + 1 + 32 + 12 * 32;
            let aarch64_cbnz_fusion_status_offset = x86_branch_relaxation_status_offset + 1;
            let mut unknown_stage = encoded.clone();
            unknown_stage[content_offset] = 9;
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&unknown_stage),
                Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownStage(9))
            );
            let mut unknown_selected_lowering_completion = encoded.clone();
            unknown_selected_lowering_completion[selected_lowering_completion_status_offset] = 9;
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(
                    &unknown_selected_lowering_completion
                ),
                Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownSelectedLoweringCompletionStatus(9))
            );
            let mut unknown_x86_branch_relaxation = encoded.clone();
            unknown_x86_branch_relaxation[x86_branch_relaxation_status_offset] = 9;
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(
                    &unknown_x86_branch_relaxation
                ),
                Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownX86BranchRelaxationStatus(9))
            );
            let mut unknown_aarch64_cbnz_fusion = encoded.clone();
            unknown_aarch64_cbnz_fusion[aarch64_cbnz_fusion_status_offset] = 9;
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(
                    &unknown_aarch64_cbnz_fusion
                ),
                Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownAarch64CbnzFusionStatus(9))
            );
            let target_offset = aarch64_cbnz_fusion_status_offset + 1 + 32;
            let mut unknown_architecture = encoded.clone();
            unknown_architecture[target_offset] = 9;
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&unknown_architecture),
                Err(
                    FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownArchitecture(
                        9
                    )
                )
            );
            let mut unknown_object_format = encoded.clone();
            unknown_object_format[target_offset + 1] = 9;
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&unknown_object_format),
                Err(
                    FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownObjectFormat(
                        9
                    )
                )
            );
            let layout_policy_offset = target_offset + 2 + 8 + 8;
            let mut unknown_layout_policy = encoded.clone();
            unknown_layout_policy[layout_policy_offset] = 9;
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&unknown_layout_policy),
                Err(
                    FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownLayoutPolicy(
                        9
                    )
                )
            );
            let mut unknown_scope = encoded.clone();
            unknown_scope[layout_policy_offset + 1] = 9;
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&unknown_scope),
                Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownScope(9))
            );
            let mut unknown_unavailable = encoded.clone();
            *unknown_unavailable.last_mut().unwrap() = 9;
            assert_eq!(
                FunctionRelativeOptimizationRealizationManifest::decode(&unknown_unavailable),
                Err(FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownUnavailableStatus(9))
            );

            let mut corrupted = realization;
            macro_rules! assert_manifest_field_is_bound {
                ($field:ident, $replacement:expr) => {{
                    let original = corrupted.manifest().record().$field;
                    corrupted.manifest_mut().record_mut().$field = $replacement;
                    assert_eq!(
                        validate_selected_lowering_function_relative_realization_custody(
                            &corrupted
                        ),
                        Err(FunctionRelativeOptimizationRealizationError::RootMismatch)
                    );
                    corrupted.manifest_mut().record_mut().$field = original;
                }};
            }
            assert_manifest_field_is_bound!(
                identity,
                omega_optimization_core::FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(
                    [0x50; 32]
                )
            );
            assert_manifest_field_is_bound!(
                selections,
                omega_optimization_core::OptimizationSelectionIdentity::from_bytes([0x51; 32])
            );
            assert_manifest_field_is_bound!(
                selected_lowering_selections,
                omega_optimization_core::OptimizationSelectionIdentity::from_bytes([0x52; 32])
            );
            assert_manifest_field_is_bound!(
                selected_lowering_completion,
                Some(
                    omega_optimization_core::SelectedLoweringOptimizationCompletionIdentity::from_bytes(
                        [0x53; 32]
                    )
                )
            );
            assert_manifest_field_is_bound!(
                function_relative_layout_selections,
                omega_optimization_core::OptimizationSelectionIdentity::from_bytes([0x54; 32])
            );
            assert_manifest_field_is_bound!(
                pre_physical_manifest,
                omega_optimization_core::PrePhysicalOptimizationManifestIdentity::from_bytes(
                    [0x54; 32]
                )
            );
            assert_manifest_field_is_bound!(
                post_allocation_manifest,
                omega_optimization_core::PostAllocationOptimizationManifestIdentity::from_bytes(
                    [0x55; 32]
                )
            );
            assert_manifest_field_is_bound!(
                selected,
                omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity::from_bytes(
                    [0x56; 32]
                )
            );
            assert_manifest_field_is_bound!(
                pre_allocation_machine_effects,
                omega_machine_optimizer::TerminalPreAllocationMachineEffectIdentity::from_bytes(
                    [0x57; 32]
                )
            );
            assert_manifest_field_is_bound!(
                post_allocation_machine,
                omega_machine_optimizer::TerminalPostAllocationMachineIdentity::from_bytes(
                    [0x58; 32]
                )
            );
            assert_manifest_field_is_bound!(
                pre_layout,
                TerminalSelectedFormEncodingIdentity::from_bytes([0x59; 32])
            );
            assert_manifest_field_is_bound!(
                baseline_resolved_layout,
                TerminalResolvedSelectedFormLayoutIdentity::from_bytes([0x5a; 32])
            );
            assert_manifest_field_is_bound!(
                resolved_layout,
                TerminalResolvedSelectedFormLayoutIdentity::from_bytes([0x5b; 32])
            );
            assert_manifest_field_is_bound!(
                x86_branch_relaxation,
                Some(TerminalX86BranchRelaxationIdentity::from_bytes([0x5c; 32]))
            );
            assert_manifest_field_is_bound!(
                whole_function_exit_contract,
                TerminalWholeFunctionExitContractIdentity::from_bytes([0x5d; 32])
            );
            assert_manifest_field_is_bound!(
                target,
                if target == NativeTarget::linux_x64() {
                    NativeTarget::linux_arm64()
                } else {
                    NativeTarget::linux_x64()
                }
            );
            let original_bytes = corrupted.manifest().record().statistics.bytes;
            corrupted.manifest_mut().record_mut().statistics.bytes = original_bytes + 1;
            assert_eq!(
                validate_selected_lowering_function_relative_realization_custody(&corrupted),
                Err(FunctionRelativeOptimizationRealizationError::RootMismatch)
            );
            corrupted.manifest_mut().record_mut().statistics.bytes = original_bytes;
            let original_result_view = corrupted.exit_contract().contract().result_view;
            corrupted.exit_contract_mut().contract_mut().result_view = RegisterViewId(u16::MAX);
            assert_eq!(
                validate_selected_lowering_function_relative_realization_custody(&corrupted),
                Err(FunctionRelativeOptimizationRealizationError::ExitContract(
                    TerminalWholeFunctionExitContractError::ArtifactMismatch
                ))
            );
            corrupted.exit_contract_mut().contract_mut().result_view = original_result_view;
            let original_exit_identity = corrupted.exit_contract().identity();
            corrupted.exit_contract_mut().contract_mut().identity =
                TerminalWholeFunctionExitContractIdentity::from_bytes([0x61; 32]);
            assert_eq!(
                validate_selected_lowering_function_relative_realization_custody(&corrupted),
                Err(FunctionRelativeOptimizationRealizationError::ExitContract(
                    TerminalWholeFunctionExitContractError::ArtifactMismatch
                ))
            );
            corrupted.exit_contract_mut().contract_mut().identity = original_exit_identity;
            let original_offset =
                corrupted.exit_contract().contract().functions[0].returns[0].offset;
            corrupted.exit_contract_mut().contract_mut().functions[0].returns[0].offset =
                original_offset + 1;
            assert_eq!(
                validate_selected_lowering_function_relative_realization_custody(&corrupted),
                Err(FunctionRelativeOptimizationRealizationError::ExitContract(
                    TerminalWholeFunctionExitContractError::ArtifactMismatch
                ))
            );
            corrupted.exit_contract_mut().contract_mut().functions[0].returns[0].offset =
                original_offset;
            assert_eq!(
                validate_selected_lowering_function_relative_realization_custody(&corrupted)
                    .unwrap(),
                *corrupted.custody()
            );
        }
    }

    #[test]
    fn frameless_exit_contract_rejects_unpreserved_x86_callee_saved_write() {
        let target = NativeTarget::linux_x64();
        let selections = OptimizationSelections::new([
            Optimization::CopyPropagation,
            Optimization::SelectedIncomingU12ExactAddImmediate,
        ])
        .unwrap();
        let legality = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_exact_add_conditional_with_selections(
                    target,
                    selections,
                    selected_lowering_budget(),
                ))
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let run = run_selected_lowering_optimizations(legality).unwrap();
        assert!(run.steps().is_empty());
        let homes = stage_optimized_register_homes_after_selected_lowering(run).unwrap();
        let rbx_units = homes
            .selected_lowering_run()
            .source_legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment()
            .physical()
            .model()
            .view_named("rbx")
            .unwrap()
            .units
            .clone();
        let error = stage_selected_lowering_function_relative_realization(homes).unwrap_err();
        let FunctionRelativeOptimizationRealizationError::ExitContract(
            TerminalWholeFunctionExitContractError::CalleeSavedWrite { instruction, unit },
        ) = error
        else {
            panic!("unpreserved RBX write must fail at the whole-function exit contract")
        };
        assert_eq!(
            instruction,
            omega_terminal_selected_instructions::TerminalSelectedInstructionId(3)
        );
        assert!(rbx_units.contains(&unit));
    }

    #[test]
    fn compiler_facing_physical_pipeline_routes_psi_only_and_selected_lowering_suites() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let (semantic, proof) = conditional_exact_binary_artifact(false);
            let psi_only_selections =
                OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                ExplicitOptimizationRequest::new(
                    psi_only_selections.clone(),
                    selected_lowering_budget(),
                )
                .unwrap(),
            )
            .unwrap();
            let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
                optimized,
                target,
                &[],
            )
            .unwrap();
            assert!(matches!(
                staged,
                StagedOptimizedVerifiedPhysicalPipeline::PsiOnly { .. }
            ));
            assert_eq!(staged.selections(), psi_only_selections.identity());
            assert_eq!(staged.selected_lowering_completion(), None);
            assert!(staged.function_relative_realization().is_none());
            let report = optimization_pipeline_report(&staged);
            assert_eq!(
                report.pre_physical().identity,
                staged.pre_physical_manifest().record().identity
            );
            assert_eq!(
                report.post_allocation().identity,
                staged.post_allocation_manifest().record().identity
            );
            assert!(report.function_relative().is_none());
            assert_eq!(
                report.render_human_text(OptimizationReportRequest::Suppressed),
                None
            );
            let text = report
                .render_human_text(OptimizationReportRequest::EmitHumanText)
                .expect("explicit human report projection");
            assert!(text.contains("[pre-physical]"));
            assert!(text.contains("[post-allocation]"));
            assert!(!text.contains("[function-relative realization]"));

            for selections in [
                OptimizationSelections::new([
                    Optimization::CopyPropagation,
                    Optimization::SelectedIncomingU12ExactAddImmediate,
                ])
                .unwrap(),
                OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate])
                    .unwrap(),
            ] {
                let optimized = optimize_artifact_sections(
                    &semantic,
                    &proof,
                    &AdmissionProfile::default(),
                    ExplicitOptimizationRequest::new(
                        selections.clone(),
                        selected_lowering_budget(),
                    )
                    .unwrap(),
                )
                .unwrap();
                let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
                    optimized,
                    target,
                    &[],
                )
                .unwrap();
                let StagedOptimizedVerifiedPhysicalPipeline::SelectedLowering { realization } =
                    &staged
                else {
                    panic!("selected-lowering phase must run when its exact family is selected")
                };
                let homes = realization.homes();
                let machine = realization.machine();
                assert_eq!(staged.selections(), selections.identity());
                assert_eq!(
                    staged.selected_lowering_completion(),
                    Some(homes.selected_lowering_run().custody().identity())
                );
                assert_eq!(
                    staged.function_relative_realization().unwrap().custody(),
                    realization.custody()
                );
                assert!(homes.selected_lowering_run().steps().is_empty());
                assert_eq!(
                    machine.machine().receipt().post_allocation_manifest(),
                    homes.post_allocation_manifest().record().identity
                );
                assert_eq!(
                    realization.manifest().record().selections,
                    selections.identity()
                );
                assert_eq!(
                    realization.manifest().record().publication,
                    FunctionRelativeOptimizationUnavailableData::Unavailable
                );
                let report = optimization_pipeline_report(&staged);
                assert_eq!(
                    report.pre_physical().identity,
                    staged.pre_physical_manifest().record().identity
                );
                assert_eq!(
                    report.post_allocation().identity,
                    staged.post_allocation_manifest().record().identity
                );
                assert_eq!(
                    report
                        .function_relative()
                        .expect("selected lowering has function-relative custody")
                        .identity,
                    realization.manifest().record().identity
                );
                assert!(
                    report
                        .render_human_text(OptimizationReportRequest::EmitHumanText)
                        .expect("explicit human report projection")
                        .contains("[function-relative realization]")
                );
            }
        }
    }

    #[test]
    fn compiler_facing_physical_pipeline_runs_only_the_named_shared_entry_copy() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let (semantic, proof) = conditional_forwarded_parameter_artifact();
            let selections = OptimizationSelections::new([
                Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
            ])
            .unwrap();
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget())
                    .unwrap(),
            )
            .unwrap();
            let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
                optimized,
                target,
                &[],
            )
            .unwrap();
            let StagedOptimizedVerifiedPhysicalPipeline::AllocationRecovery { homes, machine } =
                &staged
            else {
                panic!("the exact allocation-recovery phase must use its fixed-copy route")
            };
            let reanalysis = homes.reanalysis_stage();
            let copies = reanalysis.transformation_stage();
            let plan = copies.copies().plan();
            assert_eq!(staged.selections(), selections.identity());
            assert_eq!(staged.selected_lowering_completion(), None);
            assert!(staged.function_relative_manifest().is_none());
            assert!(staged.post_allocation_machine_optimization().is_none());
            assert_eq!(
                copies.custody().policy(),
                TerminalFixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1
            );
            assert_eq!(copies.custody().copy_count(), 1);
            assert_eq!(plan.copies.len(), 1);
            assert_eq!(plan.copies[0].destinations.len(), 2);
            assert_eq!(reanalysis.custody().entry_transition_count(), 0);
            assert_eq!(reanalysis.legality().receipt().entry_transition_count(), 0);
            assert_eq!(
                machine.machine().receipt().post_allocation_manifest(),
                homes.post_allocation_manifest().record().identity
            );
            assert_eq!(
                &validate_optimized_post_allocation_machine_plan_after_fixed_view_copy_custody(
                    homes, machine,
                )
                .unwrap(),
                machine.custody()
            );
        }
    }

    #[test]
    fn compiler_facing_physical_pipeline_runs_only_the_named_active_resident_rematerialization() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact();
            let selections = OptimizationSelections::new([
                Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            ])
            .unwrap();
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget())
                    .unwrap(),
            )
            .unwrap();
            let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
                optimized,
                target,
                &[],
            )
            .unwrap();
            let StagedOptimizedVerifiedPhysicalPipeline::ActiveResidentRematerialization {
                realization,
            } = &staged
            else {
                panic!("the exact rematerialization selection must use its owning realization")
            };
            let rematerialization = realization.source().pre_layout().source();
            let manifest = realization.manifest().record();
            let empty = OptimizationSelections::default().identity();
            assert_eq!(staged.selections(), selections.identity());
            assert_eq!(staged.selected_lowering_completion(), None);
            assert!(staged.function_relative_realization().is_none());
            assert_eq!(
                staged
                    .active_resident_rematerialization_function_relative_realization()
                    .unwrap()
                    .custody(),
                realization.custody()
            );
            assert_eq!(
                staged.function_relative_manifest(),
                Some(realization.manifest())
            );
            assert!(staged.post_allocation_machine_optimization().is_none());
            assert_eq!(
                manifest.allocation_recovery_selections,
                selections.identity()
            );
            assert_eq!(manifest.selected_lowering_selections, empty);
            assert_eq!(manifest.post_allocation_machine_selections, empty);
            assert_eq!(manifest.function_relative_layout_selections, empty);
            assert_eq!(manifest.selected_lowering_completion, None);
            assert_eq!(rematerialization.custody().applied_count(), 1);
            assert_eq!(rematerialization.custody().rewritten_use_count(), 2);
            assert_eq!(
                staged
                    .post_allocation_manifest()
                    .record()
                    .selected_transformations,
                [
                    PostAllocationSelectedTransformation::PressureRematerialization(
                        rematerialization.rematerialization().receipt().identity(),
                    )
                ]
            );
            assert_eq!(
                staged.machine().machine().receipt().selected(),
                manifest.selected
            );
            assert_eq!(
                manifest.publication,
                FunctionRelativeOptimizationUnavailableData::Unavailable
            );
        }
    }

    #[test]
    fn allocation_recovery_compositions_reject_instead_of_dispatching_a_hidden_policy() {
        let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact();
        for selections in [
            OptimizationSelections::new([
                Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
                Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            ])
            .unwrap(),
            OptimizationSelections::new([
                Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
                Optimization::SelectedIncomingU12ExactAddImmediate,
            ])
            .unwrap(),
        ] {
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
            )
            .unwrap();
            assert!(matches!(
                stage_optimized_verified_physical_pipeline_with_provider_executions(
                    optimized,
                    NativeTarget::linux_x64(),
                    &[],
                ),
                Err(OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition)
            ));
        }
    }

    #[test]
    fn compiler_facing_physical_pipeline_runs_only_the_named_aarch64_cbnz_fusion() {
        let target = NativeTarget::linux_arm64();
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let selections = OptimizationSelections::new([
            Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        ])
        .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget())
                .unwrap(),
        )
        .unwrap();
        let mut staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } =
            &staged
        else {
            panic!("the exact post-allocation phase must use its symbolic machine route")
        };
        let homes = realization.homes();
        let machine = realization.machine();
        let optimization = realization.fusion();
        assert_eq!(staged.selections(), selections.identity());
        assert_eq!(staged.selected_lowering_completion(), None);
        assert_eq!(
            staged.function_relative_manifest(),
            Some(realization.manifest())
        );
        assert_eq!(
            validate_aarch64_cbnz_function_relative_realization_custody(realization).unwrap(),
            *realization.custody()
        );
        let manifest = realization.manifest().record();
        assert_eq!(
            manifest.post_allocation_machine_selections,
            selections.identity()
        );
        assert_eq!(
            manifest.function_relative_layout_selections,
            OptimizationSelections::default().identity()
        );
        assert_eq!(
            manifest.baseline_pre_layout,
            realization.baseline_encoding().identity()
        );
        assert_eq!(manifest.pre_layout, realization.encoding().identity());
        assert_eq!(
            manifest.baseline_resolved_layout,
            realization.baseline_layout().identity()
        );
        assert_eq!(manifest.resolved_layout, realization.layout().identity());
        assert_eq!(
            manifest.aarch64_cbnz_fusion,
            Some(realization.fusion().fusion().receipt().identity())
        );
        assert_eq!(manifest.x86_branch_relaxation, None);
        assert!(matches!(
            realization.exit_contract().contract().layout_custody,
            TerminalWholeFunctionExitLayoutCustody::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 { fusion }
                if fusion == realization.fusion().fusion().receipt().identity()
        ));
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&manifest.encode()),
            Ok(manifest.clone())
        );
        assert!(
            optimization_pipeline_report(&staged)
                .function_relative()
                .is_some()
        );
        assert_eq!(optimization.fusion().receipt().action_count(), 1);
        assert_eq!(
            validate_optimized_aarch64_cbnz_fusion_custody(homes, machine, optimization).unwrap(),
            optimization.custody()
        );
        assert_eq!(
            optimization.custody().post_allocation_machine_selections(),
            selections.identity()
        );

        let ranges = homes.legality_stage().live_range_stage();
        let selected_stage = ranges.liveness_stage().selected_stage();
        let physical = selected_stage.register_environment().physical();
        let baseline_encoding = stage_optimized_layout_independent_selected_form_encoding(
            selected_stage.selected(),
            machine,
            physical,
        )
        .unwrap();
        let baseline_layout = stage_optimized_resolved_selected_form_layout(
            selected_stage.selected(),
            machine,
            physical,
            &baseline_encoding,
        )
        .unwrap();
        let fused_encoding =
            stage_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion(
                selected_stage.selected(),
                machine,
                physical,
                optimization,
            )
            .unwrap();
        let fused_layout = stage_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion(
            selected_stage.selected(),
            machine,
            physical,
            &fused_encoding,
            optimization,
        )
        .unwrap();
        let action = &optimization.fusion().plan().actions[0];
        assert_eq!(
            fused_encoding.machine_optimization().unwrap().fusion(),
            optimization.fusion().receipt().identity()
        );
        assert_eq!(
            fused_layout.machine_optimization(),
            fused_encoding.machine_optimization()
        );
        assert_ne!(baseline_encoding.identity(), fused_encoding.identity());
        assert_ne!(baseline_layout.identity(), fused_layout.identity());
        assert_eq!(
            baseline_layout.functions()[0].byte_count,
            fused_layout.functions()[0].byte_count + 4
        );
        let fused_rows = fused_layout.functions()[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .map(|row| (row.instruction, row))
            .collect::<std::collections::BTreeMap<_, _>>();
        assert!(fused_rows[&action.compare].bytes.is_empty());
        assert!(fused_rows[&action.compare].branch.is_none());
        let branch = fused_rows[&action.branch];
        assert_eq!(branch.bytes.len(), 4);
        let source_register = physical
            .model()
            .views
            .iter()
            .find(|view| view.id == action.source_read.view)
            .unwrap()
            .name
            .strip_prefix('x')
            .unwrap()
            .parse::<u32>()
            .unwrap();
        assert_eq!(
            u32::from_le_bytes(branch.bytes.as_slice().try_into().unwrap()) & 0xff00_001f,
            0xb500_0000 | source_register
        );
        assert_eq!(
            branch.branch.as_ref().unwrap().decoded_register_reads,
            [action.source_read.view]
        );
        assert!(
            branch
                .branch
                .as_ref()
                .unwrap()
                .decoded_effects
                .implicit_unit_uses
                .iter()
                .all(|unit| !action.nzcv_units.contains(unit))
        );
        validate_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion(
            selected_stage.selected(),
            machine,
            physical,
            optimization,
            &fused_encoding,
        )
        .unwrap();
        validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion(
            selected_stage.selected(),
            machine,
            physical,
            &fused_encoding,
            optimization,
            &fused_layout,
        )
        .unwrap();
        assert!(matches!(
            validate_terminal_whole_function_exit_contract(
                selected_stage.selected(),
                machine,
                physical,
                &fused_encoding,
                &fused_layout,
                realization.exit_contract(),
            ),
            Err(TerminalWholeFunctionExitContractError::Layout(
                OptimizedResolvedSelectedFormLayoutError::PreLayout(
                    OptimizedSelectedFormEncodingError::ArtifactMismatch
                )
            ))
        ));

        let mut corrupt_encoding = fused_encoding.clone();
        let branch_disposition = &mut corrupt_encoding
            .rows_mut()
            .iter_mut()
            .find(|row| row.instruction == action.branch)
            .unwrap()
            .machine_disposition;
        let omega_machine_optimizer::TerminalAarch64CbnzInstructionDisposition::
            FusedBranchNonZeroToCbnzV1 { source_read, .. } = branch_disposition
        else {
            panic!("expected fused branch disposition")
        };
        source_read.view = physical.model().view_named("x2").unwrap().id;
        assert_eq!(
            validate_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion(
                selected_stage.selected(),
                machine,
                physical,
                optimization,
                &corrupt_encoding,
            ),
            Err(OptimizedSelectedFormEncodingError::ArtifactMismatch)
        );

        let mut corrupt_layout = fused_layout.clone();
        corrupt_layout.functions_mut()[0]
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find(|row| row.instruction == action.branch)
            .unwrap()
            .bytes[0] ^= 0x20;
        assert_eq!(
            validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion(
                selected_stage.selected(),
                machine,
                physical,
                &fused_encoding,
                optimization,
                &corrupt_layout,
            ),
            Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)
        );

        let mut rehashed_corruption = optimization.fusion().plan().clone();
        rehashed_corruption.actions[0].source_read.view =
            physical.model().view_named("x2").unwrap().id;
        rehashed_corruption.identity =
            omega_machine_optimizer::terminal_aarch64_cbnz_fusion_identity(&rehashed_corruption);
        assert_eq!(
            omega_machine_optimizer::validate_aarch64_cbnz_fusion(
                selected_stage.selected(),
                ranges.liveness_stage().liveness(),
                machine.machine(),
                physical,
                rehashed_corruption,
            ),
            Err(omega_machine_optimizer::TerminalAarch64CbnzFusionError::ArtifactMismatch)
        );

        let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } =
            &mut staged
        else {
            unreachable!()
        };
        let original_layout = realization.manifest().record().resolved_layout;
        realization.manifest_mut().record_mut().resolved_layout =
            realization.baseline_layout().identity();
        assert_eq!(
            validate_aarch64_cbnz_function_relative_realization_custody(realization),
            Err(FunctionRelativeOptimizationRealizationError::RootMismatch)
        );
        realization.manifest_mut().record_mut().resolved_layout = original_layout;
        let original_custody = realization.exit_contract().contract().layout_custody;
        realization
            .exit_contract_mut()
            .contract_mut()
            .layout_custody = TerminalWholeFunctionExitLayoutCustody::BaselineNearLayoutV1;
        assert!(matches!(
            validate_aarch64_cbnz_function_relative_realization_custody(realization),
            Err(FunctionRelativeOptimizationRealizationError::ExitContract(
                TerminalWholeFunctionExitContractError::ArtifactMismatch
            ))
        ));
        realization
            .exit_contract_mut()
            .contract_mut()
            .layout_custody = original_custody;
        assert_eq!(
            validate_aarch64_cbnz_function_relative_realization_custody(realization).unwrap(),
            *realization.custody()
        );
    }

    #[test]
    fn aarch64_cbnz_fusion_composes_after_exact_selected_lowering() {
        let target = NativeTarget::linux_arm64();
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let selections = OptimizationSelections::new([
            Optimization::SelectedIncomingU12ExactAddImmediate,
            Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        ])
        .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget())
                .unwrap(),
        )
        .unwrap();
        let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::SelectedLoweringPostAllocationMachine {
            realization,
        } = &staged
        else {
            panic!("selected lowering must retain custody before post-allocation fusion")
        };
        let homes = realization.homes();
        let machine = realization.machine();
        let optimization = realization.fusion();
        assert_eq!(staged.selections(), selections.identity());
        assert_eq!(
            staged.selected_lowering_completion(),
            Some(homes.selected_lowering_run().custody().identity())
        );
        assert_eq!(optimization.fusion().receipt().action_count(), 1);
        assert_eq!(
            staged.function_relative_manifest(),
            Some(realization.manifest())
        );
        assert_eq!(
            validate_selected_lowering_aarch64_cbnz_function_relative_realization_custody(
                realization
            )
            .unwrap(),
            *realization.custody()
        );
        assert_eq!(
            realization.manifest().record().selected_lowering_completion,
            staged.selected_lowering_completion()
        );
        assert_eq!(
            realization.manifest().record().aarch64_cbnz_fusion,
            Some(optimization.fusion().receipt().identity())
        );
        assert_eq!(
            validate_optimized_aarch64_cbnz_fusion_after_selected_lowering_custody(
                homes,
                machine,
                optimization,
            )
            .unwrap(),
            optimization.custody()
        );
    }

    #[test]
    fn function_relative_only_rel8_suite_shrinks_and_replays_without_selected_lowering() {
        let target = NativeTarget::linux_x64();
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let selections =
            OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1])
                .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget())
                .unwrap(),
        )
        .unwrap();
        let mut staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap();
        assert_eq!(staged.selections(), selections.identity());
        assert_eq!(staged.selected_lowering_completion(), None);
        assert!(staged.function_relative_realization().is_none());
        assert!(
            optimization_pipeline_report(&staged)
                .function_relative()
                .is_some()
        );
        let StagedOptimizedVerifiedPhysicalPipeline::FunctionRelativeLayout { realization } =
            &mut staged
        else {
            panic!("the exact function-relative phase must use its direct realization route")
        };
        assert_eq!(
            validate_function_relative_layout_optimization_realization_custody(realization)
                .unwrap(),
            *realization.custody()
        );
        assert_eq!(realization.relaxation().actions().len(), 1);
        assert_eq!(
            realization
                .baseline_layout()
                .functions()
                .iter()
                .map(|function| function.byte_count)
                .sum::<u64>()
                .checked_sub(
                    realization
                        .layout()
                        .functions()
                        .iter()
                        .map(|function| function.byte_count)
                        .sum::<u64>()
                ),
            Some(4)
        );
        let relaxed_branch = realization
            .layout()
            .functions()
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .find(|instruction| instruction.branch.is_some())
            .unwrap();
        assert_eq!(&relaxed_branch.bytes[..1], [0x75]);

        let manifest = realization.manifest().record();
        assert_eq!(
            manifest.selected_lowering_selections,
            OptimizationSelections::default().identity()
        );
        assert_eq!(manifest.selected_lowering_completion, None);
        assert_eq!(
            manifest.function_relative_layout_selections,
            selections.identity()
        );
        assert_eq!(
            manifest.baseline_resolved_layout,
            realization.baseline_layout().identity()
        );
        assert_eq!(manifest.resolved_layout, realization.layout().identity());
        assert_eq!(
            manifest.x86_branch_relaxation,
            Some(realization.relaxation().identity())
        );
        assert!(matches!(
            realization.exit_contract().contract().layout_custody,
            TerminalWholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
                relaxation
            } if relaxation == realization.relaxation().identity()
        ));
        let original = realization.manifest().record().resolved_layout;
        realization.manifest_mut().record_mut().resolved_layout =
            realization.baseline_layout().identity();
        assert_eq!(
            validate_function_relative_layout_optimization_realization_custody(realization),
            Err(FunctionRelativeOptimizationRealizationError::RootMismatch)
        );
        realization.manifest_mut().record_mut().resolved_layout = original;
        assert_eq!(
            validate_function_relative_layout_optimization_realization_custody(realization)
                .unwrap(),
            *realization.custody()
        );
    }

    #[test]
    fn selected_lowering_and_rel8_phases_retain_both_completion_receipts() {
        let target = NativeTarget::linux_x64();
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let selections = OptimizationSelections::new([
            Optimization::SelectedIncomingU12ExactAddImmediate,
            Optimization::X86RelaxConditionalBranchesToRel8V1,
        ])
        .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget())
                .unwrap(),
        )
        .unwrap();
        let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::SelectedLowering { realization } = &staged
        else {
            panic!("the selected-lowering phase remains the owning physical route")
        };
        assert_eq!(
            validate_selected_lowering_function_relative_realization_custody(realization).unwrap(),
            *realization.custody()
        );
        let relaxation = realization
            .relaxation()
            .expect("the independently selected layout phase must execute");
        assert_eq!(relaxation.actions().len(), 1);
        let manifest = realization.manifest().record();
        assert_eq!(
            manifest.selected_lowering_completion,
            staged.selected_lowering_completion()
        );
        assert_eq!(
            manifest.function_relative_layout_selections,
            OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1,])
                .unwrap()
                .identity()
        );
        assert_eq!(manifest.x86_branch_relaxation, Some(relaxation.identity()));
        assert_eq!(manifest.resolved_layout, relaxation.layout().identity());
        assert_eq!(
            manifest.baseline_resolved_layout,
            realization.baseline_layout().identity()
        );
    }

    #[test]
    fn relocation_free_rel8_fragment_emission_retains_bytes_fuel_and_manifest_custody() {
        let target = NativeTarget::linux_x64();
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let selections =
            OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1])
                .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
        )
        .unwrap();
        let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::FunctionRelativeLayout { realization } =
            physical
        else {
            panic!("rel8 must complete its direct function-relative realization")
        };
        let mut emitted = stage_optimized_function_fragment_emission(
            StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(Box::new(realization)),
        )
        .unwrap();
        assert_eq!(
            validate_optimized_function_fragment_emission(&emitted).unwrap(),
            emitted.custody()
        );
        let fragments = emitted.fragments();
        assert_eq!(fragments.functions.len(), 1);
        let function = &fragments.functions[0];
        let flattened = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .flat_map(|row| row.bytes.iter().copied())
            .collect::<Vec<_>>();
        assert_eq!(flattened, function.bytes);
        let branch = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|row| row.branch.is_some())
            .unwrap();
        assert_eq!(branch.bytes[0], 0x75);
        let omega_terminal_machine_code::TerminalFunctionFragmentControlProvenance::ConditionalBranch {
            when_nonzero,
            when_zero,
        } = &branch.control else {
            panic!("resolved rel8 branch must retain both semantic successors")
        };
        assert!(!when_nonzero.fuel.is_empty());
        assert!(!when_zero.fuel.is_empty());
        let record = emitted.manifest().record();
        assert_eq!(
            record.source_kind,
            FunctionFragmentEmissionSourceKind::X86Rel8V1
        );
        assert_eq!(record.fragments, fragments.identity);
        assert_eq!(record.statistics.zero_byte_instruction_spans, 0);
        assert!(record.statistics.logical_fuel_settlements > 0);
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&record.encode()),
            Ok(record.clone())
        );
        let mut trailing = record.encode();
        trailing.push(0);
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&trailing),
            Err(FunctionFragmentEmissionManifestDecodeError::TrailingBytes)
        );
        let mut stale_identity = record.encode();
        stale_identity[12] ^= 1;
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&stale_identity),
            Err(FunctionFragmentEmissionManifestDecodeError::IdentityMismatch)
        );
        let mut wrong_version = record.encode();
        wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&wrong_version),
            Err(FunctionFragmentEmissionManifestDecodeError::UnsupportedVersion(2))
        );
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&record.encode()[..20]),
            Err(FunctionFragmentEmissionManifestDecodeError::Truncated)
        );

        let original_control = emitted.fragments().functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|row| row.branch.is_some())
            .unwrap()
            .control
            .clone();
        let branch = emitted.fragments_mut().functions[0]
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find(|row| row.branch.is_some())
            .unwrap();
        let omega_terminal_machine_code::TerminalFunctionFragmentControlProvenance::ConditionalBranch {
            when_nonzero,
            ..
        } = &mut branch.control else {
            unreachable!()
        };
        when_nonzero.fuel.clear();
        let fuel_corruption_identity = emitted.fragments().recomputed_identity();
        assert_ne!(fuel_corruption_identity, emitted.custody().fragments());
        emitted.fragments_mut().identity = fuel_corruption_identity;
        assert_eq!(
            validate_optimized_function_fragment_emission(&emitted),
            Err(FunctionFragmentEmissionError::ArtifactMismatch)
        );
        let branch = emitted.fragments_mut().functions[0]
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find(|row| row.branch.is_some())
            .unwrap();
        branch.control = original_control;
        let restored_identity = emitted.fragments().recomputed_identity();
        emitted.fragments_mut().identity = restored_identity;
        assert_eq!(
            validate_optimized_function_fragment_emission(&emitted).unwrap(),
            emitted.custody()
        );

        let row = emitted.fragments_mut().functions[0].blocks[0]
            .instructions
            .iter_mut()
            .find(|row| !row.bytes.is_empty())
            .unwrap();
        row.bytes[0] ^= 1;
        let corrupted_identity = emitted.fragments().recomputed_identity();
        emitted.fragments_mut().identity = corrupted_identity;
        assert_eq!(
            validate_optimized_function_fragment_emission(&emitted),
            Err(FunctionFragmentEmissionError::ArtifactMismatch)
        );
    }

    #[test]
    fn relocation_free_cbnz_fragment_emission_retains_the_elided_compare_span() {
        let target = NativeTarget::linux_arm64();
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let selections = OptimizationSelections::new([
            Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        ])
        .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
        )
        .unwrap();
        let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } =
            physical
        else {
            panic!("CBNZ must complete its direct function-relative realization")
        };
        let mut emitted = stage_optimized_function_fragment_emission(
            StagedOptimizedFunctionFragmentEmissionSource::Aarch64CbnzDirect(Box::new(realization)),
        )
        .unwrap();
        let function = &emitted.fragments().functions[0];
        let rows = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .collect::<Vec<_>>();
        let compare = rows
            .iter()
            .find(|row| {
                row.alternative.family
                    == omega_terminal_selected_instructions::TerminalMachineAlternativeFamily::CompareI64Zero
            })
            .unwrap();
        let branch = rows.iter().find(|row| row.branch.is_some()).unwrap();
        assert!(compare.bytes.is_empty());
        assert!(compare.provenance.fuel.is_empty());
        assert_eq!(compare.offset, branch.offset);
        assert_eq!(branch.bytes.len(), 4);
        assert_eq!(
            u32::from_le_bytes(branch.bytes.as_slice().try_into().unwrap()) & 0xff00_0000,
            0xb500_0000
        );
        assert_eq!(
            emitted
                .manifest()
                .record()
                .statistics
                .zero_byte_instruction_spans,
            1
        );
        assert_eq!(
            emitted.manifest().record().source_kind,
            FunctionFragmentEmissionSourceKind::Aarch64CbnzV1
        );
        assert_eq!(
            validate_optimized_function_fragment_emission(&emitted).unwrap(),
            emitted.custody()
        );

        let block = emitted.fragments_mut().functions[0]
            .blocks
            .iter_mut()
            .find(|block| block.instructions.iter().any(|row| row.bytes.is_empty()))
            .unwrap();
        block.instructions.retain(|row| !row.bytes.is_empty());
        let corrupted_identity = emitted.fragments().recomputed_identity();
        emitted.fragments_mut().identity = corrupted_identity;
        assert_eq!(
            validate_optimized_function_fragment_emission(&emitted),
            Err(FunctionFragmentEmissionError::ArtifactMismatch)
        );
    }

    #[test]
    fn relocation_free_rel8_text_section_replays_bytes_manifest_and_custody() {
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let selections =
            OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1])
                .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
        )
        .unwrap();
        let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            NativeTarget::linux_x64(),
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::FunctionRelativeLayout { realization } =
            physical
        else {
            panic!("rel8 must complete its direct function-relative realization")
        };
        let emitted = stage_optimized_function_fragment_emission(
            StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(Box::new(realization)),
        )
        .unwrap();
        let source_fragments = emitted.fragments().identity;
        let source_bytes = emitted.fragments().functions[0].bytes.clone();
        let mut placed = stage_optimized_relocation_free_text_section(emitted).unwrap();

        assert_eq!(
            validate_optimized_relocation_free_text_section(&placed).unwrap(),
            placed.custody()
        );
        let section = placed.text_section();
        assert_eq!(section.source_fragments, source_fragments);
        assert_eq!(section.section_alignment, 1);
        assert_eq!(section.bytes, source_bytes);
        assert_eq!(section.byte_count, section.bytes.len() as u64);
        assert_eq!(section.functions.len(), 1);
        assert_eq!(section.functions[0].source_function_index, 0);
        assert_eq!(section.functions[0].section_offset, 0);
        assert_eq!(section.semantic_entry_offset, 0);
        let branch = section.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|row| {
                row.alternative.family
                    == omega_terminal_selected_instructions::TerminalMachineAlternativeFamily::ConditionalBranchNonZero
            })
            .unwrap();
        assert_eq!(section.bytes[branch.section_offset as usize], 0x75);
        assert_eq!(branch.function_offset, branch.section_offset);
        assert_eq!(branch.byte_count, 2);
        assert_eq!(
            section.relocation_requirements,
            omega_object_file::TerminalTextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1
        );

        let record = placed.manifest().record();
        assert_eq!(
            record.source_fragment_manifest,
            placed.source().manifest().record().identity
        );
        assert_eq!(record.fragments, source_fragments);
        assert_eq!(record.text_section, section.identity);
        assert_eq!(record.statistics.padding_bytes, 0);
        assert_eq!(record.statistics.relocation_requirements, 0);
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&record.encode()),
            Ok(record.clone())
        );
        let mut trailing = record.encode();
        trailing.push(0);
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&trailing),
            Err(FunctionFragmentTextSectionManifestDecodeError::TrailingBytes)
        );
        let mut wrong_version = record.encode();
        wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&wrong_version),
            Err(FunctionFragmentTextSectionManifestDecodeError::UnsupportedVersion(2))
        );
        let mut stale_identity = record.encode();
        stale_identity[12] ^= 1;
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&stale_identity),
            Err(FunctionFragmentTextSectionManifestDecodeError::IdentityMismatch)
        );
        let mut unknown_relocation = record.encode();
        let relocation_tag = unknown_relocation.len() - 63;
        unknown_relocation[relocation_tag] = 2;
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&unknown_relocation),
            Err(FunctionFragmentTextSectionManifestDecodeError::UnknownRelocationRequirements(2))
        );
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&record.encode()[..20]),
            Err(FunctionFragmentTextSectionManifestDecodeError::Truncated)
        );

        let original_byte = placed.text_section().bytes[0];
        placed.text_section_mut().bytes[0] ^= 1;
        let corrupted_identity = placed.text_section().recomputed_identity();
        placed.text_section_mut().identity = corrupted_identity;
        assert_eq!(
            validate_optimized_relocation_free_text_section(&placed),
            Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch)
        );
        placed.text_section_mut().bytes[0] = original_byte;
        let restored_identity = placed.text_section().recomputed_identity();
        placed.text_section_mut().identity = restored_identity;
        assert_eq!(
            validate_optimized_relocation_free_text_section(&placed).unwrap(),
            placed.custody()
        );

        let original_manifest = placed.manifest().record().clone();
        placed.manifest_mut().record_mut().statistics.padding_bytes = 1;
        let corrupted_manifest = placed.manifest().record().recomputed_identity();
        placed.manifest_mut().record_mut().identity = corrupted_manifest;
        assert_eq!(
            validate_optimized_relocation_free_text_section(&placed),
            Err(RelocationFreeTextSectionPlacementError::ManifestMismatch)
        );
        *placed.manifest_mut().record_mut() = original_manifest;
        assert_eq!(
            validate_optimized_relocation_free_text_section(&placed).unwrap(),
            placed.custody()
        );
        placed.corrupt_custody_for_test();
        assert_eq!(
            validate_optimized_relocation_free_text_section(&placed),
            Err(RelocationFreeTextSectionPlacementError::ReceiptMismatch)
        );
    }

    #[test]
    fn relocation_free_rel8_object_container_reconstructs_replays_and_rejects_corruption() {
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let selections =
            OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1])
                .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
        )
        .unwrap();
        let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            NativeTarget::linux_x64(),
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::FunctionRelativeLayout { realization } =
            physical
        else {
            panic!("rel8 must complete its direct function-relative realization")
        };
        let emitted = stage_optimized_function_fragment_emission(
            StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(Box::new(realization)),
        )
        .unwrap();
        let placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
        let mut staged = stage_optimized_relocation_free_terminal_object_container(placed).unwrap();

        assert_eq!(
            validate_optimized_relocation_free_terminal_object_container(&staged).unwrap(),
            staged.custody()
        );
        let object = staged.object();
        assert_eq!(
            object.text_section.bytes,
            staged.source().text_section().bytes
        );
        assert_eq!(object.text_section.name, ".text");
        assert_eq!(object.text_section.alignment, 1);
        assert_eq!(object.relocation_record_count, 0);
        assert_eq!(object.symbols.len(), object_local_symbol_count(object));
        assert_eq!(object.symbols.len(), 1);
        let entry = &object.symbols[0];
        assert_eq!(entry.symbol, object.semantic_entry_symbol);
        assert_eq!(entry.machine, object.semantic_entry);
        assert_eq!(
            entry.name,
            format!("__omega_terminal_machine_{}", entry.machine.get())
        );
        assert_ne!(entry.name, "main");
        assert_ne!(entry.name, "_main");
        assert_eq!(entry.section_offset, 0);
        assert_eq!(entry.byte_count, object.text_section.byte_count);
        assert_eq!(
            omega_object_file::decode_terminal_relocation_free_object(&staged.container().bytes),
            Ok(object.clone())
        );
        let record = staged.manifest().record();
        assert_eq!(
            FunctionFragmentObjectContainerManifest::decode(&record.encode()),
            Ok(record.clone())
        );
        assert_eq!(record.statistics.sections, 1);
        assert_eq!(record.statistics.external_symbols, 0);
        assert_eq!(record.statistics.relocation_records, 0);
        assert_eq!(record.statistics.text_bytes, object.text_section.byte_count);

        let original_object = staged.object().clone();
        staged.object_mut().symbols[0].name.push_str("_corrupt");
        let corrupted_object_identity = staged.object().recomputed_identity().unwrap();
        staged.object_mut().identity = corrupted_object_identity;
        assert!(matches!(
            validate_optimized_relocation_free_terminal_object_container(&staged),
            Err(RelocationFreeTerminalObjectContainerError::InvalidObject(_))
                | Err(RelocationFreeTerminalObjectContainerError::ArtifactMismatch)
        ));
        *staged.object_mut() = original_object;

        let original_container = staged.container().clone();
        staged.container_mut().bytes[0] ^= 1;
        let corrupted_container_identity =
            omega_optimization_core::TerminalRelocationFreeObjectContainerIdentity::from_canonical_bytes(
                &staged.container().bytes,
            );
        staged.container_mut().identity = corrupted_container_identity;
        assert!(matches!(
            validate_optimized_relocation_free_terminal_object_container(&staged),
            Err(RelocationFreeTerminalObjectContainerError::InvalidContainer(_))
                | Err(RelocationFreeTerminalObjectContainerError::ContainerMismatch)
        ));
        *staged.container_mut() = original_container;

        let original_manifest = staged.manifest().record().clone();
        staged
            .manifest_mut()
            .record_mut()
            .statistics
            .external_symbols = 1;
        let corrupted_manifest_identity = staged.manifest().record().recomputed_identity();
        staged.manifest_mut().record_mut().identity = corrupted_manifest_identity;
        assert_eq!(
            validate_optimized_relocation_free_terminal_object_container(&staged),
            Err(RelocationFreeTerminalObjectContainerError::ManifestMismatch)
        );
        *staged.manifest_mut().record_mut() = original_manifest;
        staged.corrupt_custody_for_test();
        assert_eq!(
            validate_optimized_relocation_free_terminal_object_container(&staged),
            Err(RelocationFreeTerminalObjectContainerError::ReceiptMismatch)
        );
    }

    fn object_local_symbol_count(
        object: &omega_object_file::TerminalRelocationFreeObjectPlan,
    ) -> usize {
        object
            .symbols
            .iter()
            .filter(|symbol| {
                symbol.linkage
                    == omega_object_file::TerminalRelocationFreeObjectSymbolLinkage::ObjectLocalV1
            })
            .count()
    }

    #[test]
    fn relocation_free_cbnz_text_section_preserves_zero_span_and_alignment() {
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let selections = OptimizationSelections::new([
            Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        ])
        .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
        )
        .unwrap();
        let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            NativeTarget::linux_arm64(),
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } =
            physical
        else {
            panic!("CBNZ must complete its direct function-relative realization")
        };
        let emitted = stage_optimized_function_fragment_emission(
            StagedOptimizedFunctionFragmentEmissionSource::Aarch64CbnzDirect(Box::new(realization)),
        )
        .unwrap();
        let mut placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
        let section = placed.text_section();
        assert_eq!(section.section_alignment, 4);
        assert_eq!(section.byte_count % 4, 0);
        assert!(
            section
                .functions
                .iter()
                .all(|function| function.section_offset % 4 == 0 && function.byte_count % 4 == 0)
        );
        let rows = section.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .collect::<Vec<_>>();
        let compare = rows
            .iter()
            .find(|row| {
                row.alternative.family
                    == omega_terminal_selected_instructions::TerminalMachineAlternativeFamily::CompareI64Zero
            })
            .unwrap();
        let branch = rows
            .iter()
            .find(|row| {
                row.alternative.family
                    == omega_terminal_selected_instructions::TerminalMachineAlternativeFamily::ConditionalBranchNonZero
            })
            .unwrap();
        assert_eq!(compare.byte_count, 0);
        assert_eq!(compare.function_offset, branch.function_offset);
        assert_eq!(compare.section_offset, branch.section_offset);
        assert_eq!(branch.byte_count, 4);
        assert_eq!(
            u32::from_le_bytes(
                section.bytes[branch.section_offset as usize..branch.section_offset as usize + 4]
                    .try_into()
                    .unwrap()
            ) & 0xff00_0000,
            0xb500_0000
        );
        assert_eq!(
            placed
                .manifest()
                .record()
                .statistics
                .zero_byte_instruction_spans,
            1
        );

        let compare = placed.text_section_mut().functions[0]
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find(|row| row.byte_count == 0)
            .unwrap();
        compare.byte_count = 4;
        let corrupted_identity = placed.text_section().recomputed_identity();
        placed.text_section_mut().identity = corrupted_identity;
        assert_eq!(
            validate_optimized_relocation_free_text_section(&placed),
            Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch)
        );
    }

    #[test]
    fn relocation_free_cbnz_object_container_retains_zero_span_source_and_private_entry() {
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let selections = OptimizationSelections::new([
            Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        ])
        .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
        )
        .unwrap();
        let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            NativeTarget::linux_arm64(),
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } =
            physical
        else {
            panic!("CBNZ must complete its direct function-relative realization")
        };
        let emitted = stage_optimized_function_fragment_emission(
            StagedOptimizedFunctionFragmentEmissionSource::Aarch64CbnzDirect(Box::new(realization)),
        )
        .unwrap();
        let placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
        let staged = stage_optimized_relocation_free_terminal_object_container(placed).unwrap();
        assert_eq!(staged.object().text_section.alignment, 4);
        assert_eq!(
            staged.object().text_section.bytes,
            staged.source().text_section().bytes
        );
        assert!(
            staged.source().text_section().functions[0]
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| instruction.byte_count == 0)
        );
        assert_eq!(
            staged.object().symbols[0].role,
            omega_object_file::TerminalRelocationFreeObjectSymbolRole::SemanticEntryV1
        );
        assert_eq!(
            validate_optimized_relocation_free_terminal_object_container(&staged).unwrap(),
            staged.custody()
        );
    }

    #[test]
    fn optimized_rel8_terminal_object_artifact_binds_replays_and_reports_without_authority() {
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let terminal = canonical_terminal_artifact(&semantic, &proof);
        let selections =
            OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1])
                .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
        )
        .unwrap();
        let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            NativeTarget::linux_x64(),
            &[],
        )
        .unwrap();
        let physical_report = optimization_pipeline_report(&physical);
        assert_eq!(physical_report.function_fragment(), None);
        assert_eq!(physical_report.text_section(), None);
        assert_eq!(physical_report.object_container(), None);
        assert_eq!(physical_report.terminal_object_artifact(), None);
        let StagedOptimizedVerifiedPhysicalPipeline::FunctionRelativeLayout { realization } =
            physical
        else {
            panic!("rel8 must complete its direct realization")
        };
        let emitted = stage_optimized_function_fragment_emission(
            StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(Box::new(realization)),
        )
        .unwrap();
        let placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
        let object = stage_optimized_relocation_free_terminal_object_container(placed).unwrap();
        let mut staged =
            stage_validated_optimized_terminal_object_artifact(terminal, object).unwrap();

        assert_eq!(
            validate_optimized_terminal_object_artifact(&staged).unwrap(),
            staged.custody()
        );
        let artifact = staged.artifact();
        assert_eq!(artifact.terminal_psi, staged.source().object().terminal_psi);
        assert_eq!(
            artifact.semantic_entry,
            staged.source().object().semantic_entry
        );
        assert_eq!(artifact.statistics.relocation_records, 0);
        assert_eq!(
            artifact.pre_physical_manifest,
            staged
                .source()
                .source()
                .source()
                .function_relative_manifest()
                .record()
                .pre_physical_manifest
        );
        assert_eq!(
            OptimizedTerminalObjectArtifactRecord::decode(&artifact.encode()),
            Ok(artifact.clone())
        );
        let manifest = staged.manifest().record();
        assert_eq!(
            OptimizedTerminalObjectArtifactManifest::decode(&manifest.encode()),
            Ok(manifest.clone())
        );
        assert_eq!(
            manifest.external_entry_bridge,
            OptimizedTerminalObjectArtifactUnavailableData::Unavailable
        );
        assert_eq!(
            manifest.executable_image,
            OptimizedTerminalObjectArtifactUnavailableData::Unavailable
        );
        assert_eq!(
            manifest.installation,
            OptimizedTerminalObjectArtifactUnavailableData::Unavailable
        );
        assert_eq!(
            manifest.publication,
            OptimizedTerminalObjectArtifactUnavailableData::Unavailable
        );

        let artifact_identity = artifact.identity;
        let object_bytes = staged.source().container().bytes.clone();
        let report = optimization_pipeline_report_from_terminal_object_artifact(&staged);
        assert_eq!(
            report.render_human_text(OptimizationReportRequest::Suppressed),
            None
        );
        let rendered = report
            .render_human_text(OptimizationReportRequest::EmitHumanText)
            .unwrap();
        assert!(rendered.contains("[optimized Terminal object artifact]"));
        assert!(rendered.contains("publication: unavailable"));
        assert_eq!(staged.artifact().identity, artifact_identity);
        assert_eq!(staged.source().container().bytes, object_bytes);
        assert_eq!(
            report.function_fragment().unwrap().identity,
            artifact.function_fragment_manifest
        );
        assert_eq!(
            report.text_section().unwrap().identity,
            artifact.text_section_manifest
        );
        assert_eq!(
            report.object_container().unwrap().identity,
            artifact.object_container_manifest
        );
        assert_eq!(
            report.terminal_object_artifact().unwrap().artifact,
            artifact.identity
        );

        let original_artifact = staged.artifact().clone();
        staged.artifact_mut().statistics.relocation_records = 1;
        let corrupted_artifact_identity = staged.artifact().recomputed_identity();
        staged.artifact_mut().identity = corrupted_artifact_identity;
        assert_eq!(
            validate_optimized_terminal_object_artifact(&staged),
            Err(OptimizedTerminalObjectArtifactError::ArtifactMismatch)
        );
        *staged.artifact_mut() = original_artifact;

        let original_manifest = staged.manifest().record().clone();
        staged
            .manifest_mut()
            .record_mut()
            .statistics
            .function_symbols += 1;
        let corrupted_manifest_identity = staged.manifest().record().recomputed_identity();
        staged.manifest_mut().record_mut().identity = corrupted_manifest_identity;
        assert_eq!(
            validate_optimized_terminal_object_artifact(&staged),
            Err(OptimizedTerminalObjectArtifactError::ManifestMismatch)
        );
        *staged.manifest_mut().record_mut() = original_manifest;
        staged.corrupt_custody_for_test();
        assert_eq!(
            validate_optimized_terminal_object_artifact(&staged),
            Err(OptimizedTerminalObjectArtifactError::ReceiptMismatch)
        );
    }

    #[test]
    fn optimized_cbnz_terminal_object_artifact_retains_zero_span_and_rejects_detached_proof() {
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let selections = OptimizationSelections::new([
            Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        ])
        .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget())
                .unwrap(),
        )
        .unwrap();
        let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            NativeTarget::linux_arm64(),
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } =
            physical
        else {
            panic!("CBNZ must complete its direct realization")
        };
        let emitted = stage_optimized_function_fragment_emission(
            StagedOptimizedFunctionFragmentEmissionSource::Aarch64CbnzDirect(Box::new(realization)),
        )
        .unwrap();
        let placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
        let object = stage_optimized_relocation_free_terminal_object_container(placed).unwrap();

        let module = psi_terminal_codec::decode_module(&semantic).unwrap();
        let mut detached_proof = psi_terminal_codec::decode_proof_bundle(&proof).unwrap();
        detached_proof.evidence.pop();
        let detached = psi_terminal_codec::CanonicalTerminalArtifact::from_parts(
            &module,
            &detached_proof,
            None,
        )
        .unwrap();
        assert!(matches!(
            stage_validated_optimized_terminal_object_artifact(detached, object),
            Err(OptimizedTerminalObjectArtifactError::ProofMismatch)
        ));

        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
        )
        .unwrap();
        let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            NativeTarget::linux_arm64(),
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } =
            physical
        else {
            panic!("CBNZ must complete its direct realization")
        };
        let emitted = stage_optimized_function_fragment_emission(
            StagedOptimizedFunctionFragmentEmissionSource::Aarch64CbnzDirect(Box::new(realization)),
        )
        .unwrap();
        let placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
        let object = stage_optimized_relocation_free_terminal_object_container(placed).unwrap();
        let staged = stage_validated_optimized_terminal_object_artifact(
            canonical_terminal_artifact(&semantic, &proof),
            object,
        )
        .unwrap();
        assert!(
            staged.source().source().text_section().functions[0]
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| instruction.byte_count == 0)
        );
        assert_eq!(
            validate_optimized_terminal_object_artifact(&staged).unwrap(),
            staged.custody()
        );

        let mut wrong_magic = staged.artifact().encode();
        wrong_magic[0] ^= 1;
        assert_eq!(
            OptimizedTerminalObjectArtifactRecord::decode(&wrong_magic),
            Err(OptimizedTerminalObjectArtifactRecordDecodeError::WrongMagic)
        );
        let mut wrong_version = staged.manifest().record().encode();
        wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            OptimizedTerminalObjectArtifactManifest::decode(&wrong_version),
            Err(OptimizedTerminalObjectArtifactManifestDecodeError::UnsupportedVersion(2))
        );
        let mut trailing = staged.artifact().encode();
        trailing.push(0);
        assert_eq!(
            OptimizedTerminalObjectArtifactRecord::decode(&trailing),
            Err(OptimizedTerminalObjectArtifactRecordDecodeError::TrailingBytes)
        );
        let mut stale = staged.manifest().record().encode();
        stale[12] ^= 1;
        assert_eq!(
            OptimizedTerminalObjectArtifactManifest::decode(&stale),
            Err(OptimizedTerminalObjectArtifactManifestDecodeError::IdentityMismatch)
        );
    }

    #[test]
    fn relocation_free_text_section_preserves_disconnected_function_order_without_padding() {
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let selections =
            OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1])
                .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
        )
        .unwrap();
        let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            NativeTarget::linux_x64(),
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::FunctionRelativeLayout { realization } =
            physical
        else {
            panic!("rel8 must complete its direct function-relative realization")
        };
        let emitted = stage_optimized_function_fragment_emission(
            StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(Box::new(realization)),
        )
        .unwrap();
        let mut fragments = emitted.fragments().clone();
        let entry = fragments.entry;
        let first_length = fragments.functions[0].byte_count;
        let mut detached = fragments.functions[0].clone();
        detached.machine = MachineId::new(1).unwrap();
        fragments.functions.push(detached);
        fragments.identity = fragments.recomputed_identity();
        let expected_machines = [entry, MachineId::new(1).unwrap()];
        let mut placed =
            crate::function_fragment_text_section::place_fragments_for_test(&fragments).unwrap();
        assert_eq!(
            placed
                .functions
                .iter()
                .map(|function| function.machine)
                .collect::<Vec<_>>(),
            expected_machines
        );
        assert_eq!(placed.functions[0].section_offset, 0);
        assert_eq!(placed.functions[1].section_offset, first_length);
        assert_eq!(placed.semantic_entry, expected_machines[0]);
        assert_eq!(placed.semantic_entry_offset, 0);
        assert_eq!(placed.byte_count, first_length * 2);
        assert_eq!(
            placed.bytes,
            [
                fragments.functions[0].bytes.as_slice(),
                fragments.functions[1].bytes.as_slice(),
            ]
            .concat()
        );

        let replay =
            crate::function_fragment_text_section::place_fragments_for_test(&fragments).unwrap();
        assert_eq!(replay, placed);
        placed.functions.swap(0, 1);
        placed.identity = placed.recomputed_identity();
        assert_ne!(placed, replay);
    }

    #[test]
    fn relocation_free_fragment_emission_accepts_both_selected_lowering_compositions() {
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let x86_selections = OptimizationSelections::new([
            Optimization::SelectedIncomingU12ExactAddImmediate,
            Optimization::X86RelaxConditionalBranchesToRel8V1,
        ])
        .unwrap();
        let x86_optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(x86_selections, selected_lowering_budget()).unwrap(),
        )
        .unwrap();
        let x86_physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            x86_optimized,
            NativeTarget::linux_x64(),
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::SelectedLowering { realization } =
            x86_physical
        else {
            panic!("combined x86 suite must retain selected-lowering custody")
        };
        let x86 = stage_optimized_function_fragment_emission(
            StagedOptimizedFunctionFragmentEmissionSource::X86Rel8AfterSelectedLowering(Box::new(
                realization,
            )),
        )
        .unwrap();
        assert_eq!(
            validate_optimized_function_fragment_emission(&x86).unwrap(),
            x86.custody()
        );
        let x86 = stage_optimized_relocation_free_text_section(x86).unwrap();
        assert_eq!(
            validate_optimized_relocation_free_text_section(&x86).unwrap(),
            x86.custody()
        );
        let x86 = stage_optimized_relocation_free_terminal_object_container(x86).unwrap();
        assert_eq!(
            validate_optimized_relocation_free_terminal_object_container(&x86).unwrap(),
            x86.custody()
        );
        let x86 = stage_validated_optimized_terminal_object_artifact(
            canonical_terminal_artifact(&semantic, &proof),
            x86,
        )
        .unwrap();
        assert_eq!(
            validate_optimized_terminal_object_artifact(&x86).unwrap(),
            x86.custody()
        );

        let arm_selections = OptimizationSelections::new([
            Optimization::SelectedIncomingU12ExactAddImmediate,
            Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        ])
        .unwrap();
        let arm_optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(arm_selections, selected_lowering_budget()).unwrap(),
        )
        .unwrap();
        let arm_physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            arm_optimized,
            NativeTarget::linux_arm64(),
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::SelectedLoweringPostAllocationMachine {
            realization,
        } = arm_physical
        else {
            panic!("combined AArch64 suite must retain both phase completions")
        };
        let arm = stage_optimized_function_fragment_emission(
            StagedOptimizedFunctionFragmentEmissionSource::Aarch64CbnzAfterSelectedLowering(
                Box::new(realization),
            ),
        )
        .unwrap();
        assert_eq!(
            validate_optimized_function_fragment_emission(&arm).unwrap(),
            arm.custody()
        );
        let arm = stage_optimized_relocation_free_text_section(arm).unwrap();
        assert_eq!(
            validate_optimized_relocation_free_text_section(&arm).unwrap(),
            arm.custody()
        );
        let arm = stage_optimized_relocation_free_terminal_object_container(arm).unwrap();
        assert_eq!(
            validate_optimized_relocation_free_terminal_object_container(&arm).unwrap(),
            arm.custody()
        );
        let arm = stage_validated_optimized_terminal_object_artifact(
            canonical_terminal_artifact(&semantic, &proof),
            arm,
        )
        .unwrap();
        assert_eq!(
            validate_optimized_terminal_object_artifact(&arm).unwrap(),
            arm.custody()
        );
    }

    #[test]
    fn rel8_fragment_emission_rejects_selected_lowering_without_the_named_layout_rule() {
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let selections =
            OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate])
                .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
        )
        .unwrap();
        let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            NativeTarget::linux_x64(),
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::SelectedLowering { realization } = physical
        else {
            panic!("selected lowering must retain its completed realization")
        };
        assert!(realization.relaxation().is_none());
        assert!(matches!(
            stage_optimized_function_fragment_emission(
                StagedOptimizedFunctionFragmentEmissionSource::X86Rel8AfterSelectedLowering(
                    Box::new(realization),
                ),
            ),
            Err(FunctionFragmentEmissionError::MissingX86Rel8Realization)
        ));
    }

    #[test]
    fn x86_rel8_selection_rejects_a_non_x86_target_without_a_realization() {
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let selections =
            OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1])
                .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            stage_optimized_verified_physical_pipeline_with_provider_executions(
                optimized,
                NativeTarget::linux_arm64(),
                &[],
            ),
            Err(
                OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization(
                    FunctionRelativeOptimizationRealizationError::X86BranchRelaxation(
                        OptimizedX86BranchRelaxationError::UnsupportedTarget(_)
                    )
                )
            )
        ));
    }

    #[test]
    fn selected_lowering_suite_enforces_one_aggregate_budget() {
        let target = NativeTarget::linux_x64();
        let selections = OptimizationSelections::new([
            Optimization::CopyPropagation,
            Optimization::SelectedIncomingU12ExactAddImmediate,
        ])
        .unwrap();
        let source = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_exact_add_conditional_with_selections(
                    target,
                    selections.clone(),
                    selected_lowering_budget(),
                ))
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let reference = run_selected_lowering_optimizations(source).unwrap();
        let attempt = reference.terminal_attempt();
        let component_usages = [
            attempt.choices().receipt().usage(),
            attempt.recovery().receipt().usage(),
            attempt.fold().receipt().usage(),
        ];
        let maximum = |field: fn(OptimizationWorkUsage) -> u64| {
            component_usages
                .into_iter()
                .map(field)
                .max()
                .unwrap()
                .max(1)
        };
        let component_only_budget = OptimizationWorkBudget::new(
            maximum(|usage| usage.rule_evaluations),
            maximum(|usage| usage.candidates),
            maximum(|usage| usage.validation_steps),
            maximum(|usage| usage.commits),
            maximum(|usage| usage.iterations),
        )
        .unwrap();
        assert!(
            component_usages
                .into_iter()
                .all(|usage| usage.within(component_only_budget))
        );

        let source = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_exact_add_conditional_with_selections(
                    target,
                    selections,
                    component_only_budget,
                ))
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            run_selected_lowering_optimizations(source),
            Err(OptimizedLiteralFoldCustodyError::SelectedLoweringBudgetExceeded { .. })
        ));
    }

    #[test]
    fn selected_lowering_runner_rejects_a_psi_only_source_suite() {
        let legality = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_exact_add_conditional(NativeTarget::linux_x64()))
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            run_selected_lowering_optimizations(legality),
            Err(OptimizedLiteralFoldCustodyError::MissingSelectedLoweringOptimization)
        ));
    }

    #[test]
    fn literal_fold_staging_rejects_an_explicit_no_action_request() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let legality = stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_exact_add_conditional(target)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            assert!(matches!(
                stage_first_optimized_literal_fold(
                    legality,
                    TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                    TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                    TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
                    budget(),
                ),
                Err(OptimizedLiteralFoldCustodyError::NoAppliedFold)
            ));
        }
    }

    fn validate_raw_selection(
        staged: &StagedOptimizedSelectedInstructions,
        raw: omega_terminal_selected_instructions::TerminalSelectedInstructionPlan,
    ) -> Result<
        omega_terminal_target_operations_to_selected_instructions::ValidatedTerminalSelectedInstructions,
        SelectedInstructionError,
    >{
        let constraints = crate::selection::selection_constraints(
            staged.legalized(),
            staged.register_environment(),
        );
        validate_terminal_selected_instructions(
            staged.legalized(),
            &constraints,
            staged.register_environment().physical(),
            staged.register_environment().constraints(),
            raw,
        )
    }

    fn validate_raw_post_allocation(
        source: &StagedOptimizedRegisterHomes,
        staged: &StagedOptimizedPostAllocationMachinePlan,
        raw: omega_machine_optimizer::TerminalPostAllocationMachinePlan,
    ) -> Result<
        omega_machine_optimizer::ValidatedTerminalPostAllocationMachinePlan,
        omega_machine_optimizer::TerminalPostAllocationMachineError,
    > {
        let selected = source
            .legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage();
        let environment = selected.register_environment();
        omega_machine_optimizer::validate_terminal_post_allocation_machine_plan(
            selected.selected(),
            staged.effects().effects(),
            source.legality_stage().live_range_stage().ranges(),
            source.legality_stage().legality(),
            source.homes(),
            source.post_allocation_manifest(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            raw,
        )
    }

    #[test]
    fn selected_cfg_validator_rejects_target_state_path_and_value_corruption() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let staged = staged_conditional(target);

            let mut corrupted = staged.selected().plan().clone();
            corrupted.functions[0].blocks[0].instructions[0]
                .implicit_defs
                .clear();
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::ConstraintEffectMismatch { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            let TerminalSelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } = &mut corrupted.functions[0].blocks[0].terminator
            else {
                unreachable!()
            };
            std::mem::swap(when_nonzero, when_zero);
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::SuccessorProjectionMismatch { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            corrupted.functions[0].virtual_registers[0].entry_fixed_view = None;
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::VirtualRegisterProjectionMismatch { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            let TerminalSelectedTerminator::Return { instruction, .. } =
                &mut corrupted.functions[0].blocks[1].terminator
            else {
                unreachable!()
            };
            instruction.operands[0].fixed_view = None;
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                    | Err(SelectedInstructionError::ConstraintOperandMismatch { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            corrupted.functions[0].blocks[1].instructions[0].operands[0].tied_to = Some(0);
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                    | Err(SelectedInstructionError::ConstraintOperandMismatch { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            corrupted.functions[0].blocks[1].instructions[0].operands[0].early_clobber = true;
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                    | Err(SelectedInstructionError::ConstraintOperandMismatch { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            let TerminalSelectedTerminator::Return { instruction, .. } =
                &mut corrupted.functions[0].blocks[1].terminator
            else {
                unreachable!()
            };
            instruction.operands[0].virtual_register =
                omega_terminal_selected_instructions::TerminalVirtualRegisterId(2);
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                    | Err(SelectedInstructionError::UseBeforeDefinition { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            corrupted.functions[0].blocks[1].instructions[0].kind =
                TerminalSelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(11),
                };
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            corrupted.functions[0].blocks[1].instructions[0]
                .provenance
                .values[0] = ValueId::new(8_001).unwrap();
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            let TerminalSelectedTerminator::ConditionalBranch { when_nonzero, .. } =
                &mut corrupted.functions[0].blocks[0].terminator
            else {
                unreachable!()
            };
            when_nonzero.psi_edge = EdgeId::new(8_002).unwrap();
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::SuccessorProjectionMismatch { .. })
            ));

            let mut corrupted = staged.selected().plan().clone();
            let TerminalSelectedTerminator::ConditionalBranch { when_zero, .. } =
                &mut corrupted.functions[0].blocks[0].terminator
            else {
                unreachable!()
            };
            when_zero.fuel[0].units += 1;
            assert!(matches!(
                validate_raw_selection(&staged, corrupted),
                Err(SelectedInstructionError::SuccessorProjectionMismatch { .. })
                    | Err(SelectedInstructionError::ProvenancePartitionMismatch { .. })
            ));
        }
    }

    #[test]
    fn selected_content_identity_binds_every_retained_field_class() {
        let staged = staged_conditional(NativeTarget::linux_x64());
        let original = staged.selected().plan();
        let identity = terminal_selected_instruction_plan_identity(original);
        let mut mutations = Vec::new();

        let mut changed = original.clone();
        changed.target = NativeTarget::windows_x64();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.entry = MachineId::new(8_009).unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].machine = MachineId::new(8_018).unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].attachment = Some(psi_core::StructuralTypeId::new(8_010).unwrap());
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0]
            .provenance
            .operations
            .push(OperationId::new(8_011).unwrap());
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0]
            .provenance
            .edges
            .push(EdgeId::new(8_019).unwrap());
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].entry_block.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[1].scalar_type = ScalarType::Boolean;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[1].id.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[1].class.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[1].origin =
            omega_terminal_selected_instructions::TerminalVirtualRegisterOrigin::InstructionResult {
                instruction: omega_terminal_selected_instructions::TerminalSelectedInstructionId(4),
                source_value: ValueId::new(8_012).unwrap(),
            };
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[1].definition_site = ValueDefinitionSite::Node {
            block: BlockId::new(8_013).unwrap(),
            node: 7,
        };
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].virtual_registers[0].entry_fixed_view = None;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].id.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].source_block = BlockId::new(8_020).unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions.clear();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0].id.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0].kind =
            TerminalSelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(12),
            };
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .constraint
            .variant += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0].operands[0].access =
            RegisterOperandAccess::Use;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0].operands[0].tied_to = Some(0);
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0].operands[0].early_clobber = true;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .implicit_uses
            .push(RegisterUnitId(999));
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[0]
            .implicit_defs
            .clear();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .clobbers
            .push(RegisterUnitId(998));
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .provenance
            .operations
            .push(OperationId::new(8_021).unwrap());
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .provenance
            .values
            .push(ValueId::new(8_022).unwrap());
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .provenance
            .edges
            .push(EdgeId::new(8_023).unwrap());
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .provenance
            .obligations
            .push(ObligationId::new(8_014).unwrap());
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .provenance
            .fuel[0]
            .units += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        let TerminalSelectedTerminator::ConditionalBranch { when_nonzero, .. } =
            &mut changed.functions[0].blocks[0].terminator
        else {
            unreachable!()
        };
        when_nonzero.bindings.push(TerminalValueBinding {
            parameter: ValueId::new(8_015).unwrap(),
            argument: ValueId::new(8_016).unwrap(),
            scalar_type: ScalarType::Boolean,
        });
        mutations.push(changed);
        let mut changed = original.clone();
        let TerminalSelectedTerminator::ConditionalBranch { when_nonzero, .. } =
            &mut changed.functions[0].blocks[0].terminator
        else {
            unreachable!()
        };
        when_nonzero.source_target = BlockId::new(8_024).unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        let TerminalSelectedTerminator::ConditionalBranch { when_zero, .. } =
            &mut changed.functions[0].blocks[0].terminator
        else {
            unreachable!()
        };
        when_zero.fuel[0].units += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        let TerminalSelectedTerminator::Return {
            psi_return_edge, ..
        } = &mut changed.functions[0].blocks[1].terminator
        else {
            unreachable!()
        };
        *psi_return_edge = EdgeId::new(8_017).unwrap();
        mutations.push(changed);

        for mutation in mutations {
            assert_ne!(
                terminal_selected_instruction_plan_identity(&mutation),
                identity
            );
        }
    }

    #[test]
    fn staged_selection_custody_rejects_detached_environment_and_selected_plan() {
        let x86 = staged_conditional(NativeTarget::linux_x64());
        let arm = staged_conditional(NativeTarget::linux_arm64());
        assert_eq!(
            validate_optimized_selection_custody(
                x86.optimized_target(),
                arm.register_environment(),
                x86.legalized(),
                x86.selected(),
            ),
            Err(OptimizedSelectionCustodyError::RegisterEnvironmentTargetMismatch)
        );
        assert_eq!(
            validate_optimized_selection_custody(
                x86.optimized_target(),
                x86.register_environment(),
                x86.legalized(),
                arm.selected(),
            ),
            Err(OptimizedSelectionCustodyError::RootMismatch)
        );

        let mut target = x86.optimized_target().target_operations().clone();
        let forged_operation = OperationId::new(8_030).unwrap();
        target.functions[0]
            .provenance
            .operations
            .push(forged_operation);
        assert_eq!(
            validate_terminal_legalized_operations(
                &target,
                x86.optimized_target().optimized().plan(),
                x86.optimized_target().optimized().unit(),
                x86.legalized().plan().clone(),
            ),
            Err(TerminalLegalizationError::SourceCustodyMismatch)
        );

        let mut unit = x86.optimized_target().optimized().unit().clone();
        unit.functions[0].blocks[0].nodes[0].effect.output += 1_000;
        unit.identity = omega_optimization_unit::recompute_psi_optimization_unit_identity(&unit);
        assert_eq!(
            validate_terminal_legalized_operations(
                x86.optimized_target().target_operations(),
                x86.optimized_target().optimized().plan(),
                &unit,
                x86.legalized().plan().clone(),
            ),
            Err(TerminalLegalizationError::SourceCustodyMismatch)
        );
    }

    #[test]
    fn physical_stage_receipts_retain_the_pre_physical_manifest_identity() {
        let selected = staged_conditional(NativeTarget::linux_x64());
        let manifest = selected
            .optimized_target()
            .optimized()
            .pre_physical_manifest()
            .record()
            .identity;
        assert_eq!(selected.custody().manifest(), manifest);

        let liveness = stage_optimized_liveness(selected).unwrap();
        assert_eq!(liveness.custody().manifest(), manifest);
        let ranges = stage_optimized_live_ranges(liveness).unwrap();
        assert_eq!(ranges.custody().manifest(), manifest);
        let legality = stage_optimized_allocation_legality(ranges).unwrap();
        assert_eq!(legality.custody().manifest(), manifest);
        let homes = stage_optimized_register_homes(legality).unwrap();
        assert_eq!(homes.custody().manifest(), manifest);
    }

    fn named_units(staged: &StagedOptimizedLiveness, names: &[&str]) -> Vec<RegisterUnitId> {
        names
            .iter()
            .flat_map(|name| {
                staged
                    .selected_stage()
                    .register_environment()
                    .physical()
                    .model()
                    .view_named(name)
                    .unwrap()
                    .units
                    .iter()
                    .copied()
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    #[test]
    fn disconnected_functions_reach_independent_allocator_and_machine_custody() {
        let expected_machines = [
            MachineId::new(16_001).unwrap(),
            MachineId::new(17_001).unwrap(),
        ];
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let (semantic, proof) = disconnected_conditional_artifact();
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
            )
            .unwrap();
            let target =
                omega_lowering_optimizer::lower_optimized_to_target_operations(optimized, target)
                    .unwrap();
            let selected = stage_optimized_instruction_selection(target).unwrap();

            assert_eq!(selected.selected().plan().functions.len(), 2);
            assert_eq!(
                selected
                    .selected()
                    .plan()
                    .functions
                    .iter()
                    .map(|function| function.machine)
                    .collect::<Vec<_>>(),
                expected_machines
            );
            for function in &selected.selected().plan().functions {
                assert_eq!(
                    function
                        .virtual_registers
                        .iter()
                        .map(|register| register.id.0)
                        .collect::<Vec<_>>(),
                    vec![0, 1, 2]
                );
                assert_eq!(
                    function
                        .blocks
                        .iter()
                        .map(|block| block.id.0)
                        .collect::<Vec<_>>(),
                    vec![0, 1, 2]
                );
            }

            let liveness = stage_optimized_liveness(selected).unwrap();
            assert_eq!(liveness.custody().function_count(), 2);
            assert_eq!(liveness.custody().block_count(), 6);
            assert_eq!(liveness.custody().virtual_register_count(), 6);
            assert_eq!(liveness.custody().instruction_count(), 12);
            assert_eq!(liveness.custody().successor_count(), 4);
            for (function, machine) in liveness
                .liveness()
                .plan()
                .functions
                .iter()
                .zip(expected_machines)
            {
                assert_eq!(function.machine, machine);
                assert_eq!(
                    function
                        .blocks
                        .iter()
                        .flat_map(|block| &block.instructions)
                        .map(|instruction| instruction.position.0)
                        .collect::<Vec<_>>(),
                    (0..6).collect::<Vec<_>>()
                );
            }
            let mut corrupted_liveness = liveness.liveness().plan().clone();
            corrupted_liveness.functions[1].machine = expected_machines[0];
            assert_eq!(
                validate_terminal_liveness(
                    liveness.selected_stage().selected(),
                    corrupted_liveness,
                ),
                Err(TerminalLivenessError::FunctionMismatch { function: 1 })
            );

            let ranges = stage_optimized_live_ranges(liveness).unwrap();
            assert_eq!(ranges.custody().function_count(), 2);
            assert_eq!(ranges.custody().block_count(), 6);
            assert_eq!(ranges.custody().virtual_register_count(), 6);
            assert_eq!(ranges.custody().interference_count(), 0);
            for (function, machine) in ranges
                .ranges()
                .plan()
                .functions
                .iter()
                .zip(expected_machines)
            {
                assert_eq!(function.machine, machine);
                assert_eq!(
                    function
                        .block_domains
                        .iter()
                        .map(|domain| (domain.block.0, domain.start.0, domain.end.0))
                        .collect::<Vec<_>>(),
                    vec![(0, 0, 4), (1, 4, 8), (2, 8, 12)]
                );
                assert!(function.interference.is_empty());
            }
            let mut corrupted_ranges = ranges.ranges().plan().clone();
            corrupted_ranges.functions[1].machine = expected_machines[0];
            assert!(
                validate_terminal_live_ranges(
                    ranges.liveness_stage().selected_stage().selected(),
                    ranges.liveness_stage().liveness(),
                    corrupted_ranges,
                )
                .is_err()
            );

            let legality = stage_optimized_allocation_legality(ranges).unwrap();
            assert_eq!(legality.custody().function_count(), 2);
            assert_eq!(legality.custody().virtual_register_count(), 6);
            let range_stage = legality.live_range_stage();
            let environment = range_stage
                .liveness_stage()
                .selected_stage()
                .register_environment();
            let mut corrupted_legality = legality.legality().plan().clone();
            corrupted_legality.functions[1].machine = expected_machines[0];
            assert!(
                validate_terminal_allocation_legality(
                    range_stage.ranges(),
                    legality.allocator_availability(),
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    corrupted_legality,
                )
                .is_err()
            );

            let homes = stage_optimized_register_homes(legality).unwrap();
            assert_eq!(homes.custody().function_count(), 2);
            assert_eq!(homes.custody().assignment_count(), 6);
            assert_eq!(
                homes
                    .homes()
                    .plan()
                    .functions
                    .iter()
                    .map(|function| function.machine)
                    .collect::<Vec<_>>(),
                expected_machines
            );
            assert_eq!(
                homes.homes().plan().functions[0].assignments,
                homes.homes().plan().functions[1].assignments
            );
            let legality_stage = homes.legality_stage();
            let range_stage = legality_stage.live_range_stage();
            let environment = range_stage
                .liveness_stage()
                .selected_stage()
                .register_environment();
            let mut corrupted_homes = homes.homes().plan().clone();
            corrupted_homes.functions[1].machine = expected_machines[0];
            assert!(
                validate_terminal_register_homes(
                    legality_stage.legality(),
                    range_stage.ranges(),
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    corrupted_homes,
                )
                .is_err()
            );

            let post = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
            assert_eq!(post.custody().instruction_count(), 12);
            assert_eq!(post.machine().plan().functions.len(), 2);
            assert_eq!(
                post.machine()
                    .plan()
                    .functions
                    .iter()
                    .map(|function| function.machine)
                    .collect::<Vec<_>>(),
                expected_machines
            );
            let mut corrupted_post = post.machine().plan().clone();
            corrupted_post.functions[1].machine = expected_machines[0];
            let legality_stage = homes.legality_stage();
            let range_stage = legality_stage.live_range_stage();
            let selected_stage = range_stage.liveness_stage().selected_stage();
            let environment = selected_stage.register_environment();
            assert!(
                omega_machine_optimizer::validate_terminal_post_allocation_machine_plan(
                    selected_stage.selected(),
                    post.effects().effects(),
                    range_stage.ranges(),
                    legality_stage.legality(),
                    homes.homes(),
                    homes.post_allocation_manifest(),
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    corrupted_post,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn selected_liveness_is_exact_on_both_architectures() {
        for (target, before_compare, after_compare, after_branch) in [
            (
                NativeTarget::linux_x64(),
                vec!["rip", "rsp"],
                vec!["rflags", "rip", "rsp"],
                vec!["rsp"],
            ),
            (
                NativeTarget::linux_arm64(),
                vec!["pc", "sp", "x30"],
                vec!["nzcv", "pc", "sp", "x30"],
                vec!["sp", "x30"],
            ),
        ] {
            let staged = stage_optimized_liveness(staged_conditional(target)).unwrap();
            let plan = staged.liveness().plan();
            let function = &plan.functions[0];
            assert_eq!(function.entry_definitions.len(), 1);
            assert_eq!(
                function.entry_definitions[0].virtual_register,
                TerminalVirtualRegisterId(0)
            );
            assert!(function.entry_definitions[0].fixed_view.is_some());
            assert_eq!(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .map(|instruction| instruction.position.0)
                    .collect::<Vec<_>>(),
                (0..6).collect::<Vec<_>>()
            );

            let entry = &function.blocks[0];
            assert_eq!(entry.virtual_live_in, vec![TerminalVirtualRegisterId(0)]);
            assert!(entry.virtual_live_out.is_empty());
            assert_eq!(
                entry.instructions[0].virtual_uses,
                vec![TerminalVirtualRegisterId(0)]
            );
            assert!(entry.instructions[0].virtual_defs.is_empty());
            assert_eq!(
                entry.instructions[0].unit_live_in,
                named_units(&staged, &before_compare)
            );
            assert_eq!(
                entry.instructions[0].unit_live_out,
                named_units(&staged, &after_compare)
            );
            assert_eq!(
                entry.instructions[1].unit_live_in,
                entry.instructions[0].unit_live_out
            );
            assert_eq!(
                entry.instructions[1].unit_live_out,
                named_units(&staged, &after_branch)
            );
            assert_eq!(entry.successors.len(), 2);
            assert_eq!(entry.successors[0].polarity_ordinal, 0);
            assert_eq!(entry.successors[1].polarity_ordinal, 1);
            for successor in &entry.successors {
                let target_block = &function.blocks[successor.target.0 as usize];
                assert_eq!(successor.virtual_live, target_block.virtual_live_in);
                assert_eq!(successor.unit_live, target_block.unit_live_in);
            }

            for (block, register) in function.blocks[1..]
                .iter()
                .zip([TerminalVirtualRegisterId(1), TerminalVirtualRegisterId(2)])
            {
                assert!(block.virtual_live_in.is_empty());
                assert!(block.virtual_live_out.is_empty());
                assert_eq!(block.instructions[0].virtual_defs, vec![register]);
                assert_eq!(block.instructions[0].virtual_live_out, vec![register]);
                assert_eq!(block.instructions[1].virtual_uses, vec![register]);
                assert_eq!(block.instructions[1].virtual_live_in, vec![register]);
                assert!(block.instructions[1].virtual_live_out.is_empty());
            }
            assert_eq!(staged.custody().function_count(), 1);
            assert_eq!(staged.custody().block_count(), 3);
            assert_eq!(staged.custody().virtual_register_count(), 3);
            assert_eq!(staged.custody().instruction_count(), 6);
            assert_eq!(staged.custody().successor_count(), 2);
            assert_eq!(
                staged.custody().register_environment(),
                staged.selected_stage().register_environment().identity()
            );
            assert_eq!(
                staged.custody().liveness(),
                staged.liveness().receipt().identity()
            );
            assert_eq!(
                staged.custody().selected(),
                staged.selected_stage().selected().receipt().identity()
            );
        }
    }

    #[test]
    fn forwarded_parameter_conditional_retains_cross_edge_liveness() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let selected = staged_forwarded_conditional(target);
            assert_eq!(
                selected.legalized().plan().functions[0].recipe,
                TerminalLegalizationRecipe::ReturnU64EntryParameterConditionalV1
            );
            let selected_plan = selected.selected().plan();
            assert_eq!(selected_plan.functions[0].virtual_registers.len(), 2);
            assert_eq!(
                selected_plan.functions[0]
                    .blocks
                    .iter()
                    .map(|block| block.instructions.len() + 1)
                    .sum::<usize>(),
                4
            );
            assert!(
                selected_plan.functions[0].virtual_registers[0]
                    .entry_fixed_view
                    .is_some()
            );
            assert!(
                selected_plan.functions[0].virtual_registers[1]
                    .entry_fixed_view
                    .is_some()
            );

            let staged = stage_optimized_liveness(selected).unwrap();
            let function = &staged.liveness().plan().functions[0];
            assert_eq!(
                function.blocks[0].virtual_live_in,
                vec![TerminalVirtualRegisterId(0), TerminalVirtualRegisterId(1)]
            );
            assert_eq!(
                function.blocks[0].virtual_live_out,
                vec![TerminalVirtualRegisterId(1)]
            );
            for successor in &function.blocks[0].successors {
                assert_eq!(successor.virtual_live, vec![TerminalVirtualRegisterId(1)]);
            }
            for block in &function.blocks[1..] {
                assert_eq!(block.virtual_live_in, vec![TerminalVirtualRegisterId(1)]);
                assert!(block.virtual_live_out.is_empty());
                assert_eq!(
                    block.instructions[0].virtual_uses,
                    vec![TerminalVirtualRegisterId(1)]
                );
                assert!(block.instructions[0].virtual_live_out.is_empty());
                assert!(block.instructions[0].unit_live_out.is_empty());
            }
        }
    }

    #[test]
    fn forwarded_parameter_selection_rejects_fixed_input_and_path_corruption() {
        let staged = staged_forwarded_conditional(NativeTarget::linux_x64());
        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].virtual_registers[1].entry_fixed_view = None;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::VirtualRegisterProjectionMismatch { .. })
        ));

        let mut corrupted = staged.selected().plan().clone();
        let TerminalSelectedTerminator::Return { instruction, .. } =
            &mut corrupted.functions[0].blocks[1].terminator
        else {
            unreachable!()
        };
        instruction.operands[0].virtual_register = TerminalVirtualRegisterId(0);
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
        ));
    }

    #[test]
    fn live_ranges_are_block_local_and_interference_is_cfg_exact() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let staged = stage_optimized_live_ranges(
                stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
            )
            .unwrap();
            let function = &staged.ranges().plan().functions[0];
            assert_eq!(
                function
                    .block_domains
                    .iter()
                    .map(|domain| (domain.block.0, domain.start.0, domain.end.0))
                    .collect::<Vec<_>>(),
                vec![(0, 0, 4), (1, 4, 6), (2, 6, 8)]
            );
            assert_eq!(function.virtual_registers.len(), 2);
            assert_eq!(
                function.virtual_registers[0].fragments,
                vec![TerminalLiveRangeFragment {
                    block: omega_terminal_selected_instructions::TerminalSelectedBlockId(0),
                    start: TerminalLiveRangePoint(0),
                    end: TerminalLiveRangePoint(1),
                }]
            );
            assert_eq!(
                function.virtual_registers[1]
                    .fragments
                    .iter()
                    .map(|fragment| (fragment.block.0, fragment.start.0, fragment.end.0))
                    .collect::<Vec<_>>(),
                vec![(0, 0, 4), (1, 4, 5), (2, 6, 7)]
            );
            assert_eq!(
                function.virtual_registers[1]
                    .edge_connectors
                    .iter()
                    .map(|edge| (edge.polarity_ordinal, edge.psi_edge, edge.target.0))
                    .collect::<Vec<_>>(),
                vec![
                    (0, EdgeId::new(4_011).unwrap(), 1),
                    (1, EdgeId::new(4_012).unwrap(), 2),
                ]
            );
            assert_eq!(
                function.interference,
                vec![TerminalVirtualInterference {
                    lower: TerminalVirtualRegisterId(0),
                    higher: TerminalVirtualRegisterId(1),
                }]
            );
            assert_eq!(function.virtual_registers[0].fixed_constraints.len(), 1);
            assert!(matches!(
                function.virtual_registers[0].fixed_constraints[0].site,
                TerminalVirtualFixedConstraintSite::Entry
            ));
            assert_eq!(function.virtual_registers[1].fixed_constraints.len(), 3);
            assert!(matches!(
                function.virtual_registers[1].fixed_constraints[0].site,
                TerminalVirtualFixedConstraintSite::Entry
            ));
            assert!(
                function.virtual_registers[1].fixed_constraints[1..]
                    .iter()
                    .all(|constraint| matches!(
                        constraint.site,
                        TerminalVirtualFixedConstraintSite::Operand { .. }
                    ))
            );
            assert_eq!(staged.custody().interference_count(), 1);
            assert_eq!(
                staged.custody().register_environment(),
                staged
                    .liveness_stage()
                    .selected_stage()
                    .register_environment()
                    .identity()
            );
            assert_eq!(
                staged.custody().ranges(),
                staged.ranges().receipt().identity()
            );
            assert_eq!(
                staged.custody().liveness(),
                staged.liveness_stage().liveness().receipt().identity()
            );

            let repeated = stage_optimized_live_ranges(
                stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
            )
            .unwrap();
            assert_eq!(staged.ranges(), repeated.ranges());
            assert_eq!(staged.custody(), repeated.custody());
        }

        let constant = stage_optimized_live_ranges(
            stage_optimized_liveness(staged_conditional(NativeTarget::linux_x64())).unwrap(),
        )
        .unwrap();
        let function = &constant.ranges().plan().functions[0];
        assert_eq!(
            function
                .block_domains
                .iter()
                .map(|domain| (domain.block.0, domain.start.0, domain.end.0))
                .collect::<Vec<_>>(),
            vec![(0, 0, 4), (1, 4, 8), (2, 8, 12)]
        );
        assert_eq!(
            function
                .virtual_registers
                .iter()
                .flat_map(|range| &range.fragments)
                .map(|fragment| (fragment.block.0, fragment.start.0, fragment.end.0))
                .collect::<Vec<_>>(),
            vec![(0, 0, 1), (1, 5, 7), (2, 9, 11)]
        );
        assert!(function.interference.is_empty());
        assert!(
            function
                .virtual_registers
                .iter()
                .all(|range| range.edge_connectors.is_empty())
        );
    }

    #[test]
    fn allocation_legality_is_phase_exact_and_exposes_fixed_view_transitions() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let staged = stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            let function = &staged.legality().plan().functions[0];
            assert_eq!(function.virtual_registers.len(), 2);
            assert_eq!(function.virtual_registers[0].entry_transitions.len(), 0);
            assert_eq!(function.virtual_registers[1].entry_transitions.len(), 2);
            assert!(
                function.virtual_registers[1]
                    .entry_transitions
                    .iter()
                    .all(|transition| transition.from_view != transition.to_view)
            );

            let environment = staged
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment();
            let model = environment.physical().model();
            let range_function = &staged.live_range_stage().ranges().plan().functions[0];
            for register in &function.virtual_registers {
                for point in &register.points {
                    assert!(!point.candidates.is_empty());
                    for candidate in &point.candidates {
                        let view = &model.views[usize::from(candidate.0)];
                        assert_eq!(view.class, register.class);
                        assert!(view.allocatable);
                        assert!(view.units.iter().chain(&view.write_units).all(|unit| {
                            environment
                                .reservations()
                                .reserved_units()
                                .binary_search(unit)
                                .is_err()
                        }));
                        assert!(view.units.iter().chain(&view.write_units).all(|unit| {
                            range_function
                                .architectural_units
                                .iter()
                                .find(|row| row.unit == *unit)
                                .is_none_or(|row| {
                                    !row.fragments.iter().any(|fragment| {
                                        fragment.block == point.block
                                            && fragment.start <= point.point
                                            && point.point < fragment.end
                                    }) && !row.actions.iter().any(|action| {
                                        action.block == point.block && action.point == point.point
                                    })
                                })
                        }));
                    }
                }
            }
            assert_eq!(
                staged.custody().register_environment(),
                environment.identity()
            );
            assert_eq!(
                staged.custody().legality(),
                staged.legality().receipt().identity()
            );
            assert_eq!(staged.custody().entry_transition_count(), 2);

            if target == NativeTarget::linux_x64() {
                let mut overlays = environment.reservations().profile().active_overlays.clone();
                overlays.retain(|name| name != "omega.x86.metering");
                let reduced = validate_register_reservation_profile(
                    RegisterReservationProfile {
                        name: "test.no-metering".into(),
                        active_overlays: overlays,
                    },
                    target,
                    environment.physical(),
                )
                .unwrap();
                let reduced_identity = target_register_environment_identity(
                    target,
                    environment.physical(),
                    environment.constraints(),
                    &reduced,
                    environment.allocation_constraint_keys(),
                );
                let reduced_availability = materialize_terminal_allocator_availability(
                    reduced_identity,
                    target,
                    environment.physical(),
                    environment.constraints(),
                    &reduced,
                    environment.allocation_constraint_keys(),
                    TerminalAllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1,
                )
                .unwrap();
                let reduced_legality = omega_regalloc::analyze_terminal_allocation_legality(
                    staged.live_range_stage().ranges(),
                    &reduced_availability,
                    reduced_identity,
                    environment.physical(),
                    environment.constraints(),
                    &reduced,
                    environment.allocation_constraint_keys(),
                )
                .unwrap();
                let r15 = model.view_named("r15").unwrap().id;
                assert!(
                    function
                        .virtual_registers
                        .iter()
                        .flat_map(|register| &register.points)
                        .all(|point| !point.candidates.contains(&r15))
                );
                assert!(
                    reduced_legality.plan().functions[0]
                        .virtual_registers
                        .iter()
                        .flat_map(|register| &register.points)
                        .any(|point| point.candidates.contains(&r15))
                );
                assert_ne!(
                    reduced_legality.receipt().identity(),
                    staged.legality().receipt().identity()
                );
            }

            let repeated = stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(staged.legality(), repeated.legality());
            assert_eq!(staged.custody(), repeated.custody());

            let mut corrupted = staged.legality().plan().clone();
            corrupted.functions[0].virtual_registers[0].points[0]
                .candidates
                .clear();
            assert_ne!(
                terminal_allocation_legality_identity(&corrupted),
                staged.legality().receipt().identity()
            );
            let ranges = staged.live_range_stage();
            assert!(matches!(
                validate_terminal_allocation_legality(
                    ranges.ranges(),
                    staged.allocator_availability(),
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    corrupted,
                ),
                Err(TerminalAllocationLegalityError::VirtualRegisterMismatch { .. })
            ));
        }

        let constant = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_conditional(NativeTarget::linux_x64())).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(constant.custody().entry_transition_count(), 0);
    }

    #[test]
    fn transition_free_register_homes_are_deterministic_and_cfg_exact() {
        for (target, condition_view, result_view) in [
            (NativeTarget::linux_x64(), "rdi", "rax"),
            (NativeTarget::linux_arm64(), "x0", "x0"),
        ] {
            let staged = stage_optimized_register_homes(
                stage_optimized_allocation_legality(
                    stage_optimized_live_ranges(
                        stage_optimized_liveness(staged_conditional(target)).unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            let function = &staged.homes().plan().functions[0];
            assert_eq!(function.assignments.len(), 3);
            let environment = staged
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment();
            let encoded = staged.homes().plan().encode();
            let decoded = TerminalRegisterHomePlan::decode(&encoded).unwrap();
            assert_eq!(&decoded, staged.homes().plan());
            let legality = staged.legality_stage();
            let ranges = legality.live_range_stage();
            let replay = validate_terminal_register_homes(
                legality.legality(),
                ranges.ranges(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                decoded,
            )
            .unwrap();
            assert_eq!(replay, *staged.homes());
            let manifest = staged.post_allocation_manifest().record();
            assert_eq!(manifest.identity, manifest.recomputed_identity());
            assert_eq!(
                PostAllocationOptimizationManifest::decode(&manifest.encode()),
                Ok(manifest.clone())
            );
            assert_eq!(manifest.pre_physical, staged.custody().manifest());
            assert_eq!(manifest.target, target);
            assert!(manifest.selected_transformations.is_empty());
            assert_eq!(manifest.homes, staged.homes().receipt().identity());
            assert_eq!(manifest.statistics.functions, 1);
            assert_eq!(manifest.statistics.assignments, 3);
            assert_eq!(manifest.statistics.fixed_view_transitions, 0);
            assert_eq!(
                staged.custody().post_allocation_manifest(),
                manifest.identity
            );
            assert_eq!(
                validate_optimized_register_home_custody(
                    legality,
                    staged.homes(),
                    staged.post_allocation_manifest(),
                )
                .unwrap(),
                staged.custody()
            );
            assert!(manifest.render_text().contains("frame: unavailable"));
            assert_eq!(
                validate_post_allocation_optimization_manifest(
                    manifest,
                    staged.custody().manifest(),
                    &[],
                    ranges.ranges(),
                    legality.legality(),
                    staged.homes(),
                )
                .unwrap(),
                *staged.post_allocation_manifest()
            );
            let mut corrupted = manifest.clone();
            corrupted.statistics.assignments += 1;
            assert_eq!(
                validate_post_allocation_optimization_manifest(
                    &corrupted,
                    staged.custody().manifest(),
                    &[],
                    ranges.ranges(),
                    legality.legality(),
                    staged.homes(),
                ),
                Err(PostAllocationOptimizationManifestError::IdentityMismatch)
            );
            corrupted.identity = corrupted.recomputed_identity();
            assert_eq!(
                validate_post_allocation_optimization_manifest(
                    &corrupted,
                    staged.custody().manifest(),
                    &[],
                    ranges.ranges(),
                    legality.legality(),
                    staged.homes(),
                ),
                Err(PostAllocationOptimizationManifestError::ContentMismatch)
            );
            let model = environment.physical().model();
            assert_eq!(
                function.assignments[0].view,
                model.view_named(condition_view).unwrap().id
            );
            assert_eq!(
                function.assignments[1].view,
                model.view_named(result_view).unwrap().id
            );
            assert_eq!(function.assignments[1].view, function.assignments[2].view);
            assert!(
                staged
                    .legality_stage()
                    .live_range_stage()
                    .ranges()
                    .plan()
                    .functions[0]
                    .interference
                    .is_empty()
            );
            for assignment in &function.assignments {
                let view = &model.views[usize::from(assignment.view.0)];
                assert_eq!(view.class, assignment.class);
                assert!(view.units.iter().chain(&view.write_units).all(|unit| {
                    environment
                        .reservations()
                        .reserved_units()
                        .binary_search(unit)
                        .is_err()
                }));
            }
            assert_eq!(staged.custody().assignment_count(), 3);
            assert_eq!(
                staged.custody().homes(),
                staged.homes().receipt().identity()
            );
            assert_eq!(
                staged.custody().register_environment(),
                environment.identity()
            );

            let repeated = stage_optimized_register_homes(
                stage_optimized_allocation_legality(
                    stage_optimized_live_ranges(
                        stage_optimized_liveness(staged_conditional(target)).unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(staged.homes(), repeated.homes());
            assert_eq!(staged.custody(), repeated.custody());

            let mut corrupted = staged.homes().plan().clone();
            let original_view = corrupted.functions[0].assignments[0].view;
            corrupted.functions[0].assignments[0].view = model
                .views
                .iter()
                .find(|view| {
                    view.class == corrupted.functions[0].assignments[0].class
                        && view.id != original_view
                })
                .expect("fixture register class has a distinct corruption view")
                .id;
            assert_ne!(
                terminal_register_home_identity(&corrupted),
                staged.homes().receipt().identity()
            );
            let legality = staged.legality_stage();
            let ranges = legality.live_range_stage();
            assert!(matches!(
                validate_terminal_register_homes(
                    legality.legality(),
                    ranges.ranges(),
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    corrupted,
                ),
                Err(TerminalRegisterHomeError::VirtualRegisterMismatch { .. })
                    | Err(TerminalRegisterHomeError::UnknownOrIncompatibleView { .. })
            ));
        }

        let forwarded = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_forwarded_conditional(NativeTarget::linux_x64()))
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            stage_optimized_register_homes(forwarded),
            Err(OptimizedRegisterHomeCustodyError::Assignment(
                TerminalRegisterHomeError::UnresolvedEntryTransitions { count: 2, .. }
            ))
        ));
    }

    #[test]
    fn fixed_view_copies_are_explicit_reanalyzed_and_deterministic() {
        for (target, entry_name, result_name) in [
            (NativeTarget::linux_x64(), "rsi", "rax"),
            (NativeTarget::linux_arm64(), "x1", "x0"),
        ] {
            let source = stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            let source_selected = source
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .selected()
                .plan()
                .clone();
            let source_manifest = source.custody().manifest();
            let materialized = stage_optimized_fixed_view_copies(
                source,
                TerminalFixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
                budget(),
            )
            .unwrap();
            let machine_effects =
                stage_optimized_machine_effects_after_fixed_view_copies(&materialized).unwrap();
            assert_eq!(
                machine_effects.effects().receipt().selected(),
                materialized.custody().transformed_selected()
            );
            assert_eq!(
                machine_effects.custody().source(),
                &StagedOptimizedMachineEffectSourceCustodyReceipt::FixedViewCopies(
                    materialized.custody()
                )
            );
            assert_eq!(
                &validate_optimized_machine_effect_custody_after_fixed_view_copies(
                    &materialized,
                    machine_effects.effects(),
                )
                .unwrap(),
                machine_effects.custody()
            );
            let copy_plan = materialized.copies().plan();
            assert_eq!(copy_plan.copies.len(), 2);
            assert_eq!(materialized.custody().copy_count(), 2);
            assert_eq!(materialized.custody().manifest(), source_manifest);
            assert_eq!(
                copy_plan.usage,
                omega_optimization_core::OptimizationWorkUsage {
                    rule_evaluations: 1,
                    candidates: 2,
                    validation_steps: 2,
                    commits: 2,
                    iterations: 1,
                }
            );
            assert_ne!(
                materialized.custody().source_selected(),
                materialized.custody().transformed_selected()
            );
            assert_eq!(
                terminal_fixed_view_copy_identity(copy_plan),
                materialized.custody().transformation()
            );
            let transformed = &copy_plan.transformed;
            assert_eq!(transformed.functions[0].virtual_registers.len(), 4);
            let environment = materialized
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage()
                .register_environment();
            let entry_view = environment
                .physical()
                .model()
                .view_named(entry_name)
                .unwrap()
                .id;
            let result_view = environment
                .physical()
                .model()
                .view_named(result_name)
                .unwrap()
                .id;
            for (index, copy) in copy_plan.copies.iter().enumerate() {
                assert_eq!(copy.source_virtual_register, TerminalVirtualRegisterId(1));
                assert_eq!(copy.result_virtual_register.0, 2 + index as u32);
                assert_eq!(copy.copy_instruction.0, 4 + index as u32);
                assert_eq!(copy.from_view, entry_view);
                assert_eq!(copy.to_view, result_view);
                assert_eq!(copy.copy_constraint, environment.selected_keys().copy_i64);
                let block = &transformed.functions[0].blocks[index + 1];
                let instruction = block.instructions.last().unwrap();
                assert_eq!(instruction.id, copy.copy_instruction);
                assert_eq!(instruction.kind, TerminalSelectedInstructionKind::CopyI64);
                assert_eq!(
                    instruction.operands[0].virtual_register,
                    copy.source_virtual_register
                );
                assert_eq!(
                    instruction.operands[1].virtual_register,
                    copy.result_virtual_register
                );
                assert!(instruction.provenance.operations.is_empty());
                assert_eq!(instruction.provenance.values, vec![copy.source_value]);
                assert!(instruction.provenance.edges.is_empty());
                assert!(instruction.provenance.obligations.is_empty());
                assert!(instruction.provenance.fuel.is_empty());
                let TerminalSelectedTerminator::Return {
                    instruction: source_return,
                    ..
                } = &source_selected.functions[0].blocks[index + 1].terminator
                else {
                    unreachable!()
                };
                let TerminalSelectedTerminator::Return {
                    instruction: transformed_return,
                    ..
                } = &block.terminator
                else {
                    unreachable!()
                };
                assert_eq!(source_return.id, transformed_return.id);
                assert_eq!(source_return.provenance, transformed_return.provenance);
                assert_eq!(
                    transformed_return.operands[0].virtual_register,
                    copy.result_virtual_register
                );
            }

            let mut corrupted = materialized.copies().plan().clone();
            corrupted.copies[0].from_view = result_view;
            assert!(matches!(
                validate_terminal_fixed_view_copies(
                    materialized
                        .source_legality_stage()
                        .live_range_stage()
                        .liveness_stage()
                        .selected_stage()
                        .selected(),
                    materialized
                        .source_legality_stage()
                        .live_range_stage()
                        .ranges(),
                    materialized.source_legality_stage().legality(),
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    corrupted,
                ),
                Err(TerminalFixedViewCopyError::CopyMismatch { index: 0 })
            ));
            let mut corrupted = materialized.copies().plan().clone();
            corrupted.transformed.functions[0].blocks[1].instructions[0]
                .provenance
                .values
                .clear();
            assert!(matches!(
                validate_terminal_fixed_view_copies(
                    materialized
                        .source_legality_stage()
                        .live_range_stage()
                        .liveness_stage()
                        .selected_stage()
                        .selected(),
                    materialized
                        .source_legality_stage()
                        .live_range_stage()
                        .ranges(),
                    materialized.source_legality_stage().legality(),
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    corrupted,
                ),
                Err(TerminalFixedViewCopyError::TransformedPlanMismatch)
            ));
            let mut corrupted = materialized.copies().plan().clone();
            corrupted.usage.commits += 1;
            assert!(matches!(
                validate_terminal_fixed_view_copies(
                    materialized
                        .source_legality_stage()
                        .live_range_stage()
                        .liveness_stage()
                        .selected_stage()
                        .selected(),
                    materialized
                        .source_legality_stage()
                        .live_range_stage()
                        .ranges(),
                    materialized.source_legality_stage().legality(),
                    environment.identity(),
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    environment.allocation_constraint_keys(),
                    corrupted,
                ),
                Err(TerminalFixedViewCopyError::ReceiptMismatch)
            ));
            assert!(matches!(
                validate_terminal_live_ranges(
                    materialized.copies(),
                    materialized
                        .source_legality_stage()
                        .live_range_stage()
                        .liveness_stage()
                        .liveness(),
                    materialized
                        .source_legality_stage()
                        .live_range_stage()
                        .ranges()
                        .plan()
                        .clone(),
                ),
                Err(TerminalLiveRangeError::LivenessRevalidation(
                    TerminalLivenessError::RootMismatch
                ))
            ));

            let reanalyzed = stage_optimized_selected_reanalysis(materialized).unwrap();
            assert_eq!(reanalyzed.custody().entry_transition_count(), 0);
            assert_eq!(reanalyzed.legality().receipt().entry_transition_count(), 0);
            let homes = stage_optimized_register_homes_after_fixed_view_copies(reanalyzed).unwrap();
            let post = stage_optimized_post_allocation_machine_plan_after_fixed_view_copies(&homes)
                .unwrap();
            assert_eq!(
                post.machine().receipt().selected(),
                homes.reanalysis_stage().ranges().receipt().selected()
            );
            assert_eq!(
                &validate_optimized_post_allocation_machine_plan_after_fixed_view_copy_custody(
                    &homes, &post,
                )
                .unwrap(),
                post.custody()
            );
            let assignments = &homes.homes().plan().functions[0].assignments;
            assert_eq!(assignments.len(), 4);
            assert_eq!(assignments[1].view, entry_view);
            assert_ne!(assignments[0].view, assignments[1].view);
            assert_eq!(assignments[2].view, result_view);
            assert_eq!(assignments[3].view, result_view);
            assert_eq!(
                homes.reanalysis_stage().ranges().plan().functions[0].interference,
                vec![TerminalVirtualInterference {
                    lower: TerminalVirtualRegisterId(0),
                    higher: TerminalVirtualRegisterId(1),
                }]
            );
            assert_eq!(homes.custody().assignment_count(), 4);
            let manifest = homes.post_allocation_manifest().record();
            assert_eq!(manifest.identity, manifest.recomputed_identity());
            assert_eq!(
                PostAllocationOptimizationManifest::decode(&manifest.encode()),
                Ok(manifest.clone())
            );
            assert_eq!(
                manifest.selected_transformations,
                vec![PostAllocationSelectedTransformation::FixedViewCopy(
                    homes.custody().source().source().transformation()
                )]
            );
            assert_eq!(
                manifest.selected,
                homes.reanalysis_stage().ranges().plan().selected
            );
            assert_eq!(manifest.statistics.assignments, 4);
            assert_eq!(manifest.statistics.virtual_interferences, 1);
            let transformation = PostAllocationSelectedTransformation::FixedViewCopy(
                homes.custody().source().source().transformation(),
            );
            assert_eq!(
                validate_post_allocation_optimization_manifest(
                    manifest,
                    homes.custody().source().source().manifest(),
                    &[transformation, transformation],
                    homes.reanalysis_stage().ranges(),
                    homes.reanalysis_stage().legality(),
                    homes.homes(),
                ),
                Err(PostAllocationOptimizationManifestError::NonCanonicalTransformationLedger)
            );
            assert_eq!(
                homes.custody().post_allocation_manifest(),
                manifest.identity
            );
            assert_eq!(
                validate_optimized_register_home_after_fixed_view_copy_custody(
                    homes.reanalysis_stage(),
                    homes.homes(),
                    homes.post_allocation_manifest(),
                )
                .unwrap(),
                homes.custody()
            );

            let repeated = stage_optimized_register_homes_after_fixed_view_copies(
                stage_optimized_selected_reanalysis(
                    stage_optimized_fixed_view_copies(
                        stage_optimized_allocation_legality(
                            stage_optimized_live_ranges(
                                stage_optimized_liveness(staged_forwarded_conditional(target))
                                    .unwrap(),
                            )
                            .unwrap(),
                        )
                        .unwrap(),
                        TerminalFixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
                        budget(),
                    )
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(homes.homes(), repeated.homes());
            assert_eq!(homes.custody(), repeated.custody());
        }

        let constrained = OptimizationWorkBudget::new(128, 128, 128, 1, 16).unwrap();
        let source = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_forwarded_conditional(NativeTarget::linux_x64()))
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            stage_optimized_fixed_view_copies(
                source,
                TerminalFixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
                constrained,
            ),
            Err(OptimizedFixedViewCopyCustodyError::Materialization(
                TerminalFixedViewCopyError::BudgetExceeded { .. }
            ))
        ));

        let constant = stage_optimized_fixed_view_copies(
            stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged_conditional(NativeTarget::linux_x64()))
                        .unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
            TerminalFixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
            budget(),
        )
        .unwrap();
        assert!(constant.copies().plan().copies.is_empty());
        assert_eq!(
            constant.copies().plan().source_selected,
            constant.copies().receipt().transformed_selected()
        );
    }

    #[test]
    fn architectural_actions_do_not_inflate_semantic_unit_fragments() {
        for (target, instruction_pointer) in [
            (NativeTarget::linux_x64(), "rip"),
            (NativeTarget::linux_arm64(), "pc"),
        ] {
            let staged = stage_optimized_live_ranges(
                stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
            )
            .unwrap();
            let unit = named_units(staged.liveness_stage(), &[instruction_pointer])[0];
            let range = staged.ranges().plan().functions[0]
                .architectural_units
                .iter()
                .find(|range| range.unit == unit)
                .unwrap();
            assert_eq!(
                range
                    .fragments
                    .iter()
                    .map(|fragment| (fragment.block.0, fragment.start.0, fragment.end.0))
                    .collect::<Vec<_>>(),
                vec![(0, 0, 3)]
            );
            assert!(range.actions.iter().any(|action| {
                action.point == TerminalLiveRangePoint(3)
                    && action.kind == TerminalArchitecturalUnitActionKind::Def
            }));
        }
    }

    #[test]
    fn independent_live_range_validation_rejects_corruption_and_detachment() {
        let staged =
            stage_optimized_liveness(staged_forwarded_conditional(NativeTarget::linux_x64()))
                .unwrap();
        let valid =
            analyze_terminal_live_ranges(staged.selected_stage().selected(), staged.liveness())
                .unwrap();
        let identity = terminal_live_range_identity(valid.plan());

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].virtual_registers[1].fragments[0]
            .end
            .0 -= 1;
        assert!(matches!(
            validate_terminal_live_ranges(
                staged.selected_stage().selected(),
                staged.liveness(),
                corrupted.clone(),
            ),
            Err(TerminalLiveRangeError::VirtualRegisterMismatch { .. })
        ));
        assert_ne!(terminal_live_range_identity(&corrupted), identity);

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].virtual_registers[1].edge_connectors[0].polarity_ordinal = 1;
        assert!(matches!(
            validate_terminal_live_ranges(
                staged.selected_stage().selected(),
                staged.liveness(),
                corrupted,
            ),
            Err(TerminalLiveRangeError::NonCanonicalRows { .. })
                | Err(TerminalLiveRangeError::VirtualRegisterMismatch { .. })
        ));

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].interference.clear();
        assert!(matches!(
            validate_terminal_live_ranges(
                staged.selected_stage().selected(),
                staged.liveness(),
                corrupted,
            ),
            Err(TerminalLiveRangeError::InterferenceMismatch { .. })
        ));

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].virtual_registers[1].fixed_constraints[0]
            .view
            .0 += 1;
        assert!(matches!(
            validate_terminal_live_ranges(
                staged.selected_stage().selected(),
                staged.liveness(),
                corrupted,
            ),
            Err(TerminalLiveRangeError::VirtualRegisterMismatch { .. })
        ));

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].architectural_units[0].actions[0]
            .point
            .0 += 1;
        assert!(matches!(
            validate_terminal_live_ranges(
                staged.selected_stage().selected(),
                staged.liveness(),
                corrupted,
            ),
            Err(TerminalLiveRangeError::ArchitecturalUnitMismatch { .. })
        ));

        let arm =
            stage_optimized_liveness(staged_forwarded_conditional(NativeTarget::linux_arm64()))
                .unwrap();
        let arm_ranges =
            analyze_terminal_live_ranges(arm.selected_stage().selected(), arm.liveness()).unwrap();
        assert!(matches!(
            validate_optimized_live_range_custody(&staged, &arm_ranges),
            Err(OptimizedLiveRangeCustodyError::Revalidation(
                TerminalLiveRangeError::RootMismatch
            ))
        ));
    }

    #[test]
    fn selected_liveness_is_deterministic_and_identity_binds_every_domain() {
        let first =
            stage_optimized_liveness(staged_conditional(NativeTarget::linux_x64())).unwrap();
        let second =
            stage_optimized_liveness(staged_conditional(NativeTarget::linux_x64())).unwrap();
        assert_eq!(first.liveness(), second.liveness());
        assert_eq!(first.custody(), second.custody());

        let original = first.liveness().plan();
        let identity = terminal_liveness_identity(original);
        let mut mutations = Vec::new();
        let mut changed = original.clone();
        changed.selected =
            omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity::from_canonical_bytes(
                b"changed-selected",
            );
        mutations.push(changed);
        let mut changed = original.clone();
        changed.target = NativeTarget::windows_x64();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.fuel_schedule = psi_core::FuelScheduleIdentity::new(
            original.fuel_schedule.marker().checked_add(1).unwrap(),
        )
        .unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.optimization_unit =
            omega_optimization_core::OptimizationUnitIdentity::from_canonical_bytes(b"changed");
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].machine = MachineId::new(8_101).unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].entry_definitions[0].fixed_view = None;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].entry_definitions[0].virtual_register = TerminalVirtualRegisterId(8);
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].entry_definitions[0].class.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].operand_positions[0].position.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].operand_positions[0].instruction.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].operand_positions[0].operand += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].operand_positions[0].virtual_register.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].operand_positions[0].access = RegisterOperandAccess::Def;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].operand_positions[0].class.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].operand_positions[0].fixed_view =
            changed.functions[0].entry_definitions[0].fixed_view;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].operand_positions[0].tied_to = Some(0);
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].operand_positions[0].early_clobber = true;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].block.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].source_block = BlockId::new(8_103).unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0]
            .virtual_live_in
            .push(TerminalVirtualRegisterId(9));
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0]
            .unit_live_in
            .push(RegisterUnitId(u16::MAX));
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0]
            .virtual_live_out
            .push(TerminalVirtualRegisterId(9));
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0]
            .unit_live_out
            .push(RegisterUnitId(u16::MAX));
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[0].position.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[0].instruction.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[0]
            .virtual_uses
            .clear();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .virtual_defs
            .clear();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .virtual_live_in
            .push(TerminalVirtualRegisterId(9));
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[1].instructions[0]
            .virtual_live_out
            .clear();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[1]
            .unit_uses
            .clear();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[0]
            .unit_defs
            .clear();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[0]
            .unit_clobbers
            .push(RegisterUnitId(u16::MAX));
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[0]
            .unit_live_in
            .clear();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[0]
            .unit_live_out
            .clear();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].successors[0].polarity_ordinal = 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].successors[0].psi_edge = EdgeId::new(8_102).unwrap();
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].successors[0].terminator.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].successors[0].target.0 += 1;
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].successors[0]
            .virtual_live
            .push(TerminalVirtualRegisterId(9));
        mutations.push(changed);
        let mut changed = original.clone();
        changed.functions[0].blocks[0].successors[0]
            .unit_live
            .clear();
        mutations.push(changed);
        for mutation in mutations {
            assert_ne!(terminal_liveness_identity(&mutation), identity);
        }
    }

    #[test]
    fn independent_liveness_validator_rejects_raw_transfer_and_path_corruption() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let selected = staged_conditional(target);
            let valid = analyze_terminal_liveness(selected.selected()).unwrap();

            let mut corrupted = valid.plan().clone();
            corrupted.functions[0].blocks[0].virtual_live_in.clear();
            assert!(matches!(
                validate_terminal_liveness(selected.selected(), corrupted),
                Err(TerminalLivenessError::BlockMismatch { .. })
            ));

            let mut corrupted = valid.plan().clone();
            corrupted.functions[0].blocks[0].instructions[0]
                .unit_live_out
                .clear();
            assert!(matches!(
                validate_terminal_liveness(selected.selected(), corrupted),
                Err(TerminalLivenessError::TransferMismatch { .. })
            ));

            let mut corrupted = valid.plan().clone();
            corrupted.functions[0].blocks[1].instructions[1]
                .virtual_live_in
                .clear();
            assert!(matches!(
                validate_terminal_liveness(selected.selected(), corrupted),
                Err(TerminalLivenessError::TransferMismatch { .. })
            ));

            let mut corrupted = valid.plan().clone();
            corrupted.functions[0].blocks[0].successors.swap(0, 1);
            assert!(matches!(
                validate_terminal_liveness(selected.selected(), corrupted),
                Err(TerminalLivenessError::SuccessorMismatch { .. })
            ));

            let mut corrupted = valid.plan().clone();
            corrupted.functions[0].blocks[2].instructions[0].position.0 = 99;
            assert!(matches!(
                validate_terminal_liveness(selected.selected(), corrupted),
                Err(TerminalLivenessError::NonDensePositions { .. })
            ));
        }
    }

    #[test]
    fn liveness_custody_rejects_a_detached_same_shape_target() {
        let x86 = staged_conditional(NativeTarget::linux_x64());
        let arm = staged_conditional(NativeTarget::linux_arm64());
        let arm_liveness = analyze_terminal_liveness(arm.selected()).unwrap();
        assert!(matches!(
            validate_optimized_liveness_custody(&x86, &arm_liveness),
            Err(OptimizedLivenessCustodyError::Revalidation(
                TerminalLivenessError::RootMismatch
            ))
        ));
    }

    fn staged_callable_object_artifact(
        target: NativeTarget,
        selected_lowering: bool,
    ) -> StagedValidatedOptimizedTerminalObjectArtifact {
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let layout = match target.architecture {
            omega_target::Architecture::X86_64 => Optimization::X86RelaxConditionalBranchesToRel8V1,
            omega_target::Architecture::Aarch64 => {
                Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1
            }
        };
        let selections = if selected_lowering {
            OptimizationSelections::new([
                Optimization::SelectedIncomingU12ExactAddImmediate,
                layout,
            ])
            .unwrap()
        } else {
            OptimizationSelections::new([layout]).unwrap()
        };
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
        )
        .unwrap();
        let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap();
        let source = match physical {
            StagedOptimizedVerifiedPhysicalPipeline::FunctionRelativeLayout { realization } => {
                StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(Box::new(realization))
            }
            StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } => {
                StagedOptimizedFunctionFragmentEmissionSource::Aarch64CbnzDirect(Box::new(
                    realization,
                ))
            }
            StagedOptimizedVerifiedPhysicalPipeline::SelectedLowering { realization } => {
                StagedOptimizedFunctionFragmentEmissionSource::X86Rel8AfterSelectedLowering(
                    Box::new(realization),
                )
            }
            StagedOptimizedVerifiedPhysicalPipeline::SelectedLoweringPostAllocationMachine {
                realization,
            } => StagedOptimizedFunctionFragmentEmissionSource::Aarch64CbnzAfterSelectedLowering(
                Box::new(realization),
            ),
            _ => panic!("fixture must complete a function-relative realization"),
        };
        let fragments = stage_optimized_function_fragment_emission(source).unwrap();
        let text = stage_optimized_relocation_free_text_section(fragments).unwrap();
        let object = stage_optimized_relocation_free_terminal_object_container(text).unwrap();
        stage_validated_optimized_terminal_object_artifact(
            canonical_terminal_artifact(&semantic, &proof),
            object,
        )
        .unwrap()
    }

    #[test]
    fn ordinary_callable_entry_replays_target_abi_and_edge_specific_results() {
        use omega_calling_conventions::{CallingPolicy, MachineRegister};

        for (target, policy, parameter, result) in [
            (
                NativeTarget::linux_x64(),
                CallingPolicy::SystemVAMD64,
                MachineRegister::X86Rdi,
                MachineRegister::X86Rax,
            ),
            (
                NativeTarget::windows_x64(),
                CallingPolicy::MicrosoftX64,
                MachineRegister::X86Rcx,
                MachineRegister::X86Rax,
            ),
            (
                NativeTarget::linux_arm64(),
                CallingPolicy::Aapcs64,
                MachineRegister::Aarch64X(0),
                MachineRegister::Aarch64X(0),
            ),
            (
                NativeTarget::macos_arm64(),
                CallingPolicy::Aapcs64,
                MachineRegister::Aarch64X(0),
                MachineRegister::Aarch64X(0),
            ),
        ] {
            let artifact = staged_callable_object_artifact(target, false);
            let object_identity = artifact.source().object().identity;
            let object_bytes = artifact.source().container().bytes.clone();
            let staged = stage_validated_optimized_terminal_ordinary_callable_entry(artifact)
                .expect("ordinary callable classification");
            assert_eq!(
                validate_optimized_terminal_ordinary_callable_entry(&staged).unwrap(),
                staged.custody()
            );
            let entry = staged.entry();
            assert_eq!(entry.calling_policy, policy);
            assert_eq!(entry.parameters.len(), 1);
            assert_eq!(entry.parameters[0].abi_register, parameter);
            assert_eq!(
                entry.parameters[0].fixed_view,
                entry.parameters[0].assigned_view
            );
            assert_eq!(entry.result.abi_register, result);
            assert_eq!(entry.returns.len(), 2);
            assert_ne!(entry.returns[0].value, entry.returns[1].value);
            assert_ne!(
                entry.returns[0].virtual_register,
                entry.returns[1].virtual_register
            );
            assert!(entry.returns.iter().all(|returned| {
                returned.view == entry.result.view
                    && returned.storage_units == entry.result.storage_units
            }));
            assert_eq!(staged.source().source().object().identity, object_identity);
            assert_eq!(staged.source().source().container().bytes, object_bytes);
            assert_eq!(staged.source().source().object().relocation_record_count, 0);
            assert_ne!(entry.semantic_entry_symbol_name, "main");
            assert_ne!(entry.semantic_entry_symbol_name, "_main");
            assert_eq!(
                OptimizedTerminalOrdinaryCallableEntryRecord::decode(&entry.encode().unwrap())
                    .unwrap(),
                *entry
            );
            assert_eq!(
                OptimizedTerminalOrdinaryCallableEntryManifest::decode(
                    &staged.manifest().record().encode()
                )
                .unwrap(),
                *staged.manifest().record()
            );
        }
    }

    #[test]
    fn ordinary_callable_entry_accepts_both_selected_lowering_compositions_and_reports_opaquely() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let staged = stage_validated_optimized_terminal_ordinary_callable_entry(
                staged_callable_object_artifact(target, true),
            )
            .unwrap();
            let artifact_identity = staged.source().artifact().identity;
            let container_identity = staged.source().source().container().identity;
            let container_bytes = staged.source().source().container().bytes.clone();
            assert_eq!(
                validate_optimized_terminal_ordinary_callable_entry(&staged).unwrap(),
                staged.custody()
            );
            let prior = optimization_pipeline_report_from_terminal_object_artifact(staged.source());
            assert!(prior.ordinary_callable_entry().is_none());
            let report =
                optimization_pipeline_report_from_terminal_ordinary_callable_entry(&staged);
            assert_eq!(
                report.ordinary_callable_entry(),
                Some(staged.manifest().record())
            );
            assert!(
                report
                    .render_human_text(OptimizationReportRequest::Suppressed)
                    .is_none()
            );
            let text = report
                .render_human_text(OptimizationReportRequest::EmitHumanText)
                .unwrap();
            assert!(text.contains("external process entry bridge: required"));
            assert!(text.contains("publication: unavailable"));
            assert_eq!(staged.source().artifact().identity, artifact_identity);
            assert_eq!(
                staged.source().source().container().identity,
                container_identity
            );
            assert_eq!(staged.source().source().container().bytes, container_bytes);
        }
    }

    #[test]
    fn ordinary_callable_entry_rejects_record_manifest_and_codec_corruption() {
        let mut staged = stage_validated_optimized_terminal_ordinary_callable_entry(
            staged_callable_object_artifact(NativeTarget::linux_x64(), false),
        )
        .unwrap();
        staged.entry_mut().returns[0].value = ValueId::new(99_991).unwrap();
        assert_eq!(
            validate_optimized_terminal_ordinary_callable_entry(&staged),
            Err(OptimizedTerminalOrdinaryCallableEntryError::RecordMismatch)
        );

        let mut staged = stage_validated_optimized_terminal_ordinary_callable_entry(
            staged_callable_object_artifact(NativeTarget::linux_x64(), false),
        )
        .unwrap();
        staged.entry_mut().parameters[0].storage_units.clear();
        assert_eq!(
            validate_optimized_terminal_ordinary_callable_entry(&staged),
            Err(OptimizedTerminalOrdinaryCallableEntryError::RecordMismatch)
        );

        let mut staged = stage_validated_optimized_terminal_ordinary_callable_entry(
            staged_callable_object_artifact(NativeTarget::linux_x64(), false),
        )
        .unwrap();
        staged.entry_mut().semantic_entry_symbol_name = "main".to_owned();
        assert_eq!(
            validate_optimized_terminal_ordinary_callable_entry(&staged),
            Err(OptimizedTerminalOrdinaryCallableEntryError::RecordMismatch)
        );

        let mut staged = stage_validated_optimized_terminal_ordinary_callable_entry(
            staged_callable_object_artifact(NativeTarget::linux_x64(), false),
        )
        .unwrap();
        staged.entry_mut().exit_policy =
            TerminalWholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1;
        assert_eq!(
            validate_optimized_terminal_ordinary_callable_entry(&staged),
            Err(OptimizedTerminalOrdinaryCallableEntryError::RecordMismatch)
        );

        let mut staged = stage_validated_optimized_terminal_ordinary_callable_entry(
            staged_callable_object_artifact(NativeTarget::linux_x64(), false),
        )
        .unwrap();
        staged.manifest_mut().record_mut().return_count += 1;
        assert_eq!(
            validate_optimized_terminal_ordinary_callable_entry(&staged),
            Err(OptimizedTerminalOrdinaryCallableEntryError::ManifestMismatch)
        );

        let mut staged = stage_validated_optimized_terminal_ordinary_callable_entry(
            staged_callable_object_artifact(NativeTarget::linux_x64(), false),
        )
        .unwrap();
        staged.corrupt_custody_for_test();
        assert_eq!(
            validate_optimized_terminal_ordinary_callable_entry(&staged),
            Err(OptimizedTerminalOrdinaryCallableEntryError::ReceiptMismatch)
        );

        let staged = stage_validated_optimized_terminal_ordinary_callable_entry(
            staged_callable_object_artifact(NativeTarget::linux_x64(), false),
        )
        .unwrap();
        let mut wrong_magic = staged.entry().encode().unwrap();
        wrong_magic[0] ^= 1;
        assert_eq!(
            OptimizedTerminalOrdinaryCallableEntryRecord::decode(&wrong_magic),
            Err(OptimizedTerminalOrdinaryCallableEntryDecodeError::WrongMagic)
        );
        let mut wrong_version = staged.manifest().record().encode();
        wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            OptimizedTerminalOrdinaryCallableEntryManifest::decode(&wrong_version),
            Err(OptimizedTerminalOrdinaryCallableEntryManifestDecodeError::UnsupportedVersion(2))
        );
        let mut trailing = staged.entry().encode().unwrap();
        trailing.push(0);
        assert_eq!(
            OptimizedTerminalOrdinaryCallableEntryRecord::decode(&trailing),
            Err(OptimizedTerminalOrdinaryCallableEntryDecodeError::TrailingBytes)
        );
        let mut stale = staged.entry().encode().unwrap();
        stale[12] ^= 1;
        assert_eq!(
            OptimizedTerminalOrdinaryCallableEntryRecord::decode(&stale),
            Err(OptimizedTerminalOrdinaryCallableEntryDecodeError::IdentityMismatch)
        );
    }
}
