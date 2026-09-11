//! Optimizer module role: stage group.
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
mod native_execution;
mod text_placement_checks;
use std::collections::BTreeSet;

use abstract_operations::{AbstractOperation, ValueBinding};
use abstract_operations_to_abstract_operations::OptimizationRunError;
use calling_conventions::{IndirectPointerLocation, MachineRegister, ValueLocation};
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
