//! Optimizer module role: stage group. Immutable Psi rewrite candidate taxonomy.
//!
//! `model` owns atomic plans and independent witnesses, `candidate` owns
//! construction and invariant-preserving access, `canonical_encoding` owns
//! model-neutral primitive writers, and `codec` owns candidate identity
//! encoding. Callers never observe a partially built plan.

use optimization_core::{
    AcceptedObligationFactIdentity, AnalysisInvalidationSet, AnalysisSet,
    OptimizationCandidateIdentity, OptimizationFactReference, OptimizationRuleContract,
    OptimizationRuleIdentity, OptimizationSafetyClass, OptimizationUnitIdentity,
    OwnershipFrontierFactIdentity, ScalarConstantFactIdentity, ValueRangeFactIdentity,
};
use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, EdgeId, IntegerCarrier, IntegerSign, IntegerType,
    IntegerValue, MachineId, OperationId, PlaceId, ScalarType, StructuralCaseId, StructuralFieldId,
    ValueId,
};

use crate::{
    FuelSettlement, OwnershipFrontierSite, PsiProvenance, ValueDefinition, ValueDefinitionSite,
};

mod candidate;
mod canonical_encoding;
mod codec;
mod model;

use model::PsiRewriteWitness;
pub use model::{
    AdjacentBlockMergeRewrite, BlockParameterIncomingBinding, BooleanConstantRewrite,
    CaseMembershipSpecializationRewrite, ConstantConditionalRewrite, DeadScalarNodeRewrite,
    DominatingScalarCommonSubexpressionRewrite, FieldValueResolution, FieldValueRow,
    FieldValueSpecializationRewrite, FoldedCaseMembershipRow, FoldedFieldValue,
    ForwardedFieldValue, IntegerConstantRewrite, IntegerEvaluationWitness, LinearEmptyBlockRewrite,
    LocalScalarCommonSubexpressionRewrite, NodeLocation, NonAdjacentBlockMergeRewrite,
    OwnershipFrontierWitness, OwnershipFrontierWitnessRow, PathQualifiedEmptyBlockRewrite,
    PhiTranslatedScalarGvnRewrite, PhiTranslatedScalarIncoming, ProofCertifiedScalarIdentityKind,
    ProofCertifiedScalarIdentityRewrite, ProvenanceDisposition, ProvenanceRewrite,
    PsiRealizationSite, PsiRewriteCandidate, PsiRewriteCandidateError, PsiRewriteDecisionPoint,
    PsiRewritePatch, RedundantBlockParameterRewrite, RedundantBlockParameterWitness,
    ScalarConstantValue, ScalarEvaluationWitness, ScalarSubstitution, SccpBlockRow, SccpEdgeRow,
    SccpEdgeState, SccpMachineSnapshot, SccpValueRow, SccpValueState, SharedJumpFusionRewrite,
    SpecializedStateEdgeRow, StateArgumentSpecializationRewrite, TotalScalarIdentityKind,
    TotalScalarIdentityRewrite, UnreachablePrivateMachinesRewrite,
    derived_sccp_scalar_constant_fact_identity, literal_scalar_constant_fact_identity,
};

#[cfg(test)]
mod tests;
