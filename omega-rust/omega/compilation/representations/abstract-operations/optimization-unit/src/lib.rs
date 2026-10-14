#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Reconstructible, target-neutral optimization input derived from verified
//! Terminal Psi realization requirements.
//!
//! This crate deliberately performs no optimization. It makes the implicit
//! structure in [`AbstractOperationPlan`] explicit so independent
//! validators and later passes do not have to rediscover CFG, SSA, semantic
//! fuel, effects, or provenance from a mutable instruction stream.

use std::{collections::BTreeSet, sync::Arc};

use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan,
    AbstractSuccessor, ValueBinding,
};
use optimization_core::{
    AcceptedObligationFactIdentity, OptimizationUnitIdentity, OwnershipFrontierFactIdentity,
    ProofQuestionIdentity, ScalarConstantFactIdentity, ValueRangeFactIdentity,
};
use semantic_vocabulary::{
    AdmissionSiteId, BlockId, ClaimId, ContractId, EdgeId, EvidenceIdentity, FuelScheduleIdentity,
    IntegerCarrier, IntegerType, IntegerValue, MachineId, ObligationId, OperationId, PlaceId,
    ScalarType, ServiceId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BoundaryMachineDeclaration, ContentEntryClaim, EntryClaim, EvidenceContractLane,
    MachineContract, ProviderCandidateConformance, ServiceDeclaration, StructuralDomainDeclaration,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, TerminalAffineCleanupAction, TerminalPsiIdentity,
    TerminalRootServiceReach,
};

pub(crate) use optimization_core::codec_bytes;
mod cursor;
mod optimization_unit;

pub use optimization_unit::{
    AbstractOptimizationEvidence, AcceptedObligationFact, AcceptedObligationFactIndexError,
    AdjacentBlockMergeRewrite, BlockParameterIncomingBinding, BooleanConstantRewrite,
    CaseMembershipSpecializationRewrite, ConstantConditionalRewrite, CycleComponentEdge,
    CycleComponentId, DeadScalarNodeRewrite, DispatchCondition,
    DominatingScalarCommonSubexpressionRewrite, EffectLink, Establishment, FieldEvidence,
    FieldValueResolution, FieldValueRow, FieldValueSpecializationRewrite, FoldedCaseMembershipRow,
    FoldedFieldValue, ForwardedFieldValue, FuelSettlement, FunctionAnalysis,
    IntegerConstantRewrite, IntegerEvaluationWitness, IntervalExtraction,
    InvalidPsiTransformationLedger, LinearEmptyBlockRewrite, LocalScalarCommonSubexpressionRewrite,
    LoopInvariantNodeResult, LoopInvariantScalarMotionCandidate, LoopInvariantScalarNode,
    LoopInvariantScalarRelocation, NodeLocation, NonAdjacentBlockMergeRewrite,
    ObservationKnowledge, OptimizationBlock, OptimizationEdge, OptimizationFact,
    OptimizationManifestStage, OptimizationNode, OptimizationRunUsage,
    OptimizationStructuralStatistics, OptimizationUnitBuildError, OptimizerCycleComponent,
    OptimizerCycleComponentSnapshot, OptimizerRankingCertificateSnapshot,
    OptimizerUnsignedCountdownRankingCertificate, OptimizerUnsignedMinusOneDescent,
    OptimizerUnsignedPositiveGuard, OwnershipEvent, OwnershipFrontierFact,
    OwnershipFrontierLiveClaim, OwnershipFrontierOwnedPlace, OwnershipFrontierPartialCustody,
    OwnershipFrontierSite, OwnershipFrontierSnapshot, OwnershipFrontierWitness,
    OwnershipFrontierWitnessRow, PathQualifiedEmptyBlockRewrite, PhiTranslatedScalarGvnRewrite,
    PhiTranslatedScalarIncoming, PhysicalOptimizationDataStatus, Position,
    PrePhysicalOptimizationManifest, ProofCertifiedScalarIdentityKind,
    ProofCertifiedScalarIdentityRewrite, ProofQuestionIndexError, ProofQuestionOwner,
    ProvenanceDisposition, ProvenanceRewrite, PrunedMachineCustody, PsiCandidateDeclaration,
    PsiNodeObservation, PsiOptimizationCommit, PsiOptimizationFunction, PsiOptimizationUnit,
    PsiProvenance, PsiRealizationSite, PsiRewriteCandidate, PsiRewriteCandidateError,
    PsiRewriteDecisionPoint, PsiRewritePatch, PsiTransformationLedger, PsiTransformationRecord,
    PsiValidatedCandidateDeclaration, RedundantBlockParameterRewrite,
    RedundantBlockParameterWitness, ScalarConstantValue, ScalarSubstitution, SccpBlockRow,
    SccpEdgeRow, SccpEdgeState, SccpMachineSnapshot, SccpValueRow, SccpValueState,
    SharedJumpFusionRewrite, SourceCheckedScalarReceiving, SpecializedStateEdgeRow,
    StateArgumentSpecializationRewrite, TotalScalarIdentityKind, TotalScalarIdentityRewrite,
    UnreachablePrivateMachinesRewrite, ValueDefinition, ValueDefinitionSite, ValueRangeFact,
    ValueRangeRegion, ValueRangeScope, ValueRangeSupport, ValueUse, admit_field_node,
    argument_rewrites_place, attach_accepted_obligation_facts, attach_ownership_frontier_facts,
    attach_verified_module_context, bound_place, bound_value,
    canonical_ownership_frontier_snapshot, declared_structural_type,
    derived_sccp_scalar_constant_fact_identity, dominators, encode_location,
    encode_value_definition_site_identity, established_case_at, extract_integer_intervals,
    field_evidence, function_analysis, integer_comparison_condition,
    literal_scalar_constant_fact_identity, observed_field_declaration, operation_definition,
    placed_view_catalog_identity_bytes, proof_basis, proof_range_goal,
    reachable_blocks_after_conditional_fold, recompute_psi_optimization_unit_identity,
    reconstruct_psi_closed_region_observation, reconstruct_psi_observation_model,
    reconstruct_psi_optimization_function_seed, reconstruct_psi_optimization_unit_seed,
    resolved_sole_case, root_producer, scalar_refinement_identity_bytes, scalar_value_definition,
    shape, sole_case_at_path, structural_child, value_range_fact_identity,
};
pub(crate) use optimization_unit::{ProofQuestion, ProofQuestionAdmissionKind, ProofQuestionClass};
