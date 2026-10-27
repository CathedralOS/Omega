//! The phi-translated scalar GVN rules: one row per expression-key flavour
//! over the shared join-parameter translation beside it.

use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use super::propose_phi_translated;
use crate::rules::global_value_numbering::phi_translated_contract;
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    /// Obligation-free total-scalar translation across join parameters.
    PhiTranslatedObligationFreeScalarGvnRule,
    phi_translated_contract(
        b"omega.psi-rule.phi-translated-obligation-free-total-scalar-gvn.v1",
        OptimizationSafetyClass::ExactOperationSemantics,
    ),
    propose = propose_phi_translated,
    super::total_cse_phi_rows,
    super::expression_keys::TotalScalarExpressionKey::references_any,
    |key, translation| key.translate(translation),
);

psi_rule_contract!(
    /// Proof-certified total-scalar translation across join parameters.
    PhiTranslatedProofCertifiedScalarGvnRule,
    phi_translated_contract(
        b"omega.psi-rule.phi-translated-proof-certified-total-scalar-gvn.v1",
        OptimizationSafetyClass::ProofCertified,
    ),
    propose = propose_phi_translated,
    super::proof_certified_cse_phi_rows,
    super::expression_keys::ProofCertifiedScalarExpressionKey::references_any,
    |key, translation| Some(key.translate(translation)),
);

psi_rule_contract!(
    /// Proof-certified translation from exact redundants to total-policy
    /// leaders.
    PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule,
    phi_translated_contract(
        b"omega.psi-rule.phi-translated-proof-certified-compatible-policy-scalar-gvn.v1",
        OptimizationSafetyClass::ProofCertified,
    ),
    propose = propose_phi_translated,
    super::compatible_policy_cse_phi_rows,
    super::expression_keys::CompatiblePolicyScalarExpressionKey::references_any,
    |key, translation| Some(key.translate(translation)),
);
