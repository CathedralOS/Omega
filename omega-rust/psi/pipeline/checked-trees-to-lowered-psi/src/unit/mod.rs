//! Unit machine lowering.
//!
//! Owns the producers dispatched for Unit plans: attached closure assembly,
//! bounded dynamic composed dispatch, structural multi-state control, affine
//! cleanup, and the entry requirements every Unit body must satisfy.

use std::collections::{BTreeMap, BTreeSet};

use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedBooleanExpression, CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan,
    CheckedComposedUnitControlTerminatorPlan, CheckedIntegerComparisonKind,
    CheckedNominalAffineUnitCleanupMachinePlan, CheckedPartialAffineUnitCleanupMachinePlan,
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedStructuralUnitControlMachinePlan,
    CheckedStructuralUnitControlTerminatorPlan, CheckedTerminalSignatureEligibility, CheckedTrees,
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, CheckedUnitEntryClaimPlan,
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape, ClosedScalarContractValue,
};
use language_semantics::{
    CarryPolicy, Multiplicity, PermissionClaimIdentity, SemanticDomainId, ServiceReachId,
    ServiceReachInterface, ServiceReachPlan, ServiceReachRowId, ServiceReachSummary,
};
use lowered_psi::{LoweredPsi, LoweredSourceCallOccurrence};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, DomainSemanticId, EdgeId, IntegerSign, IntegerType,
    IntegerValue, MachineId, ObligationId, PlaceId, Proposition, QualifiedScalarType, ScalarTerm,
    ScalarType, ServiceId, StructuralCaseId, StructuralDomainId, StructuralFieldId,
    StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryStructuralResultDeclaration,
    ByteSequenceCarrier, ClaimTransfer, CompletionReceipt, ContractClause, EntryClaim,
    InstallationReachDependency, MachineContract, NominalAffineCleanup, Operation, OperationKind,
    OperationResult, ProgramLocalRootIntroductionSchema, ProviderCandidateConformance,
    ProviderParameterRefinement, ProviderRefinement, ProviderSignature, ProviderSignatureParameter,
    ScalarQualificationCatalog, ServiceDeclaration, StructuralAccess, StructuralAffineDiscard,
    StructuralArgument, StructuralCaseDeclaration, StructuralCaseSuccessorEdge,
    StructuralDomainDeclaration, StructuralDomainRequirement, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalBlockNaturalRank,
    TerminalMachine, TerminalMachineResult, TerminalModule, TerminalNaturalCycle,
    TerminalNaturalRankComparison, TerminalNaturalRankEdge, TerminalRankedScc, Terminator,
    ValueDeclaration, VocabularyMarker,
    program_local_root_introduction_compatibility_report_identity,
};
use terminal_verifier::{ObligationEvidence, ProofBundle};

use crate::emission::boolean_control::{
    boolean_decision_block_count, emit_reserved_boolean_tuple_stage_blocks,
    lower_boolean_value_decision,
};
use crate::emission::expression_validation::{
    contains_short_circuit, direct_expression_contains_short_circuit,
    validate_direct_parameter_types,
};
use crate::emission::operation_emission::{
    emit_boolean_expression, emit_direct_expression, emit_scalar_binding,
};
use crate::emission::scalar_types::{
    integer_landing_scalar_type, integer_scalar_type, integer_value, terminal_scalar_type,
};
use crate::expression_preparation::bindings::structural_paths::lower_structural_path;
use crate::expression_preparation::prepare_expression::lower_checked_scalar_expression;
use crate::lowering_error::{LoweringError, unsupported};
use crate::proofs::content_conservation::lower_boundary_content_guarantees;
use crate::proofs::crash_routes::{
    lower_boundary_crash_routes, lower_checked_crash_route_buckets,
    lower_structural_crash_route_buckets, structural_crash_route_argument_prefix,
    substitute_structural_crash_route_roots,
};
use crate::proofs::operation_proofs::finalize_operation_proofs;
use crate::proofs::{content_conservation, evidence_lowering};
use crate::retention::placed_view_inputs::lower_placed_view_input;
use crate::returns::structural_types::{
    lower_mixed_cases, lower_mixed_fields, lower_structural_type_plans,
    terminal_byte_sequence_carrier, terminal_structural_field_type,
};
use crate::scalar_graph::scalar_carriers;
use crate::terminal_identities::{
    TERMINAL_MACHINE_IDENTITY_STRIDE, TERMINAL_UNIT_CALL_OBLIGATION_BASE, allocate_dense, block_id,
    boundary_machine_id, claim_id, contract_id, dense_identity, edge_id, lookup_claim_id,
    lookup_domain_id, lookup_machine_id, lookup_service_id, lookup_type_id, machine_id,
    obligation_id, operation_id, place_id, service_id, structural_domain_id, structural_field_id,
    structural_type_id, value_id,
};
use crate::unit::attached_unit::{
    checked_unit_call_closure_including, checked_unit_target_reach_matches,
    collect_service_summary, lower_installation_machine_service_ceiling,
    lower_nominal_cleanup_closure, lower_root_service_reach, lower_unit_effect_closure,
    lower_unit_parameters, unique_unit_machine,
};

pub(crate) mod attached_unit;
pub(crate) mod crash_interface;
pub(crate) use crash_interface::effective_crash_routes;
pub(crate) mod plan_omissions;
pub(crate) use plan_omissions::unit_plan_omission_explanation;
pub(crate) mod dynamic_composed_unit;
pub(crate) mod runtime_requirements;
pub(crate) mod structural_unit_control;
pub(crate) mod unit_cleanup;
