//! Scalar evaluation, elimination, identity, and common-subexpression candidates.

use super::super::*;

impl PsiRewriteCandidate {
    #[allow(clippy::too_many_arguments)]
    pub fn new_integer_evaluation(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        substitutions: Vec<ScalarSubstitution>,
        provenance: Vec<ProvenanceRewrite>,
        witness: ScalarEvaluationWitness,
        predicted_cost_delta: i64,
        patch: IntegerConstantRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            substitutions,
            provenance,
            PsiRewriteWitness::ScalarEvaluation(witness),
            predicted_cost_delta,
            PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch),
        )
    }

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
    pub fn new_boolean_evaluation(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        substitutions: Vec<ScalarSubstitution>,
        provenance: Vec<ProvenanceRewrite>,
        witness: ScalarEvaluationWitness,
        predicted_cost_delta: i64,
        patch: BooleanConstantRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            substitutions,
            provenance,
            PsiRewriteWitness::ScalarEvaluation(witness),
            predicted_cost_delta,
            PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_dead_scalar_node(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        predicted_cost_delta: i64,
        patch: DeadScalarNodeRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            Vec::new(),
            provenance,
            PsiRewriteWitness::StructuralIdentity,
            predicted_cost_delta,
            PsiRewritePatch::RemoveDeadScalarNode(patch),
        )
    }

    pub fn new_proof_certified_dead_scalar_node(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        obligation_fact: AcceptedObligationFactIdentity,
        predicted_cost_delta: i64,
        patch: DeadScalarNodeRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            Vec::new(),
            provenance,
            PsiRewriteWitness::AcceptedObligation(obligation_fact),
            predicted_cost_delta,
            PsiRewritePatch::RemoveDeadScalarNode(patch),
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

    pub fn new_local_scalar_common_subexpression(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        predicted_cost_delta: i64,
        patch: LocalScalarCommonSubexpressionRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            vec![ScalarSubstitution {
                from: patch.redundant_result,
                to: patch.leader_result,
                scalar_type: patch.scalar_type,
            }],
            provenance,
            PsiRewriteWitness::StructuralIdentity,
            predicted_cost_delta,
            PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch),
        )
    }

    pub fn new_proof_certified_local_scalar_common_subexpression(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        obligation_fact: AcceptedObligationFactIdentity,
        predicted_cost_delta: i64,
        patch: LocalScalarCommonSubexpressionRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            vec![ScalarSubstitution {
                from: patch.redundant_result,
                to: patch.leader_result,
                scalar_type: patch.scalar_type,
            }],
            provenance,
            PsiRewriteWitness::AcceptedObligation(obligation_fact),
            predicted_cost_delta,
            PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch),
        )
    }

    pub fn new_dominating_scalar_common_subexpression(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        predicted_cost_delta: i64,
        patch: DominatingScalarCommonSubexpressionRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            vec![ScalarSubstitution {
                from: patch.redundant_result,
                to: patch.leader_result,
                scalar_type: patch.scalar_type,
            }],
            provenance,
            PsiRewriteWitness::StructuralIdentity,
            predicted_cost_delta,
            PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch),
        )
    }

    pub fn new_proof_certified_dominating_scalar_common_subexpression(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        obligation_fact: AcceptedObligationFactIdentity,
        predicted_cost_delta: i64,
        patch: DominatingScalarCommonSubexpressionRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            vec![ScalarSubstitution {
                from: patch.redundant_result,
                to: patch.leader_result,
                scalar_type: patch.scalar_type,
            }],
            provenance,
            PsiRewriteWitness::AcceptedObligation(obligation_fact),
            predicted_cost_delta,
            PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch),
        )
    }

    pub fn new_phi_translated_scalar_common_subexpression(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        predicted_cost_delta: i64,
        patch: PhiTranslatedScalarGvnRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            Vec::new(),
            provenance,
            PsiRewriteWitness::StructuralIdentity,
            predicted_cost_delta,
            PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch),
        )
    }

    pub fn new_proof_certified_phi_translated_scalar_common_subexpression(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        obligation_fact: AcceptedObligationFactIdentity,
        predicted_cost_delta: i64,
        patch: PhiTranslatedScalarGvnRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            Vec::new(),
            provenance,
            PsiRewriteWitness::AcceptedObligation(obligation_fact),
            predicted_cost_delta,
            PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch),
        )
    }
}
