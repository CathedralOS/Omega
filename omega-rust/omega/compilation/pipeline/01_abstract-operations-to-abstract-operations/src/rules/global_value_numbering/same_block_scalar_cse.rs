//! The same-block scalar CSE rules: one row per expression-key flavour over
//! the shared same-block traversal beside it.

use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use super::propose_same_block;
use crate::rules::global_value_numbering::same_block_contract;
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    /// Obligation-free same-block common-subexpression elimination.
    SameBlockTotalScalarCseRule,
    same_block_contract(
        b"omega.psi-rule.same-block-obligation-free-total-scalar-cse.v1",
        OptimizationSafetyClass::ExactOperationSemantics,
    ),
    propose = propose_same_block,
    super::TOTAL_CSE_KEYS,
);

psi_rule_contract!(
    /// Proof-certified same-block common-subexpression elimination.
    SameBlockProofCertifiedScalarCseRule,
    same_block_contract(
        b"omega.psi-rule.same-block-proof-certified-total-scalar-cse.v1",
        OptimizationSafetyClass::ProofCertified,
    ),
    propose = propose_same_block,
    super::PROOF_CERTIFIED_CSE_KEYS,
);

psi_rule_contract!(
    /// Same-block elimination from proof-bearing redundants to total-policy
    /// leaders.
    SameBlockProofCertifiedCompatiblePolicyScalarCseRule,
    same_block_contract(
        b"omega.psi-rule.same-block-proof-certified-compatible-policy-scalar-cse.v1",
        OptimizationSafetyClass::ProofCertified,
    ),
    propose = propose_same_block,
    super::COMPATIBLE_POLICY_CSE_KEYS,
);
