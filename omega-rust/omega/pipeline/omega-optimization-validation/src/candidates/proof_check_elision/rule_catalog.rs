//! Exact proof-check rule identity to independent validation-protocol map.

use omega_optimization_core::OptimizationRuleIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProofCheckValidationRoute {
    DeadScalar,
    OperandSubstitution,
    SelfSubtract,
    SelfRemainder,
    SelfDivide,
    RemainderByOne,
    RemainderByNegativeOne,
}

impl ProofCheckValidationRoute {
    pub(super) fn for_rule(rule: OptimizationRuleIdentity) -> Option<Self> {
        let route = if rule
            == identity(b"omega.psi-rule.dead-unused-proof-certified-scalar-elimination.v1")
        {
            Self::DeadScalar
        } else if [
            b"omega.psi-rule.live-proof-certified-integer-identity-elimination.v1".as_slice(),
            b"omega.psi-rule.live-proof-certified-integer-divide-by-one-elimination.v1".as_slice(),
            b"omega.psi-rule.live-proof-certified-exact-integer-multiply-by-zero-elimination.v1"
                .as_slice(),
            b"omega.psi-rule.live-proof-certified-integer-zero-dividend-elimination.v1".as_slice(),
            b"omega.psi-rule.live-proof-certified-exact-integer-zero-value-shift-elimination.v1"
                .as_slice(),
            b"omega.psi-rule.live-proof-certified-exact-signed-integer-negative-one-value-shift-right-elimination.v1"
                .as_slice(),
        ]
        .into_iter()
        .any(|domain| rule == identity(domain))
        {
            Self::OperandSubstitution
        } else if rule
            == identity(
                b"omega.psi-rule.live-proof-certified-exact-integer-self-subtract-elimination.v1",
            )
        {
            Self::SelfSubtract
        } else if rule
            == identity(
                b"omega.psi-rule.live-proof-certified-integer-self-remainder-elimination.v1",
            )
        {
            Self::SelfRemainder
        } else if rule
            == identity(b"omega.psi-rule.live-proof-certified-integer-self-divide-elimination.v1")
        {
            Self::SelfDivide
        } else if rule
            == identity(
                b"omega.psi-rule.live-proof-certified-integer-remainder-by-one-elimination.v1",
            )
        {
            Self::RemainderByOne
        } else if rule
            == identity(
                b"omega.psi-rule.live-proof-certified-signed-integer-remainder-by-negative-one-elimination.v1",
            )
        {
            Self::RemainderByNegativeOne
        } else {
            return None;
        };
        Some(route)
    }
}

pub(crate) fn is_proof_check_elision_rule(rule: OptimizationRuleIdentity) -> bool {
    ProofCheckValidationRoute::for_rule(rule).is_some()
}

fn identity(domain: &[u8]) -> OptimizationRuleIdentity {
    OptimizationRuleIdentity::from_canonical_bytes(domain)
}
