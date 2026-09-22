//! Optimizer module role: stage group. Semantic vocabulary for immutable Psi rewrite plans, witnesses, identities, and candidate contracts.
//!
//! Foundations define source locations, provenance, and substitutions. Scalar
//! evaluation and SCCP own constant evidence; CFG and scalar plans name exact
//! mutations; contracts bind those plans into independently validated candidates.

mod cfg_rewrite_plans;
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
pub(super) use contracts::PsiRewriteWitness;
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
