//! Block-parameter, control-flow, block-merge, and machine-pruning candidates.

use super::super::*;

impl PsiRewriteCandidate {
    pub fn new_redundant_block_parameter(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        witness: RedundantBlockParameterWitness,
        predicted_cost_delta: i64,
        patch: RedundantBlockParameterRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        if witness.incoming.is_empty() {
            return Err(PsiRewriteCandidateError::EmptyIncomingBindings);
        }
        if witness
            .incoming
            .windows(2)
            .any(|pair| (pair[0].edge, pair[0].source) >= (pair[1].edge, pair[1].source))
        {
            return Err(PsiRewriteCandidateError::NonCanonicalIncomingBindings);
        }
        let substitutions = vec![ScalarSubstitution {
            from: patch.parameter,
            to: patch.replacement,
            scalar_type: patch.scalar_type,
        }];
        Self::new(
            input,
            contract,
            affected_blocks,
            substitutions,
            provenance,
            PsiRewriteWitness::RedundantBlockParameter(witness),
            predicted_cost_delta,
            PsiRewritePatch::RemoveRedundantBlockParameter(patch),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_constant_conditional(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        condition_fact: ScalarConstantFactIdentity,
        predicted_cost_delta: i64,
        patch: ConstantConditionalRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            Vec::new(),
            provenance,
            PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::Unary {
                operand_fact: condition_fact,
            }),
            predicted_cost_delta,
            PsiRewritePatch::FoldConstantConditional(patch),
        )
    }

    pub fn new_linear_empty_block(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        predicted_cost_delta: i64,
        patch: LinearEmptyBlockRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            Vec::new(),
            provenance,
            PsiRewriteWitness::StructuralIdentity,
            predicted_cost_delta,
            PsiRewritePatch::ThreadLinearEmptyBlock(patch),
        )
    }

    pub fn new_path_qualified_empty_block(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        predicted_cost_delta: i64,
        patch: PathQualifiedEmptyBlockRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            Vec::new(),
            provenance,
            PsiRewriteWitness::StructuralIdentity,
            predicted_cost_delta,
            PsiRewritePatch::ThreadPathQualifiedEmptyBlock(patch),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_adjacent_block_merge(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        substitutions: Vec<ScalarSubstitution>,
        provenance: Vec<ProvenanceRewrite>,
        ownership_witness: OwnershipFrontierWitness,
        predicted_cost_delta: i64,
        patch: AdjacentBlockMergeRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        if ownership_witness
            .rows
            .windows(2)
            .any(|pair| pair[0].site >= pair[1].site)
        {
            return Err(PsiRewriteCandidateError::NonCanonicalOwnershipFrontierWitness);
        }
        Self::new(
            input,
            contract,
            affected_blocks,
            substitutions,
            provenance,
            PsiRewriteWitness::OwnershipFrontiers(ownership_witness),
            predicted_cost_delta,
            PsiRewritePatch::MergeAdjacentBlock(patch),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_non_adjacent_block_merge(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        substitutions: Vec<ScalarSubstitution>,
        provenance: Vec<ProvenanceRewrite>,
        predicted_cost_delta: i64,
        patch: NonAdjacentBlockMergeRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            substitutions,
            provenance,
            PsiRewriteWitness::StructuralIdentity,
            predicted_cost_delta,
            PsiRewritePatch::MergeNonAdjacentBlock(patch),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_shared_jump_fusion(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        substitutions: Vec<ScalarSubstitution>,
        provenance: Vec<ProvenanceRewrite>,
        predicted_cost_delta: i64,
        patch: SharedJumpFusionRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            substitutions,
            provenance,
            PsiRewriteWitness::StructuralIdentity,
            predicted_cost_delta,
            PsiRewritePatch::FuseSharedTerminalJump(patch),
        )
    }

    pub fn new_unreachable_private_machines(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        provenance: Vec<ProvenanceRewrite>,
        predicted_cost_delta: i64,
        patch: UnreachablePrivateMachinesRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            Vec::new(),
            Vec::new(),
            provenance,
            PsiRewriteWitness::StructuralIdentity,
            predicted_cost_delta,
            PsiRewritePatch::PruneUnreachablePrivateMachines(patch),
        )
    }
}
