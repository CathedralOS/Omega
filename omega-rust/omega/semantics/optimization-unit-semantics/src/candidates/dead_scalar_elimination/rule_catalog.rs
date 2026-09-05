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

pub(super) fn expected_safety(rule: OptimizationRuleIdentity) -> Option<OptimizationSafetyClass> {
    dead_scalar_family(rule).map(|family| match family {
        DeadScalarFamily::Literal | DeadScalarFamily::UnconditionallyTotal => {
            OptimizationSafetyClass::ExactOperationSemantics
        }
        DeadScalarFamily::ProofCertified => OptimizationSafetyClass::ProofCertified,
    })
}

pub(super) fn validator_identity(
    rule: OptimizationRuleIdentity,
) -> Option<OptimizationValidatorIdentity> {
    dead_scalar_family(rule).map(|family| {
        OptimizationValidatorIdentity::from_canonical_bytes(match family {
            DeadScalarFamily::Literal => b"omega.validator.dead-unused-scalar-literal.v1",
            DeadScalarFamily::UnconditionallyTotal => {
                b"omega.validator.dead-unused-unconditionally-total-scalar.v1"
            }
            DeadScalarFamily::ProofCertified => {
                b"omega.validator.dead-unused-proof-certified-scalar-node.v1"
            }
        })
    })
}
