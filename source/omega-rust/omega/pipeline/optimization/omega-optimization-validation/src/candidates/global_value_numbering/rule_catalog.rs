//! Exact GVN rule identities and the proof classes they admit.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScalarCseScope {
    SameBlock,
    Dominating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScalarCseProofClass {
    ObligationFree,
    ProofCertified,
    CompatiblePolicy,
}

pub(super) fn scoped_proof_class(
    scope: ScalarCseScope,
    rule: OptimizationRuleIdentity,
) -> Result<ScalarCseProofClass, OptimizationUnitValidationError> {
    let rows = match scope {
        ScalarCseScope::SameBlock => [
            (
                b"omega.psi-rule.same-block-obligation-free-total-scalar-cse.v1".as_slice(),
                ScalarCseProofClass::ObligationFree,
            ),
            (
                b"omega.psi-rule.same-block-proof-certified-total-scalar-cse.v1".as_slice(),
                ScalarCseProofClass::ProofCertified,
            ),
            (
                b"omega.psi-rule.same-block-proof-certified-compatible-policy-scalar-cse.v1"
                    .as_slice(),
                ScalarCseProofClass::CompatiblePolicy,
            ),
        ],
        ScalarCseScope::Dominating => [
            (
                b"omega.psi-rule.dominator-obligation-free-total-scalar-gvn.v1".as_slice(),
                ScalarCseProofClass::ObligationFree,
            ),
            (
                b"omega.psi-rule.dominator-proof-certified-total-scalar-gvn.v1".as_slice(),
                ScalarCseProofClass::ProofCertified,
            ),
            (
                b"omega.psi-rule.dominator-proof-certified-compatible-policy-scalar-gvn.v1"
                    .as_slice(),
                ScalarCseProofClass::CompatiblePolicy,
            ),
        ],
    };
    proof_class_from_rows(rule, &rows)
}

pub(super) fn phi_translated_proof_class(
    rule: OptimizationRuleIdentity,
) -> Result<ScalarCseProofClass, OptimizationUnitValidationError> {
    proof_class_from_rows(
        rule,
        &[
            (
                b"omega.psi-rule.phi-translated-obligation-free-total-scalar-gvn.v1".as_slice(),
                ScalarCseProofClass::ObligationFree,
            ),
            (
                b"omega.psi-rule.phi-translated-proof-certified-total-scalar-gvn.v1".as_slice(),
                ScalarCseProofClass::ProofCertified,
            ),
            (
                b"omega.psi-rule.phi-translated-proof-certified-compatible-policy-scalar-gvn.v1"
                    .as_slice(),
                ScalarCseProofClass::CompatiblePolicy,
            ),
        ],
    )
}

fn proof_class_from_rows(
    rule: OptimizationRuleIdentity,
    rows: &[(&[u8], ScalarCseProofClass)],
) -> Result<ScalarCseProofClass, OptimizationUnitValidationError> {
    rows.iter()
        .find_map(|(identity, proof_class)| {
            (rule == OptimizationRuleIdentity::from_canonical_bytes(identity))
                .then_some(*proof_class)
        })
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)
}
