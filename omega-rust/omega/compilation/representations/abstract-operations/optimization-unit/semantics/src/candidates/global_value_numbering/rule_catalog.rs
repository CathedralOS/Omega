//! Exact GVN rule identities and the proof classes they admit.
use crate::OptimizationUnitValidationError;
use optimization_core::{AnalysisKind, OptimizationRuleIdentity, OptimizationSafetyClass};

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

/// The analyses a scalar common-subexpression proposal reads. A same-block
/// elimination compares two rows inside one block; a dominating or
/// phi-translated one also reconstructs the dominator or join frontier.
pub(super) const SAME_BLOCK_ANALYSES: &[AnalysisKind] =
    &[AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries];
pub(super) const DOMINATING_ANALYSES: &[AnalysisKind] = &[
    AnalysisKind::ControlFlowGraph,
    AnalysisKind::Dominators,
    AnalysisKind::UseDefinition,
    AnalysisKind::EffectSummaries,
];

/// The safety class one proof class claims: an obligation-free identity holds by
/// exact operation semantics; a proof-certified or compatible-policy rewrite
/// holds only under the proof the candidate carries.
pub(super) fn safety_class(proof_class: ScalarCseProofClass) -> OptimizationSafetyClass {
    match proof_class {
        ScalarCseProofClass::ObligationFree => OptimizationSafetyClass::ExactOperationSemantics,
        ScalarCseProofClass::ProofCertified | ScalarCseProofClass::CompatiblePolicy => {
            OptimizationSafetyClass::ProofCertified
        }
    }
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
