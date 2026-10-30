//! Scalar evaluation, elimination, identity, and common-subexpression candidates.

use super::super::{
    AcceptedObligationFactIdentity, BlockId, BooleanConstantRewrite, DeadScalarNodeRewrite,
    DominatingScalarCommonSubexpressionRewrite, IntegerConstantRewrite,
    LocalScalarCommonSubexpressionRewrite, OptimizationRuleContract, OptimizationUnitIdentity,
    PhiTranslatedScalarGvnRewrite, ProofCertifiedScalarIdentityRewrite, ProvenanceRewrite,
    PsiRewriteCandidate, PsiRewriteCandidateError, PsiRewritePatch, PsiRewriteWitness,
    ScalarConstantFactIdentity, ScalarEvaluationWitness, ScalarSubstitution, ScalarType,
    TotalScalarIdentityRewrite,
};
impl PsiRewriteCandidate {
    #[rustfmt::skip]
    structural_identity_constructors![
        ( new_dead_scalar_node, DeadScalarNodeRewrite, RemoveDeadScalarNode ),
        ( new_phi_translated_scalar_common_subexpression, PhiTranslatedScalarGvnRewrite, EliminatePhiTranslatedScalarCommonSubexpression ),
    ];

    #[rustfmt::skip]
    scalar_evaluation_constructors![
        ( new_integer_evaluation, IntegerConstantRewrite, ReplaceIntegerOperationWithConstant ),
        ( new_boolean_evaluation, BooleanConstantRewrite, ReplaceBooleanOperationWithConstant ),
    ];

    #[rustfmt::skip]
    accepted_obligation_constructors![
        ( new_proof_certified_dead_scalar_node, DeadScalarNodeRewrite, RemoveDeadScalarNode ),
        ( new_proof_certified_phi_translated_scalar_common_subexpression, PhiTranslatedScalarGvnRewrite, EliminatePhiTranslatedScalarCommonSubexpression ),
    ];

    #[rustfmt::skip]
    redundant_result_pair_constructors![
        (new_local_scalar_common_subexpression, new_proof_certified_local_scalar_common_subexpression, LocalScalarCommonSubexpressionRewrite, EliminateLocalScalarCommonSubexpression, redundant_result, leader_result, scalar_type),
        (new_dominating_scalar_common_subexpression, new_proof_certified_dominating_scalar_common_subexpression, DominatingScalarCommonSubexpressionRewrite, EliminateDominatedScalarCommonSubexpression, redundant_result, leader_result, scalar_type),
    ];

    /// Replace one proof-certified integer operation with an independently
    /// equivalent typed constant while preserving its result and source
    /// occurrence in place. Unlike scalar constant evaluation, the witness is
    /// the exact accepted obligation alone; the rule-specific validator must
    /// reconstruct the symbolic law that determines `patch.constant`.
    #[allow(clippy::too_many_arguments)]
    pub fn new_proof_certified_integer_constant_replacement(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        obligation_fact: AcceptedObligationFactIdentity,
        predicted_cost_delta: i64,
        patch: IntegerConstantRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            Vec::new(),
            provenance,
            PsiRewriteWitness::AcceptedObligation(obligation_fact),
            predicted_cost_delta,
            PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch),
        )
    }

    /// Replace one proof-certified integer operation with an independently
    /// equivalent typed constant when the symbolic law also depends on one
    /// direct scalar-literal fact. The operation stays at its authored site,
    /// so no scalar substitution is introduced.
    #[allow(clippy::too_many_arguments)]
    pub fn new_literal_proof_certified_integer_constant_replacement(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        constant_fact: ScalarConstantFactIdentity,
        obligation_fact: AcceptedObligationFactIdentity,
        predicted_cost_delta: i64,
        patch: IntegerConstantRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            Vec::new(),
            provenance,
            PsiRewriteWitness::ProofCertifiedScalarIdentity {
                constant_fact,
                obligation_fact,
            },
            predicted_cost_delta,
            PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_proof_certified_scalar_identity(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        constant_fact: ScalarConstantFactIdentity,
        obligation_fact: AcceptedObligationFactIdentity,
        predicted_cost_delta: i64,
        patch: ProofCertifiedScalarIdentityRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            vec![ScalarSubstitution {
                from: patch.result,
                to: patch.replacement,
                scalar_type: ScalarType::Integer(patch.scalar_type),
            }],
            provenance,
            PsiRewriteWitness::ProofCertifiedScalarIdentity {
                constant_fact,
                obligation_fact,
            },
            predicted_cost_delta,
            PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch),
        )
    }

    /// Eliminate one wrapping scalar law whose defining operand is
    /// independently bound to a literal fact. No proof obligation is needed:
    /// the closed wrapping laws are total over their declared integer types.
    #[allow(clippy::too_many_arguments)]
    pub fn new_total_scalar_identity(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        constant_fact: ScalarConstantFactIdentity,
        predicted_cost_delta: i64,
        patch: TotalScalarIdentityRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            vec![ScalarSubstitution {
                from: patch.result,
                to: patch.replacement,
                scalar_type: ScalarType::Integer(patch.scalar_type),
            }],
            provenance,
            PsiRewriteWitness::TotalScalarIdentity { constant_fact },
            predicted_cost_delta,
            PsiRewritePatch::EliminateTotalScalarIdentity(patch),
        )
    }
}
