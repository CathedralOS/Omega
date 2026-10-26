//! Obligation-free total-scalar translation across join parameters.

use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use super::expression_keys::total_scalar_expression;
use crate::rules::global_value_numbering::phi_translated_contract;
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    PhiTranslatedObligationFreeScalarGvnRule,
    phi_translated_contract(
        b"omega.psi-rule.phi-translated-obligation-free-total-scalar-gvn.v1",
        OptimizationSafetyClass::ExactOperationSemantics,
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
        |unit, function, effects, value_types| {
            super::collect_scalar_rows(
                unit,
                function,
                effects,
                |operation| total_scalar_expression(operation, value_types),
                |operation| total_scalar_expression(operation, value_types).map(|row| (row, ())),
            )
        },
        |key, parameters| key.references_any(parameters),
        |key, translation| key.translate(translation),
        |found| {
            PsiRewriteCandidate::new_phi_translated_scalar_common_subexpression(
                unit.identity,
                contract,
                found.affected_blocks,
                found.provenance,
                -1,
                found.patch,
            )
            .map_err(RuleProposalError::InvalidCandidate)
        },
    )
}
