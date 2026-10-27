//! Exact proof-check rule identity to independent validation-protocol map, and
//! the five proof-certified integer constant laws themselves.
//!
//! Three protocols exist: the dead-scalar node elision, the operand
//! substitution identity, and the integer constant laws. Each law's row states
//! the exact producer rule it replays, the validator identity it publishes,
//! the analysis roster its proposal carries, and the two law-owned operations
//! the shared protocol calls to reconstruct its shape and to check its
//! evidence.
use optimization_core::{AnalysisKind, OptimizationRuleIdentity};

use super::integer_constant_law::IndependentIntegerConstantLaw;
use super::same_operand::{
    accepted_operation_obligation_evidence, exact_integer_self_subtract_shape,
    integer_self_divide_shape, integer_self_remainder_shape,
};
use super::unit_divisor::{
    integer_remainder_by_one_shape, materialized_divisor_evidence,
    signed_integer_remainder_by_negative_one_shape,
};

#[derive(Debug, Clone, Copy)]
pub(super) enum ProofCheckValidationRoute {
    DeadScalar,
    OperandSubstitution,
    IntegerConstantLaw(IndependentIntegerConstantLaw),
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
        } else {
            // Each constant law names its own rule domain, so the roster below
            // is the only place this mapping is stated.
            return integer_constant_laws()
                .into_iter()
                .find(|law| rule == identity(law.rule_domain))
                .map(Self::IntegerConstantLaw);
        };
        Some(route)
    }
}

pub(crate) fn is_proof_check_elision_rule(rule: OptimizationRuleIdentity) -> bool {
    ProofCheckValidationRoute::for_rule(rule).is_some()
}

/// The five proof-certified integer constant laws. Each row names the exact
/// producer rule it replays, the validator identity one accepted rewrite
/// publishes, the analyses its proposal reads, and the law-owned recogniser
/// and evidence check the shared protocol calls.
fn integer_constant_laws() -> [IndependentIntegerConstantLaw; 5] {
    const SAME_OPERAND_ANALYSES: &[AnalysisKind] =
        &[AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries];
    const MATERIALIZED_DIVISOR_ANALYSES: &[AnalysisKind] = &[
        AnalysisKind::ScalarConstants,
        AnalysisKind::UseDefinition,
        AnalysisKind::EffectSummaries,
    ];
    [
        IndependentIntegerConstantLaw {
            rule_domain:
                b"omega.psi-rule.live-proof-certified-exact-integer-self-subtract-elimination.v1",
            validator_domain:
                b"omega.validator.live-proof-certified-exact-integer-self-subtract-elimination.v1",
            required_analyses: SAME_OPERAND_ANALYSES,
            recognise: exact_integer_self_subtract_shape,
            verify_evidence: accepted_operation_obligation_evidence,
        },
        IndependentIntegerConstantLaw {
            rule_domain: b"omega.psi-rule.live-proof-certified-integer-self-remainder-elimination.v1",
            validator_domain: b"omega.validator.live-proof-certified-integer-self-remainder-elimination.v1",
            required_analyses: SAME_OPERAND_ANALYSES,
            recognise: integer_self_remainder_shape,
            verify_evidence: accepted_operation_obligation_evidence,
        },
        IndependentIntegerConstantLaw {
            rule_domain: b"omega.psi-rule.live-proof-certified-integer-self-divide-elimination.v1",
            validator_domain: b"omega.validator.live-proof-certified-integer-self-divide-elimination.v1",
            required_analyses: SAME_OPERAND_ANALYSES,
            recognise: integer_self_divide_shape,
            verify_evidence: accepted_operation_obligation_evidence,
        },
        IndependentIntegerConstantLaw {
            rule_domain:
                b"omega.psi-rule.live-proof-certified-integer-remainder-by-one-elimination.v1",
            validator_domain:
                b"omega.validator.live-proof-certified-integer-remainder-by-one-elimination.v1",
            required_analyses: MATERIALIZED_DIVISOR_ANALYSES,
            recognise: integer_remainder_by_one_shape,
            verify_evidence: materialized_divisor_evidence,
        },
        IndependentIntegerConstantLaw {
            rule_domain: b"omega.psi-rule.live-proof-certified-signed-integer-remainder-by-negative-one-elimination.v1",
            validator_domain: b"omega.validator.live-proof-certified-signed-integer-remainder-by-negative-one-elimination.v1",
            required_analyses: MATERIALIZED_DIVISOR_ANALYSES,
            recognise: signed_integer_remainder_by_negative_one_shape,
            verify_evidence: materialized_divisor_evidence,
        },
    ]
}

fn identity(domain: &[u8]) -> OptimizationRuleIdentity {
    OptimizationRuleIdentity::from_canonical_bytes(domain)
}
