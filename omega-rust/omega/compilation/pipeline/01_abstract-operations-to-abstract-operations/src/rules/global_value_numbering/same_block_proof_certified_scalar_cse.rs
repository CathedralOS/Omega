//! Proof-certified same-block common-subexpression elimination.

use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use super::expression_keys::proof_certified_scalar_expression;
use crate::rules::accepted_obligations::accepted_obligation_fact;
use crate::rules::global_value_numbering::same_block_contract;
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    SameBlockProofCertifiedScalarCseRule,
    same_block_contract(
        b"omega.psi-rule.same-block-proof-certified-total-scalar-cse.v1",
        OptimizationSafetyClass::ProofCertified,
    ),
);

impl PsiOptimizationRule for SameBlockProofCertifiedScalarCseRule {
    psi_rule_contract!(@adapter);

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        super::propose_same_block_scalar_cse(
            unit,
            analyses,
            |_| (),
            |_, unit, function, operation| {
                let row = proof_certified_scalar_expression(operation)?;
                accepted_obligation_fact(unit, function.machine, row.1).ok()?;
                Some(row)
            },
            |_, unit, function, operation| {
                let row = proof_certified_scalar_expression(operation)?;
                let fact = accepted_obligation_fact(unit, function.machine, row.1).ok()?;
                Some((row, fact))
            },
            |same| {
                PsiRewriteCandidate::new_proof_certified_local_scalar_common_subexpression(
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
