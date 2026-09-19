//! Optimizer module role: stage group.
use super::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AllocatedCalleeSavedRequirementError,
    AllocatedCalleeSavedRequirementIdentity, AllocatedCalleeSavedRequirementPlan,
    AllocatedCalleeSavedRequirementPolicy, AllocationEvidence, AllocationReplayError,
    AllocationSource, CalleeSavedModificationWitness, EmptyOptimizationSelections,
    ExplicitOptimizationRequest, FixedFramePublicationCustodyFieldForTest,
    FrameAbiPreservationConvention, FunctionFragmentEmissionError,
    FunctionFragmentEmissionManifest, FunctionFragmentEmissionManifestDecodeError,
    FunctionFragmentFrameApplicationError, FunctionFragmentFrameApplicationIdentity,
    FunctionFragmentObjectContainerManifest, FunctionFragmentReplayInputs,
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionManifestDecodeError,
    FunctionRelativeOptimizationRealizationError, FunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationUnavailableData, FunctionTargetFrameLayout, LiteralFoldPolicy,
    LiveRangeError, LivenessError, NonAuthoritativeCalleeSaveSlotId,
    NonAuthoritativeCalleeSaveStorageError, NonAuthoritativeCalleeSaveStoragePlan,
    NonAuthoritativeCalleeSaveStoragePolicy, NonAuthoritativeSpillFrameRequirementPlan,
    NonAuthoritativeSpillFrameRequirementPolicy, OptimizationPipelineError,
    OptimizationPipelineRequest, OptimizedActiveResidentRematerializationCustodyFieldForTest,
    OptimizedActiveResidentRematerializationError,
    OptimizedActiveResidentRematerializationPressureCustodyFieldForTest,
    OptimizedAllocationLegalityCustodyFieldForTest,
    OptimizedFixedPrecoloredSegmentHomeCustodyFieldForTest, OptimizedFixedViewCopyCustodyError,
    OptimizedFixedViewCopyCustodyFieldForTest, OptimizedLiveRangeCustodyError,
    OptimizedLiveRangeCustodyFieldForTest, OptimizedLivenessCustodyError,
    OptimizedLivenessCustodyFieldForTest, OptimizedObjectArtifactError,
    OptimizedObjectArtifactManifest, OptimizedObjectArtifactRecord,
    OptimizedObjectArtifactUnavailableData, OptimizedOrdinaryCallableEntryDecodeError,
    OptimizedOrdinaryCallableEntryDisposition, OptimizedOrdinaryCallableEntryError,
    OptimizedOrdinaryCallableEntryManifest, OptimizedOrdinaryCallableEntryManifestDecodeError,
    OptimizedOrdinaryCallableEntryRecord, OptimizedPostAllocationMachinePipelineError,
    OptimizedPostCopyRegisterHomeCustodyFieldForTest, OptimizedPostLiteralFoldHomeCustodyError,
    OptimizedPostLiteralFoldHomeCustodyFieldForTest, OptimizedPostSelectedLoweringHomeCustodyError,
    OptimizedPostSelectedLoweringHomeCustodyFieldForTest, OptimizedRegisterHomeCustodyFieldForTest,
    OptimizedResolvedSelectedFormLayoutError, OptimizedSelectedFormEncodingError,
    OptimizedSelectedReanalysisCustodyFieldForTest, OptimizedSelectionCustodyFieldForTest,
    OptimizedSelectionPipelineError, OptimizedTargetLoweringRequest,
    OptimizedX86BranchRelaxationError, PostAllocationMachineCustodyFieldForTest,
    PostAllocationMachinePlanReceiptFieldForTest, PostAllocationOptimizationManifest,
    PostAllocationOptimizationManifestError, PostAllocationSelectedTransformation,
    PressureRematerializationError, PressureRematerializationPolicy, RegisterHomeError,
    RegisterHomePlan, Rel8ExitBoundaryForTest, RelocationFreeObjectContainerError,
    RelocationFreeTextSectionPlacementError, ResolvedLayoutOptimizationError,
    ResolvedSelectedFormLayoutIdentity, ReturnAddressFrameCustody, RuntimeSpillAllocationError,
    SelectedFormEncodingState, SelectedFunctionLayoutPolicy, SpillFrameRequirementError,
    StagedFixedFrameFunctionRelativeRealization, StagedFunctionFragmentFrameApplication,
    StagedOptimizedActiveResidentRematerialization, StagedOptimizedAllocationLegality,
    StagedOptimizedFixedFrameTextSection, StagedOptimizedFixedPrecoloredSegmentHomes,
    StagedOptimizedFixedViewCopies, StagedOptimizedFunctionFragmentEmission,
    StagedOptimizedFunctionFragmentEmissionSource, StagedOptimizedLiveness,
    StagedOptimizedPostAllocationMachinePlan, StagedOptimizedRegisterHomes,
    StagedOptimizedRelocationFreeObjectContainer, StagedOptimizedSelectedFormEncoding,
    StagedOptimizedSelectedInstructions, StagedOptimizedVerifiedPhysicalPipeline,
    StagedOptimizedX86BranchRelaxation, StagedValidatedOptimizedObjectArtifact,
    StagedValidatedOptimizedOrdinaryCallableEntry, TargetFrameLayoutError,
    TargetFrameLayoutIdentity, TargetFrameLayoutPlan, TargetFrameLayoutPolicy,
    TargetFrameProtocolEncodingError, TargetFrameProtocolEncodingIdentity,
    TargetFrameProtocolEncodingPlan, TargetFrameProtocolEncodingPolicy,
    ValidatedAllocatedCalleeSavedRequirements, ValidatedNonAuthoritativeCalleeSaveStorage,
    ValidatedNonAuthoritativeSpillFrameRequirements, ValidatedOptimizedTargetOperations,
    ValidatedTargetFrameLayout, ValidatedTargetFrameProtocolEncoding,
    ValidatedTargetRegisterEnvironment, WholeFunctionEntryAssumption, WholeFunctionExitContract,
    WholeFunctionExitContractError, WholeFunctionExitContractIdentity,
    WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy, WholeFunctionFrameDisposition,
    WholeFunctionReturnMechanism, WholeFunctionReturnValueEvidence, X86BranchRelaxationIdentity,
    X86BranchRelaxationWorkAxis, allocated_callee_saved_requirement_identity,
    analyze_machine_effects, baseline_target_register_environment, compiler_baseline_request_v1,
    corrupt_fixed_frame_realization_custody_for_test,
    corrupt_fixed_frame_realization_encoding_for_test,
    corrupt_fixed_frame_realization_exit_for_test, corrupt_fixed_frame_realization_layout_for_test,
    corrupt_fixed_frame_realization_manifest_for_test, execute_resolved_layout_optimization,
    lower_optimized_to_target_operations, materialize_allocator_availability,
    non_authoritative_callee_save_storage_identity,
    non_authoritative_spill_frame_requirement_identity, optimization_pipeline_report,
    optimization_pipeline_report_from_object_artifact,
    optimization_pipeline_report_from_ordinary_callable_entry, optimize_artifact_sections,
    register_home_identity, replace_fixed_frame_realization_exit_for_test,
    run_selected_lowering_optimizations, selected_abi_preservation,
    stage_active_resident_register_allocation, stage_allocated_callee_saved_requirements,
    stage_first_optimized_literal_fold, stage_fixed_frame_function_relative_realization,
    stage_function_fragment_frame_application, stage_leaf_local_fixed_view_register_allocation,
    stage_leaf_local_fixed_view_register_allocation_composing, stage_next_optimized_literal_fold,
    stage_non_authoritative_callee_save_storage, stage_non_authoritative_spill_frame_requirements,
    stage_optimized_active_resident_rematerialization,
    stage_optimized_active_resident_rematerialization_pressure,
    stage_optimized_allocation_legality,
    stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1,
    stage_optimized_allocation_legality_with_availability,
    stage_optimized_fixed_frame_text_section, stage_optimized_fixed_precolored_segment_homes,
    stage_optimized_fixed_view_copies, stage_optimized_function_fragment_emission,
    stage_optimized_layout_independent_selected_form_encoding, stage_optimized_live_ranges,
    stage_optimized_liveness, stage_optimized_post_allocation_machine_plan,
    stage_optimized_register_homes, stage_optimized_register_homes_after_fixed_view_copies,
    stage_optimized_register_homes_after_literal_folds,
    stage_optimized_register_homes_after_selected_lowering,
    stage_optimized_relocation_free_object_container,
    stage_optimized_resolved_selected_form_layout, stage_optimized_selected_reanalysis,
    stage_optimized_verified_physical_pipeline, stage_optimized_x86_branch_relaxation,
    stage_register_allocation, stage_shared_entry_fixed_view_register_allocation,
    stage_target_frame_layout, stage_target_frame_protocol_encoding,
    stage_validated_optimized_object_artifact, stage_validated_optimized_ordinary_callable_entry,
    swap_fixed_frame_realization_source_for_test, validate_allocated_callee_saved_requirements,
    validate_fixed_frame_function_relative_realization,
    validate_function_fragment_frame_application, validate_machine_effects,
    validate_non_authoritative_callee_save_storage,
    validate_non_authoritative_spill_frame_requirements,
    validate_optimized_active_resident_rematerialization,
    validate_optimized_active_resident_rematerialization_pressure,
    validate_optimized_allocation_legality_custody, validate_optimized_fixed_frame_text_section,
    validate_optimized_fixed_precolored_segment_home_custody,
    validate_optimized_fixed_view_copy_custody, validate_optimized_function_fragment_emission,
    validate_optimized_layout_independent_selected_form_encoding,
    validate_optimized_literal_fold_custody, validate_optimized_live_range_custody,
    validate_optimized_liveness_custody, validate_optimized_object_artifact,
    validate_optimized_ordinary_callable_entry,
    validate_optimized_post_allocation_machine_plan_custody,
    validate_optimized_register_home_after_fixed_view_copy_custody,
    validate_optimized_register_home_after_literal_fold_custody,
    validate_optimized_register_home_after_selected_lowering_custody,
    validate_optimized_register_home_custody, validate_optimized_relocation_free_object_container,
    validate_optimized_resolved_selected_form_layout,
    validate_optimized_selected_reanalysis_custody, validate_optimized_selection_custody,
    validate_resolved_layout_optimization, validate_selected_lowering_optimization_custody,
    validate_target_frame_layout, validate_target_frame_protocol_encoding,
};
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
mod native_execution;
mod text_placement_checks;
use std::collections::BTreeSet;

use abstract_operations::{AbstractOperation, ValueBinding};
use abstract_operations_to_abstract_operations::OptimizationRunError;
use calling_conventions::{IndirectPointerLocation, MachineRegister, ValueLocation};
use native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions;
use optimization_core::{
    Optimization, OptimizationReportRequest, OptimizationSelections, OptimizationWorkBudget,
    OptimizationWorkUsage,
};
use optimization_unit::{FuelSettlement, OwnershipEvent, PsiProvenance, ValueDefinitionSite};
use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use register_homes::{
    AllocatorAvailabilityPolicy, RecoveryClassification, RecoveryClassificationPolicy,
    RecoveryVictimRole, SpillChoicePolicy, allocation_legality_identity,
};
use register_model::{
    RegisterOperandAccess, RegisterReservationProfile, RegisterUnitId, RegisterViewId,
    target_register_environment_identity, validate_register_reservation_profile,
};
use selected_instructions::{
    ArchitecturalUnitActionKind, LiveRangeFragment, LiveRangePoint, VirtualFixedConstraintSite,
    VirtualInterference, live_range_identity, liveness_identity,
};
use selected_instructions::{
    MachineBarrier, SelectedInstructionId, SelectedInstructionKind, SelectedTerminator,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use selected_instructions_to_register_homes::{
    AllocationLegalityError, FixedViewCopyError, FixedViewCopyPolicy, analyze_live_ranges,
    analyze_liveness, choose_spill_victims, classify_pressure_recovery,
    validate_allocation_legality, validate_fixed_view_copies, validate_live_ranges,
    validate_liveness, validate_post_allocation_optimization_manifest, validate_register_homes,
};
use semantic_vocabulary::{
    BlockId, ContractId, DomainSemanticId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType,
    IntegerValue, MachineId, ObligationId, OperationId, PlaceId, ScalarType, StructuralDomainId,
    StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::{
    LegalizationError, SelectedInstructionError, legalize_target_operations,
    selected_instruction_plan_identity, validate_legalized_operations,
    validate_selected_instructions,
};
use terminal_psi::{
    BindingRelevance, Block, MachineContract, Operation, OperationKind, OperationResult,
    StructuralAccess, StructuralDomainDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{ObligationEvidence, ProofBundle, reconstruct_operation_obligations};

/// Test shorthand for the production target-setup then instruction-selection sequence.
fn stage_optimized_instruction_selection(
    optimized_target: ValidatedOptimizedTargetOperations,
) -> Result<StagedOptimizedSelectedInstructions, OptimizedSelectionPipelineError> {
    let environment = baseline_target_register_environment(optimized_target.target())
        .expect("the baseline test register environment must validate");
    target_operations_to_selected_instructions::stage_optimized_instruction_selection(
        optimized_target,
        environment,
    )
}

pub(crate) mod fixtures;

pub(crate) use fixtures::*;

mod coordination;
mod cyclic_psi;
mod stages;
mod validation;
