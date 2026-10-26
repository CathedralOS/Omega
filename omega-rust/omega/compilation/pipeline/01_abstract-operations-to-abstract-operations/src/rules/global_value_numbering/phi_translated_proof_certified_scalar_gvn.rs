//! Proof-certified total-scalar translation across join parameters.

use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use super::expression_keys::proof_certified_scalar_expression;
use crate::rules::accepted_obligations::accepted_obligation_fact;
use crate::rules::global_value_numbering::phi_translated_contract;
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    PhiTranslatedProofCertifiedScalarGvnRule,
    phi_translated_contract(
        b"omega.psi-rule.phi-translated-proof-certified-total-scalar-gvn.v1",
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
            super::collect_scalar_rows(
                unit,
                function,
                effects,
                |operation| {
                    let row = proof_certified_scalar_expression(operation)?;
                    accepted_obligation_fact(unit, function.machine, row.1).ok()?;
                    Some(row)
                },
                |operation| {
                    let row = proof_certified_scalar_expression(operation)?;
                    let fact = accepted_obligation_fact(unit, function.machine, row.1).ok()?;
                    Some((row, fact))
                },
            )
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
