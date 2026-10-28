//! The dominating GVN rules: one row per expression-key flavour over the
//! shared dominating-leader traversal beside it.

use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use super::propose_dominating;
use crate::rules::global_value_numbering::dominating_contract;
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    /// Obligation-free cross-block elimination from dominating leaders.
    DominatorTotalScalarGvnRule,
    dominating_contract(
        b"omega.psi-rule.dominator-obligation-free-total-scalar-gvn.v1",
        OptimizationSafetyClass::ExactOperationSemantics,
    ),
    propose = propose_dominating,
    super::TOTAL_CSE_KEYS,
);

psi_rule_contract!(
    /// Proof-certified cross-block elimination from dominating leaders.
    DominatorProofCertifiedScalarGvnRule,
    dominating_contract(
        b"omega.psi-rule.dominator-proof-certified-total-scalar-gvn.v1",
        OptimizationSafetyClass::ProofCertified,
    ),
    propose = propose_dominating,
    super::PROOF_CERTIFIED_CSE_KEYS,
);

psi_rule_contract!(
    /// Cross-block elimination from proof-bearing redundants to total-policy
    /// leaders.
    DominatorProofCertifiedCompatiblePolicyScalarGvnRule,
    dominating_contract(
        b"omega.psi-rule.dominator-proof-certified-compatible-policy-scalar-gvn.v1",
        OptimizationSafetyClass::ProofCertified,
    ),
    propose = propose_dominating,
    super::COMPATIBLE_POLICY_CSE_KEYS,
);
