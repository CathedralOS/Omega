//! Block-parameter, control-flow, block-merge, and machine-pruning candidates.

use super::super::{
    AdjacentBlockMergeRewrite, BlockId, CaseMembershipSpecializationRewrite,
    ConstantConditionalRewrite, FieldValueSpecializationRewrite, LinearEmptyBlockRewrite,
    NonAdjacentBlockMergeRewrite, OptimizationRuleContract, OptimizationUnitIdentity,
    OwnershipFrontierWitness, PathQualifiedEmptyBlockRewrite, ProvenanceRewrite,
    PsiRewriteCandidate, PsiRewriteCandidateError, PsiRewritePatch, PsiRewriteWitness,
    RedundantBlockParameterRewrite, RedundantBlockParameterWitness, ScalarConstantFactIdentity,
    ScalarEvaluationWitness, ScalarSubstitution, SharedJumpFusionRewrite,
    StateArgumentSpecializationRewrite, UnreachablePrivateMachinesRewrite,
};
impl PsiRewriteCandidate {
    #[rustfmt::skip]
    structural_identity_constructors![
        ( new_linear_empty_block, LinearEmptyBlockRewrite, ThreadLinearEmptyBlock ),
        ( new_path_qualified_empty_block, PathQualifiedEmptyBlockRewrite, ThreadPathQualifiedEmptyBlock ),
        ( new_state_argument_specialization, StateArgumentSpecializationRewrite, SpecializeStateArgument ),
        ( new_case_membership_specialization, CaseMembershipSpecializationRewrite, SpecializeCaseMembership ),
        ( new_unreachable_private_machines, UnreachablePrivateMachinesRewrite, PruneUnreachablePrivateMachines ),
    ];

    #[rustfmt::skip]
    substituted_structural_identity_constructors![
        ( new_non_adjacent_block_merge, NonAdjacentBlockMergeRewrite, MergeNonAdjacentBlock ),
        ( new_shared_jump_fusion, SharedJumpFusionRewrite, FuseSharedTerminalJump ),
        ( new_field_value_specialization, FieldValueSpecializationRewrite, SpecializeFieldValue ),
    ];

    #[rustfmt::skip]
    condition_fact_constructors![( new_constant_conditional, ConstantConditionalRewrite, FoldConstantConditional ),];

    #[allow(clippy::too_many_arguments)]
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
}
