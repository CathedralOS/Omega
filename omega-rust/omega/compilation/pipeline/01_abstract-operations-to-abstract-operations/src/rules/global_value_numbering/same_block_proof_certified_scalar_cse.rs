//! Proof-certified same-block common-subexpression elimination.

use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use super::expression_keys::proof_certified_scalar_expression;
use crate::rules::global_value_numbering::same_block_contract;
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SameBlockProofCertifiedScalarCseRule;

impl SameBlockProofCertifiedScalarCseRule {
    pub fn contract() -> OptimizationRuleContract {
        same_block_contract(
            b"omega.psi-rule.same-block-proof-certified-total-scalar-cse.v1",
            OptimizationSafetyClass::ProofCertified,
        )
    }
}

impl PsiOptimizationRule for SameBlockProofCertifiedScalarCseRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        super::propose_same_block_scalar_cse(
            unit,
            analyses,
            true,
            |_| (),
            |_, operation| proof_certified_scalar_expression(operation),
            |same| {
                PsiRewriteCandidate::new_proof_certified_local_scalar_common_subexpression(
                    unit.identity,
                    Self::contract(),
                    same.affected_blocks,
                    same.provenance,
                    same.obligation_fact
                        .expect("proof-certified match carries its obligation fact"),
                    -1,
                    same.patch,
                )
                .map_err(RuleProposalError::InvalidCandidate)
            },
        )
    }
}
