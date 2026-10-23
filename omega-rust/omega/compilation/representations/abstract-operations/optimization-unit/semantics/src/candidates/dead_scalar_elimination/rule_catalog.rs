//! Exact dead-scalar rule identities and their independently accepted families.
use optimization_core::{OptimizationRuleIdentity, OptimizationSafetyClass};

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

/// The canonical validator-domain bytes one dead-scalar family publishes its
/// accepted rewrite under.
pub(super) fn validator_bytes(rule: OptimizationRuleIdentity) -> Option<&'static [u8]> {
    dead_scalar_family(rule).map(|family| match family {
        DeadScalarFamily::Literal => b"omega.validator.dead-unused-scalar-literal.v1".as_slice(),
        DeadScalarFamily::UnconditionallyTotal => {
            b"omega.validator.dead-unused-unconditionally-total-scalar.v1".as_slice()
        }
        DeadScalarFamily::ProofCertified => {
            b"omega.validator.dead-unused-proof-certified-scalar-node.v1".as_slice()
        }
    })
}
