//! Same-block elimination from proof-bearing redundants to total-policy leaders.

use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{OptimizationFact, PsiOptimizationUnit, PsiRewriteCandidate};

use super::expression_keys::{compatible_policy_scalar_leader, compatible_policy_scalar_redundant};
use crate::rules::accepted_obligations::accepted_obligation_fact;
use crate::rules::global_value_numbering::same_block_contract;
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    SameBlockProofCertifiedCompatiblePolicyScalarCseRule,
    same_block_contract(
        b"omega.psi-rule.same-block-proof-certified-compatible-policy-scalar-cse.v1",
        OptimizationSafetyClass::ProofCertified,
    ),
);

impl PsiOptimizationRule for SameBlockProofCertifiedCompatiblePolicyScalarCseRule {
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
            |_, _, function, operation| {
                let row = compatible_policy_scalar_leader(operation)?;
                if function.facts.iter().any(|fact| {
                    matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                        if *support == row.1)
                }) {
                    return None;
                }
                Some(row)
            },
            |_, unit, function, operation| {
                let row = compatible_policy_scalar_redundant(operation)?;
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
