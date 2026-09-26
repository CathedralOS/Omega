//! Optimizer module role: stage group. Immutable Psi rewrite candidate taxonomy.
//!
//! Atomic plans and their independent witnesses come first: `foundations`
//! defines source locations, provenance and substitutions; `scalar_evaluation`
//! and `sccp` own constant evidence; `cfg_rewrite_plans` and
//! `scalar_rewrite_plans` name exact mutations; `contracts` binds those plans
//! into independently validated candidates. `candidate` owns construction and
//! invariant-preserving access, `canonical_encoding` owns the primitive
//! writers, and `codec` owns candidate identity encoding. Callers never
//! observe a partially built plan.

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
mod cfg_rewrite_plans;
mod codec;
mod contracts;
mod foundations;
mod scalar_evaluation;
mod scalar_rewrite_plans;
mod sccp;

pub use cfg_rewrite_plans::{
    AdjacentBlockMergeRewrite, BlockParameterIncomingBinding, CaseMembershipSpecializationRewrite,
    ConstantConditionalRewrite, FieldValueResolution, FieldValueRow,
    FieldValueSpecializationRewrite, FoldedCaseMembershipRow, FoldedFieldValue,
    ForwardedFieldValue, LinearEmptyBlockRewrite, NonAdjacentBlockMergeRewrite,
    OwnershipFrontierWitness, OwnershipFrontierWitnessRow, PathQualifiedEmptyBlockRewrite,
    RedundantBlockParameterRewrite, RedundantBlockParameterWitness, SharedJumpFusionRewrite,
    SpecializedStateEdgeRow, StateArgumentSpecializationRewrite, UnreachablePrivateMachinesRewrite,
};
use contracts::PsiRewriteWitness;
pub use contracts::{
    PsiRewriteCandidate, PsiRewriteCandidateError, PsiRewriteDecisionPoint, PsiRewritePatch,
};
pub use foundations::{
    NodeLocation, ProvenanceDisposition, ProvenanceRewrite, PsiRealizationSite, ScalarSubstitution,
};
pub use scalar_evaluation::{
    IntegerEvaluationWitness, ScalarConstantValue, ScalarEvaluationWitness,
    literal_scalar_constant_fact_identity,
};
pub use scalar_rewrite_plans::{
    BooleanConstantRewrite, DeadScalarNodeRewrite, DominatingScalarCommonSubexpressionRewrite,
    IntegerConstantRewrite, LocalScalarCommonSubexpressionRewrite, PhiTranslatedScalarGvnRewrite,
    PhiTranslatedScalarIncoming, ProofCertifiedScalarIdentityKind,
    ProofCertifiedScalarIdentityRewrite, TotalScalarIdentityKind, TotalScalarIdentityRewrite,
};
pub use sccp::{
    SccpBlockRow, SccpEdgeRow, SccpEdgeState, SccpMachineSnapshot, SccpValueRow, SccpValueState,
    derived_sccp_scalar_constant_fact_identity,
};

#[cfg(test)]
mod tests;
