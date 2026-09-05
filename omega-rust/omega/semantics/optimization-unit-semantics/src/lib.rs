#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Independent meaning and rewrite validation for the optimization-unit representation.
//! This crate neither lowers Terminal Psi nor sequences optimization passes or publication.
//! Candidate producers do not participate in acceptance. The caller retains
//! Terminal admission and supplies any independently admitted cycle roster;
//! structural success alone grants no execution or publication authority.

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
    OwnershipEvent, OwnershipFrontierOwnedPlace, OwnershipFrontierSite, OwnershipFrontierSnapshot,
    OwnershipFrontierWitness, OwnershipFrontierWitnessRow, PhiTranslatedScalarGvnRewrite,
    PhiTranslatedScalarIncoming, ProofCertifiedScalarIdentityKind,
    ProofCertifiedScalarIdentityRewrite, ProvenanceDisposition, ProvenanceRewrite,
    PsiNodeObservation, PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
    PsiRealizationSite, PsiRewriteCandidate, PsiRewritePatch, RedundantBlockParameterRewrite,
    ScalarConstantValue, ScalarSubstitution, SccpBlockRow, SccpEdgeRow, SccpEdgeState,
    SccpMachineSnapshot, SccpValueRow, SccpValueState, SharedJumpFusionRewrite,
    TotalScalarIdentityKind, TotalScalarIdentityRewrite, ValueDefinition, ValueDefinitionSite,
    ValueUse, canonical_ownership_frontier_snapshot, derived_sccp_scalar_constant_fact_identity,
    literal_scalar_constant_fact_identity, recompute_psi_optimization_unit_identity,
    reconstruct_psi_closed_region_observation, reconstruct_psi_observation_model,
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
mod unit_validation;

pub use candidates::*;
pub use current_value_ranges::{
    validate_current_value_range_fact, validate_current_value_range_fact_at,
};
pub use error::OptimizationUnitValidationError;
pub(crate) use unit_validation::*;
pub use unit_validation::{
    validate_psi_optimization_unit, validate_psi_optimization_unit_with_admitted_cycle_machines,
};

#[cfg(test)]
mod tests;
