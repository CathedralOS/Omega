//! Exact dead-scalar rule identities and their independently accepted families.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DeadScalarFamily {
    Literal,
    UnconditionallyTotal,
    ProofCertified,
}

pub(super) fn dead_scalar_family(rule: OptimizationRuleIdentity) -> Option<DeadScalarFamily> {
    if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.dead-unused-scalar-literal-elimination.v1",
        )
    {
        Some(DeadScalarFamily::Literal)
    } else if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.dead-unused-unconditionally-total-scalar-elimination.v1",
        )
    {
        Some(DeadScalarFamily::UnconditionallyTotal)
    } else if rule
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.dead-unused-proof-certified-scalar-elimination.v1",
        )
    {
        Some(DeadScalarFamily::ProofCertified)
    } else {
        None
    }
}

pub(super) fn expected_safety(rule: OptimizationRuleIdentity) -> OptimizationSafetyClass {
    if dead_scalar_family(rule) == Some(DeadScalarFamily::ProofCertified) {
        OptimizationSafetyClass::ProofCertified
    } else {
        OptimizationSafetyClass::ExactOperationSemantics
    }
}
