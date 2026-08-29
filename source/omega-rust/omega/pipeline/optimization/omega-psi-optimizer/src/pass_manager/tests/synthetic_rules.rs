//! Synthetic rules used to exercise pass-manager failure and policy paths.

use super::*;

#[derive(Debug)]
pub(super) struct NonProfitableExactRule;

impl PsiOptimizationRule for NonProfitableExactRule {
    fn contract(&self) -> omega_optimization_core::OptimizationRuleContract {
        ExactIntegerAddConstantsRule::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<omega_optimization_unit::PsiRewriteCandidate>, RuleProposalError> {
        ExactIntegerAddConstantsRule
                .propose(unit, analyses)?
                .into_iter()
                .map(|candidate| {
                    let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) =
                        candidate.patch()
                    else {
                        return Err(RuleProposalError::InvalidCandidate(
                            omega_optimization_unit::PsiRewriteCandidateError::PatchDecisionPointMismatch,
                        ));
                    };
                    omega_optimization_unit::PsiRewriteCandidate::new_integer_evaluation(
                        candidate.input(),
                        Self.contract(),
                        candidate.affected_blocks().to_vec(),
                        candidate.substitutions().to_vec(),
                        candidate.provenance().to_vec(),
                        candidate.scalar_evaluation_witness().unwrap(),
                        0,
                        patch,
                    )
                    .map_err(RuleProposalError::InvalidCandidate)
                })
                .collect()
    }
}

#[derive(Debug)]
pub(super) struct DuplicateExactRule;

impl PsiOptimizationRule for DuplicateExactRule {
    fn contract(&self) -> omega_optimization_core::OptimizationRuleContract {
        ExactIntegerAddConstantsRule::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<omega_optimization_unit::PsiRewriteCandidate>, RuleProposalError> {
        let mut candidates = ExactIntegerAddConstantsRule.propose(unit, analyses)?;
        candidates.push(candidates[0].clone());
        Ok(candidates)
    }
}

#[derive(Debug)]
pub(super) struct InvalidEvaluationExactRule;

impl PsiOptimizationRule for InvalidEvaluationExactRule {
    fn contract(&self) -> omega_optimization_core::OptimizationRuleContract {
        ExactIntegerAddConstantsRule::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<omega_optimization_unit::PsiRewriteCandidate>, RuleProposalError> {
        ExactIntegerAddConstantsRule
                .propose(unit, analyses)?
                .into_iter()
                .map(|candidate| {
                    let PsiRewritePatch::ReplaceIntegerOperationWithConstant(mut patch) =
                        candidate.patch()
                    else {
                        return Err(RuleProposalError::InvalidCandidate(
                            omega_optimization_unit::PsiRewriteCandidateError::PatchDecisionPointMismatch,
                        ));
                    };
                    patch.constant = psi_core::IntegerValue::Unsigned(0);
                    omega_optimization_unit::PsiRewriteCandidate::new_integer_evaluation(
                        candidate.input(),
                        Self.contract(),
                        candidate.affected_blocks().to_vec(),
                        candidate.substitutions().to_vec(),
                        candidate.provenance().to_vec(),
                        candidate.scalar_evaluation_witness().unwrap(),
                        candidate.predicted_cost_delta(),
                        patch,
                    )
                    .map_err(RuleProposalError::InvalidCandidate)
                })
                .collect()
    }
}
