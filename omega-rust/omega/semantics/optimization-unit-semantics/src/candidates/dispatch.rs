//! Typed candidate dispatch to exact independent validators.
use crate::OptimizationUnitValidationError;
use crate::PsiOptimizationUnit;
use crate::PsiRewriteCandidate;
use crate::PsiRewritePatch;
use crate::ValidatedPsiRewrite;
use crate::is_proof_check_elision_rule;
use crate::validate_adjacent_block_merge_candidate;
use crate::validate_case_membership_specialization_candidate;
use crate::validate_constant_conditional_candidate;
use crate::validate_dead_scalar_node_candidate;
use crate::validate_dominating_scalar_common_subexpression_candidate;
use crate::validate_field_value_specialization_candidate;
use crate::validate_linear_empty_block_candidate;
use crate::validate_local_scalar_common_subexpression_candidate;
use crate::validate_non_adjacent_block_merge_candidate;
use crate::validate_path_qualified_empty_block_candidate;
use crate::validate_phi_translated_scalar_common_subexpression_candidate;
use crate::validate_proof_check_elision_candidate;
use crate::validate_redundant_block_parameter_candidate;
use crate::validate_scalar_evaluation_candidate;
use crate::validate_shared_jump_fusion_candidate;
use crate::validate_state_argument_specialization_candidate;
use crate::validate_total_scalar_identity_candidate;
use crate::validate_unreachable_private_machines_candidate;

pub fn validate_psi_rewrite_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    if is_proof_check_elision_rule(candidate.rule()) {
        return validate_proof_check_elision_candidate(input, candidate);
    }
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
            validate_proof_check_elision_candidate(input, candidate)
        }
        PsiRewritePatch::EliminateTotalScalarIdentity(_) => {
            validate_total_scalar_identity_candidate(input, candidate)
        }
        PsiRewritePatch::PruneUnreachablePrivateMachines(_) => {
            validate_unreachable_private_machines_candidate(input, candidate)
        }
        PsiRewritePatch::SpecializeStateArgument(_) => {
            validate_state_argument_specialization_candidate(input, candidate)
        }
        PsiRewritePatch::SpecializeCaseMembership(_) => {
            validate_case_membership_specialization_candidate(input, candidate)
        }
        PsiRewritePatch::SpecializeFieldValue(_) => {
            validate_field_value_specialization_candidate(input, candidate)
        }
    }
}
