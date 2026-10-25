//! Proof-certified translation from exact redundants to total-policy leaders.

use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::rules::global_value_numbering::collect_compatible_policy_scalar_rows;
use crate::rules::global_value_numbering::phi_translated_contract;
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule,
    phi_translated_contract(
        b"omega.psi-rule.phi-translated-proof-certified-compatible-policy-scalar-gvn.v1",
        OptimizationSafetyClass::ProofCertified,
    ),
    propose = propose,
);

fn propose(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    super::propose_phi_translated_scalar_gvn(
        unit,
        analyses,
        |unit, function, effects, _| {
            let (leaders, redundants) =
                collect_compatible_policy_scalar_rows(unit, function, effects);
            // The compatible-policy rule refuses a redundant at the function
            // entry: an entry block has no incoming join to translate.
            let redundants = redundants
                .into_iter()
                .filter(|row| row.1.block != function.entry)
                .collect();
            (leaders, redundants)
        },
        |key, parameters| key.references_any(parameters),
        |key, translation| Some(key.translate(translation)),
        |found| {
            PsiRewriteCandidate::new_proof_certified_phi_translated_scalar_common_subexpression(
                unit.identity,
                contract,
                found.affected_blocks,
                found.provenance,
                found.extras,
                -1,
                found.patch,
            )
            .map_err(RuleProposalError::InvalidCandidate)
        },
    )
}
