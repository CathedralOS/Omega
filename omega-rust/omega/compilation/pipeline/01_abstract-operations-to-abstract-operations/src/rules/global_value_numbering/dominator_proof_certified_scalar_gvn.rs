//! Proof-certified cross-block elimination from dominating leaders.

use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use super::expression_keys::proof_certified_scalar_expression;
use crate::rules::accepted_obligations::accepted_obligation_fact;
use crate::rules::global_value_numbering::dominating_contract;
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DominatorProofCertifiedScalarGvnRule;

impl DominatorProofCertifiedScalarGvnRule {
    pub fn contract() -> OptimizationRuleContract {
        dominating_contract(
            b"omega.psi-rule.dominator-proof-certified-total-scalar-gvn.v1",
            OptimizationSafetyClass::ProofCertified,
        )
    }
}

impl PsiOptimizationRule for DominatorProofCertifiedScalarGvnRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        super::propose_dominating_scalar_gvn(
            unit,
            analyses,
            |_| (),
            |_, unit, machine, operation| {
                let row = proof_certified_scalar_expression(operation)?;
                let fact = accepted_obligation_fact(unit, machine, row.1).ok()?;
                Some((row, fact))
            },
            |same| {
                PsiRewriteCandidate::new_proof_certified_dominating_scalar_common_subexpression(
                    unit.identity,
                    Self::contract(),
                    same.affected_blocks,
                    same.provenance,
                    same.extras,
                    -1,
                    same.patch,
                )
                .map_err(RuleProposalError::InvalidCandidate)
            },
        )
    }
}
