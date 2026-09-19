//! Immutable optimization-unit aggregate and its exact carrier-family map.
//!
//! The one program root for the representation: the aggregate struct below
//! defines the current unit, and every concept area — construction,
//! identity, ledger, observation, evidence, rewrite vocabulary, and tests —
//! is subordinate to it.

use super::{
    AbstractFunction, AbstractOperation, AbstractOperationPlan, AbstractSuccessor, Arc, BTreeSet,
    BlockId, BoundaryMachineDeclaration, FuelScheduleIdentity, MachineId, ObligationId,
    OperationId, OptimizationUnitIdentity, PlaceId, ProviderCandidateConformance, ScalarType,
    ServiceDeclaration, StructuralDomainDeclaration, StructuralPlaceDeclaration,
    StructuralPlaceKind, TerminalPsiIdentity, TerminalRootServiceReach, ValueId,
};
mod attachment;
mod construction;
mod cycles;
mod evidence;
mod graph;
mod identity;
mod ledger;
mod manifest;
mod observation;
mod ownership;
mod proof;
mod range;
mod rewrite;
#[cfg(test)]
mod tests;

pub use attachment::*;
pub use construction::{OptimizationUnitBuildError, reconstruct_psi_optimization_unit_seed};
pub use cycles::*;
pub use evidence::*;
pub use graph::*;
pub use identity::{recompute_psi_optimization_unit_identity, structural_domain_catalog_identity};
pub use ledger::{
    InvalidPsiTransformationLedger, PrunedMachineCustody, PsiTransformationLedger,
    PsiTransformationLedgerDecodeError, PsiTransformationRecord,
};
pub use manifest::*;
pub use observation::{
    ObservationEventClass, ObservationKnowledge, PsiClosedRegionBlockObservation,
    PsiClosedRegionObservation, PsiClosedRegionSemantics, PsiNodeObservation, PsiObservableEvent,
    PsiObservationModel, PsiRegionBoundaryEdgeObservation, PsiRegionFrontierObservation,
    reconstruct_psi_closed_region_observation, reconstruct_psi_observation_model,
};
pub use ownership::*;
pub use proof::*;
pub use range::*;
pub use rewrite::{
    AdjacentBlockMergeRewrite, BlockParameterIncomingBinding, BooleanConstantRewrite,
    ConstantConditionalRewrite, DeadScalarNodeRewrite, DominatingScalarCommonSubexpressionRewrite,
    IntegerConstantRewrite, IntegerEvaluationWitness, LinearEmptyBlockRewrite,
    LocalScalarCommonSubexpressionRewrite, NodeLocation, NonAdjacentBlockMergeRewrite,
    OwnershipFrontierWitness, OwnershipFrontierWitnessRow, PathQualifiedEmptyBlockRewrite,
    PhiTranslatedScalarGvnRewrite, PhiTranslatedScalarIncoming, ProofCertifiedScalarIdentityKind,
    ProofCertifiedScalarIdentityRewrite, ProvenanceDisposition, ProvenanceRewrite,
    PsiRealizationSite, PsiRewriteCandidate, PsiRewriteCandidateError, PsiRewriteDecisionPoint,
    PsiRewritePatch, RedundantBlockParameterRewrite, RedundantBlockParameterWitness,
    ScalarConstantValue, ScalarEvaluationWitness, ScalarSubstitution, SccpBlockRow, SccpEdgeRow,
    SccpEdgeState, SccpMachineSnapshot, SccpValueRow, SccpValueState, SharedJumpFusionRewrite,
    TotalScalarIdentityKind, TotalScalarIdentityRewrite, UnreachablePrivateMachinesRewrite,
    derived_sccp_scalar_constant_fact_identity, literal_scalar_constant_fact_identity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiOptimizationUnit {
    pub identity: OptimizationUnitIdentity,
    pub psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub entry: MachineId,
    /// Target-neutral module declarations needed by layout, ABI, and checked
    /// provider installation after the full Terminal module is discarded.
    pub structural_types: abstract_operations::StructuralTypeCatalog,
    /// Exact verifier-owned qualification-domain catalog. Bare lowering seeds
    /// leave this empty; optimizer admission attaches it before rewrites run.
    pub structural_domains: Arc<[StructuralDomainDeclaration]>,
    /// Exact verifier-owned boundary-service hierarchy. Bare lowering seeds
    /// leave this empty; optimizer admission attaches the complete catalog so
    /// call reach and concrete service effects remain independently replayable.
    pub services: Arc<[ServiceDeclaration]>,
    /// Exact current-revision closure of services executable from `entry`.
    /// Unlike declaration custody, this derived row may narrow when a checked
    /// rewrite removes an unreachable call or concrete service effect.
    pub root_service_reach: TerminalRootServiceReach,
    pub boundary_machines: Vec<BoundaryMachineDeclaration>,
    pub provider_candidates: Vec<ProviderCandidateConformance>,
    pub accepted_obligation_facts: Vec<AcceptedObligationFact>,
    /// Complete immutable verifier proof-question roster in reconstruction
    /// order. This is source-site authority, not a function-wide range index.
    pub proof_questions: Vec<ProofQuestion>,
    /// Immutable verifier projection, absent only on low-level bare seeds that
    /// are not authorized optimizer inputs.
    pub ownership_frontier_facts: Vec<OwnershipFrontierFact>,
    /// Canonical custody for source functions removed by independently proven
    /// whole-program reachability rewrites. Source ordinals bind each removed
    /// machine to the immutable verified Terminal-Psi function roster.
    pub pruned_machines: Vec<PrunedMachineCustody>,
    pub functions: Vec<PsiOptimizationFunction>,
}
