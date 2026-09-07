//! Optimizer module role: stage group.
mod text_placement_checks;
use std::collections::BTreeSet;

use abstract_operations::{AbstractOperation, ValueBinding};
use abstract_operations_to_abstract_operations::OptimizationRunError;
use calling_conventions::{IndirectPointerLocation, MachineRegister, ValueLocation};
use legalized_operations::{
    LegalizationRecipe, LegalizationTheorem, LegalizedLeafValue, LegalizedTemporaryId,
    legalized_operation_plan_identity,
};
use optimization_core::{
    Optimization, OptimizationSelections, OptimizationWorkBudget, OptimizationWorkUsage,
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
    AllocationLegalityError, AllocatorAvailabilityError, FixedViewCopyError, FixedViewCopyPolicy,
    LiteralFoldPlan, LiteralFoldPolicy, LiveRangeError, LivenessError,
    PostAllocationOptimizationManifest, PostAllocationOptimizationManifestError,
    PostAllocationSelectedTransformation, PressureRematerializationError,
    PressureRematerializationPolicy, RegisterHomeError, RegisterHomePlan,
    analyze_allocation_legality, analyze_live_ranges, analyze_liveness, choose_spill_victims,
    classify_pressure_recovery, fixed_view_copy_identity, fold_selected_incoming_literal,
    materialize_allocator_availability, register_home_identity, validate_allocation_legality,
    validate_allocator_availability, validate_fixed_view_copies, validate_literal_fold,
    validate_live_ranges, validate_liveness, validate_post_allocation_optimization_manifest,
    validate_register_homes,
};
use semantic_vocabulary::{
    BlockId, ContractId, DomainSemanticId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType,
    IntegerValue, MachineId, ObligationId, OperationId, PlaceId, ScalarType, StructuralDomainId,
    StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use target::NativeTarget;
use target_operations::{
    TargetIntegerControl, TargetIntegerExpression, TargetOperation, TargetUnitOperation,
    TargetUnitScalarArgumentSource,
};
use target_operations_to_selected_instructions::{
    LegalizationError, SelectedInstructionError, legalization_validator_identity,
    legalize_target_operations, selected_instruction_plan_identity, validate_legalized_operations,
    validate_selected_instructions,
};
use terminal_psi::{
    BindingRelevance, Block, CrashCause, CrashRouteBucket, CrashRouteGuard, MachineContract,
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralDomainDeclaration,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{ObligationEvidence, ProofBundle, reconstruct_operation_obligations};

use super::*;
use native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions;

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
