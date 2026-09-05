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
    StructuralPlaceDeclaration, StructuralTypeDeclaration, TerminalAffineCleanupAction,
    TerminalPsiIdentity, TerminalRootServiceReach,
};

mod evidence;
mod identity;
pub use evidence::*;
mod ledger;
mod observation;
mod rewrite;

pub use identity::{recompute_psi_optimization_unit_identity, structural_domain_catalog_identity};

pub use ledger::{
    InvalidPsiTransformationLedger, PsiTransformationLedger, PsiTransformationLedgerDecodeError,
    PsiTransformationRecord,
};
pub use observation::{
    ObservationEventClass, ObservationKnowledge, PsiClosedRegionBlockObservation,
    PsiClosedRegionObservation, PsiClosedRegionSemantics, PsiNodeObservation, PsiObservableEvent,
    PsiObservationModel, PsiRegionBoundaryEdgeObservation, PsiRegionFrontierObservation,
    reconstruct_psi_closed_region_observation, reconstruct_psi_observation_model,
};

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

/// The exact immutable Terminal Psi semantic site realized by one unit node.
mod construction;
mod model;

pub use construction::{OptimizationUnitBuildError, reconstruct_psi_optimization_unit_seed};
pub use model::*;

#[cfg(test)]
mod tests;
