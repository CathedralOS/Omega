#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Independent structural validation for [`PsiOptimizationUnit`].
//!
//! Pass implementations do not participate in this validator. Publication
//! must call it after applying a candidate and before committing the candidate
//! to the durable transformation ledger.

use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::AbstractOperation as O;
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationCandidateIdentity,
    OptimizationRuleIdentity, OptimizationSafetyClass, OptimizationUnitIdentity,
    OptimizationValidatorIdentity,
};
use optimization_unit::{
    AdjacentBlockMergeRewrite, BlockParameterIncomingBinding, BooleanConstantRewrite,
    ConstantConditionalRewrite, DeadScalarNodeRewrite, IntegerConstantRewrite,
    IntegerEvaluationWitness, LocalScalarCommonSubexpressionRewrite, NodeLocation,
    NonAdjacentBlockMergeRewrite, ObservationKnowledge, OptimizationEdge, OptimizationFact,
    OwnershipEvent, OwnershipFrontierFact, OwnershipFrontierLiveClaim, OwnershipFrontierOwnedPlace,
    OwnershipFrontierPartialCustody, OwnershipFrontierSite, OwnershipFrontierSnapshot,
    OwnershipFrontierWitness, OwnershipFrontierWitnessRow, PhiTranslatedScalarGvnRewrite,
    PhiTranslatedScalarIncoming, ProofCertifiedScalarIdentityKind,
    ProofCertifiedScalarIdentityRewrite, ProofQuestion, ProofQuestionAdmissionKind,
    ProofQuestionClass, ProofQuestionOwner, ProvenanceDisposition, ProvenanceRewrite,
    PsiNodeObservation, PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
    PsiRealizationSite, PsiRewriteCandidate, PsiRewritePatch, RedundantBlockParameterRewrite,
    ScalarConstantValue, ScalarSubstitution, SccpBlockRow, SccpEdgeRow, SccpEdgeState,
    SccpMachineSnapshot, SccpValueRow, SccpValueState, SharedJumpFusionRewrite,
    TotalScalarIdentityKind, TotalScalarIdentityRewrite, ValueDefinition, ValueDefinitionSite,
    ValueUse, canonical_ownership_frontier_snapshot, derived_sccp_scalar_constant_fact_identity,
    literal_scalar_constant_fact_identity, recompute_psi_optimization_unit_identity,
    reconstruct_psi_closed_region_observation, reconstruct_psi_observation_model,
    structural_domain_catalog_identity,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, ContentProjectionExpression, ContentProjectionScalar,
    ContentTerm, EdgeId, IntegerCarrier, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, PlaceId, Proposition, ScalarTerm, ScalarType, ServiceId, StructuralDomainId,
    StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_fuel::TerminalFuelSchedule;

mod candidates;
mod current_ownership;
mod current_value_ranges;
mod error;
mod prephysical_manifest;
mod projection;
mod unit_validation;

pub use candidates::*;
pub use error::OptimizationUnitValidationError;
pub(crate) use unit_validation::*;
pub use unit_validation::{
    CycleComponentEdge, CycleComponentId, OptimizerCycleComponent, OptimizerCycleComponentSnapshot,
    OptimizerRankingCertificateSnapshot, OptimizerUnsignedCountdownRankingCertificate,
    OptimizerUnsignedMinusOneDescent, OptimizerUnsignedPositiveGuard,
    ValidatedOptimizerCycleComponents, ValidatedOptimizerRankingCertificates,
    validate_psi_cycle_component_snapshot, validate_psi_optimization_unit,
    validate_psi_ranking_certificate_snapshot, validate_transformed_psi_cycle_components,
    validate_transformed_psi_optimization_unit, validate_verified_psi_cycle_components,
    validate_verified_psi_optimization_unit,
};

pub use current_value_ranges::{
    validate_current_value_range_fact, validate_current_value_range_fact_at,
};

pub use prephysical_manifest::{
    OptimizationManifestStage, OptimizationStructuralStatistics, PhysicalOptimizationDataStatus,
    PrePhysicalOptimizationManifest, PrePhysicalOptimizationManifestDecodeError,
    PrePhysicalOptimizationManifestError, ValidatedPrePhysicalOptimizationManifest,
    project_pre_physical_optimization_manifest, validate_pre_physical_optimization_manifest,
};
pub use projection::{
    OptimizedAbstractPlanProjectionError, ValidatedOptimizedAbstractPlanProjection,
    validate_optimized_abstract_plan_projection,
};

#[cfg(test)]
mod tests;
