//! Canonical scheduling coordinate derived from the typed patch family.

use super::super::*;

pub(super) fn derive(
    patch: &PsiRewritePatch,
) -> Result<PsiRewriteDecisionPoint, PsiRewriteCandidateError> {
    let decision_point = match &patch {
        PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) => {
            PsiRewriteDecisionPoint::Node(patch.location)
        }
        PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch) => {
            PsiRewriteDecisionPoint::Node(patch.location)
        }
        PsiRewritePatch::RemoveRedundantBlockParameter(patch) => {
            PsiRewriteDecisionPoint::Node(NodeLocation {
                machine: patch.machine,
                block: patch.block,
                node: 0,
            })
        }
        PsiRewritePatch::FoldConstantConditional(patch) => {
            PsiRewriteDecisionPoint::Node(patch.location)
        }
        PsiRewritePatch::ThreadLinearEmptyBlock(patch) => {
            PsiRewriteDecisionPoint::Node(patch.predecessor)
        }
        PsiRewritePatch::ThreadPathQualifiedEmptyBlock(patch) => {
            PsiRewriteDecisionPoint::Node(patch.empty)
        }
        PsiRewritePatch::MergeAdjacentBlock(patch) => {
            PsiRewriteDecisionPoint::Node(patch.predecessor)
        }
        PsiRewritePatch::MergeNonAdjacentBlock(patch) => {
            PsiRewriteDecisionPoint::Node(patch.predecessor)
        }
        PsiRewritePatch::FuseSharedTerminalJump(patch) => {
            PsiRewriteDecisionPoint::Node(patch.predecessor)
        }
        PsiRewritePatch::RemoveDeadScalarNode(patch) => {
            PsiRewriteDecisionPoint::Node(patch.location)
        }
        PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch) => {
            PsiRewriteDecisionPoint::Node(patch.redundant)
        }
        PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) => {
            PsiRewriteDecisionPoint::Node(patch.redundant)
        }
        PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) => {
            PsiRewriteDecisionPoint::Node(patch.redundant)
        }
        PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) => {
            PsiRewriteDecisionPoint::Node(patch.location)
        }
        PsiRewritePatch::PruneUnreachablePrivateMachines(patch) => {
            if patch.machines.is_empty() || patch.machines.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(PsiRewriteCandidateError::NonCanonicalAffectedRegion);
            }
            let machines = patch
                .machines
                .iter()
                .map(|row| row.machine)
                .collect::<Vec<_>>();
            if machines.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(PsiRewriteCandidateError::NonCanonicalAffectedRegion);
            }
            PsiRewriteDecisionPoint::MachineSet(machines)
        }
    };
    Ok(decision_point)
}
