//! Typed candidate dispatch to exact independent validators.

use super::*;

pub fn validate_psi_rewrite_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    match candidate.patch() {
        PsiRewritePatch::ReplaceIntegerOperationWithConstant(_)
        | PsiRewritePatch::ReplaceBooleanOperationWithConstant(_) => {
            validate_scalar_evaluation_candidate(input, candidate)
        }
        PsiRewritePatch::RemoveRedundantBlockParameter(_) => {
            validate_redundant_block_parameter_candidate(input, candidate)
        }
        PsiRewritePatch::FoldConstantConditional(_) => {
            validate_constant_conditional_candidate(input, candidate)
        }
        PsiRewritePatch::ThreadLinearEmptyBlock(_) => {
            validate_linear_empty_block_candidate(input, candidate)
        }
        PsiRewritePatch::ThreadPathQualifiedEmptyBlock(_) => {
            validate_path_qualified_empty_block_candidate(input, candidate)
        }
        PsiRewritePatch::MergeAdjacentBlock(_) => {
            validate_adjacent_block_merge_candidate(input, candidate)
        }
        PsiRewritePatch::MergeNonAdjacentBlock(_) => {
            validate_non_adjacent_block_merge_candidate(input, candidate)
        }
        PsiRewritePatch::FuseSharedTerminalJump(_) => {
            validate_shared_jump_fusion_candidate(input, candidate)
        }
        PsiRewritePatch::RemoveDeadScalarNode(_) => {
            validate_dead_scalar_node_candidate(input, candidate)
        }
        PsiRewritePatch::EliminateLocalScalarCommonSubexpression(_) => {
            validate_local_scalar_common_subexpression_candidate(input, candidate)
        }
        PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(_) => {
            validate_dominating_scalar_common_subexpression_candidate(input, candidate)
        }
        PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(_) => {
            validate_phi_translated_scalar_common_subexpression_candidate(input, candidate)
        }
        PsiRewritePatch::EliminateProofCertifiedScalarIdentity(_) => {
            validate_proof_certified_scalar_identity_candidate(input, candidate)
        }
        PsiRewritePatch::PruneUnreachablePrivateMachines(_) => {
            validate_unreachable_private_machines_candidate(input, candidate)
        }
    }
}
