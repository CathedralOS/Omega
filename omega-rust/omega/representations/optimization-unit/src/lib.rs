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
    IntegerCarrier, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId, OperationId,
    PlaceId, ScalarType, ServiceId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BoundaryMachineDeclaration, ContentEntryClaim, EntryClaim, EvidenceContractLane,
    MachineContract, ProviderCandidateConformance, ServiceDeclaration, StructuralDomainDeclaration,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, TerminalAffineCleanupAction, TerminalPsiIdentity,
    TerminalRootServiceReach,
};

mod optimization_unit;

pub use optimization_unit::{
    AbstractOptimizationEvidence, AcceptedObligationFact, AcceptedObligationFactIndexError,
    AdjacentBlockMergeRewrite, BlockParameterIncomingBinding, BooleanConstantRewrite,
    CaseMembershipSpecializationRewrite, ConstantConditionalRewrite, CycleComponentEdge,
    CycleComponentId, DeadScalarNodeRewrite, DominatingScalarCommonSubexpressionRewrite,
    EffectLink, FieldValueResolution, FieldValueRow, FieldValueSpecializationRewrite,
    FoldedCaseMembershipRow, FoldedFieldValue, ForwardedFieldValue, FuelSettlement,
    IntegerConstantRewrite, IntegerEvaluationWitness, InvalidPsiTransformationLedger,
    LinearEmptyBlockRewrite, LocalScalarCommonSubexpressionRewrite, NodeLocation,
    NonAdjacentBlockMergeRewrite, ObservationEventClass, ObservationKnowledge, OptimizationBlock,
    OptimizationEdge, OptimizationFact, OptimizationManifestStage, OptimizationNode,
    OptimizationRunUsage, OptimizationStructuralStatistics, OptimizationUnitBuildError,
    OptimizerCycleComponent, OptimizerCycleComponentSnapshot, OptimizerRankingCertificateSnapshot,
    OptimizerUnsignedCountdownRankingCertificate, OptimizerUnsignedMinusOneDescent,
    OptimizerUnsignedPositiveGuard, OwnershipEvent, OwnershipFrontierFact,
    OwnershipFrontierFactIndexError, OwnershipFrontierLiveClaim, OwnershipFrontierOwnedPlace,
    OwnershipFrontierPartialCustody, OwnershipFrontierSite, OwnershipFrontierSnapshot,
    OwnershipFrontierWitness, OwnershipFrontierWitnessRow, PathQualifiedEmptyBlockRewrite,
    PhiTranslatedScalarGvnRewrite, PhiTranslatedScalarIncoming, PhysicalOptimizationDataStatus,
    PrePhysicalOptimizationManifest, PrePhysicalOptimizationManifestDecodeError,
    ProofCertifiedScalarIdentityKind, ProofCertifiedScalarIdentityRewrite, ProofQuestion,
    ProofQuestionAdmissionKind, ProofQuestionClass, ProofQuestionIndexError, ProofQuestionOwner,
    ProvenanceDisposition, ProvenanceRewrite, PrunedMachineCustody,
    PsiClosedRegionBlockObservation, PsiClosedRegionObservation, PsiClosedRegionSemantics,
    PsiNodeObservation, PsiObservableEvent, PsiObservationModel, PsiOptimizationCommit,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, PsiRealizationSite,
    PsiRegionBoundaryEdgeObservation, PsiRegionFrontierObservation, PsiRewriteCandidate,
    PsiRewriteCandidateError, PsiRewriteDecisionPoint, PsiRewritePatch, PsiTransformationLedger,
    PsiTransformationLedgerDecodeError, PsiTransformationRecord, PsiValidatedCandidateDeclaration,
    RedundantBlockParameterRewrite, RedundantBlockParameterWitness, ScalarConstantValue,
    ScalarEvaluationWitness, ScalarSubstitution, SccpBlockRow, SccpEdgeRow, SccpEdgeState,
    SccpMachineSnapshot, SccpValueRow, SccpValueState, SharedJumpFusionRewrite,
    SpecializedStateEdgeRow, StateArgumentSpecializationRewrite, TotalScalarIdentityKind,
    TotalScalarIdentityRewrite, UnreachablePrivateMachinesRewrite, ValueDefinition,
    ValueDefinitionSite, ValueRangeFact, ValueRangeRegion, ValueRangeScope, ValueRangeSupport,
    ValueUse, accepted_obligation_fact_identity, attach_accepted_obligation_facts,
    attach_ownership_frontier_facts, attach_proof_questions, canonical_ownership_frontier_snapshot,
    derived_sccp_scalar_constant_fact_identity, encode_value_definition_site_identity,
    literal_scalar_constant_fact_identity, ownership_frontier_fact_identity,
    proof_question_identity, recompute_psi_optimization_unit_identity,
    reconstruct_psi_closed_region_observation, reconstruct_psi_observation_model,
    reconstruct_psi_optimization_unit_seed, restamp_psi_function_derived_metadata,
    structural_domain_catalog_identity, value_range_fact_identity,
};
