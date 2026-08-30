use std::collections::BTreeSet;

use omega_abstract_operations::{AbstractOperation, ValueBinding};
use omega_calling_conventions::{IndirectPointerLocation, MachineRegister, ValueLocation};
use omega_legalized_operations::{
    LegalizationRecipe, LegalizationTheorem, LegalizedLeafValue, LegalizedTemporaryId,
    legalized_operation_plan_identity,
};
use omega_optimization_core::{
    Optimization, OptimizationSelections, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_optimization_unit::{FuelSettlement, OwnershipEvent, PsiProvenance, ValueDefinitionSite};
use omega_psi_optimizer::OptimizationRunError;
use omega_regalloc::{
    AllocationLegalityError, AllocatorAvailabilityError, AllocatorAvailabilityPolicy,
    ArchitecturalUnitActionKind, FixedViewCopyError, FixedViewCopyPolicy, LiteralFoldPlan,
    LiteralFoldPolicy, LiveRangeError, LiveRangeFragment, LiveRangePoint, LivenessError,
    PostAllocationOptimizationManifest, PostAllocationOptimizationManifestError,
    PostAllocationSelectedTransformation, PressureRematerializationError,
    PressureRematerializationPolicy, RecoveryClassification, RecoveryClassificationPolicy,
    RecoveryVictimRole, RegisterHomeError, RegisterHomePlan, SpillChoicePolicy,
    VirtualFixedConstraintSite, VirtualInterference, allocation_legality_identity,
    analyze_allocation_legality, analyze_live_ranges, analyze_liveness, choose_spill_victims,
    classify_pressure_recovery, fixed_view_copy_identity, fold_selected_incoming_literal,
    live_range_identity, liveness_identity, materialize_allocator_availability,
    register_home_identity, validate_allocation_legality, validate_allocator_availability,
    validate_fixed_view_copies, validate_literal_fold, validate_live_ranges, validate_liveness,
    validate_post_allocation_optimization_manifest, validate_register_homes,
};
use omega_register_model::{
    RegisterOperandAccess, RegisterReservationProfile, RegisterUnitId, RegisterViewId,
    target_register_environment_identity, validate_register_reservation_profile,
};
use omega_selected_instructions::{
    MachineBarrier, SelectedInstructionId, SelectedInstructionKind, SelectedTerminator,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use omega_target::NativeTarget;
use omega_target_operations::{TargetIntegerControl, TargetIntegerExpression, TargetOperation};
use omega_target_operations_to_selected_instructions::{
    LegalizationError, SelectedInstructionError, legalization_validator_identity,
    legalize_target_operations, selected_instruction_plan_identity, validate_legalized_operations,
    validate_selected_instructions,
};
use psi_core::{
    BlockId, ContractId, DomainSemanticId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType,
    IntegerValue, MachineId, ObligationId, OperationId, PlaceId, ScalarType, StructuralDomainId,
    StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use psi_proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker,
};
use psi_terminal::{
    BindingRelevance, Block, CrashCause, CrashRouteBucket, CrashRouteGuard, MachineContract,
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralDomainDeclaration,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration, VocabularyMarker,
};
use psi_terminal_verifier::{ObligationEvidence, ProofBundle, reconstruct_operation_obligations};

use super::*;
use crate::coordination::physical_pipeline::stage_optimized_verified_physical_pipeline_with_provider_executions;
use crate::stages::selection::assignment::{
    stage_optimized_assignment, validate_optimized_assignment_custody,
};

pub(crate) mod fixtures;

pub(crate) use fixtures::*;

mod coordination;
mod stages;
mod validation;
