//! Admission of supplied certificates under their recorded predicate denotation.
//!
//! The receiver selects the declared conversion and replays the certificate
//! against its reconstructed goal and premises. Proof search belongs to
//! `certificate_search`; it does not participate in this decision.

use crate::{check_predicate_denotations, check_predicate_denotations_with_value_equalities};
use semantic_vocabulary::{Proposition, PropositionContext};
use terminal_psi::CrashCertificate;

/// Check a supplied crash certificate without searching. The recorded
/// denotation lane is part of the certificate: a node produced under equality
/// transport is replayed against that conversion exactly as produced.
pub fn check_denotation_certificate(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    certificate: &CrashCertificate,
) -> bool {
    let convert = if certificate.with_value_equalities {
        check_predicate_denotations_with_value_equalities
    } else {
        check_predicate_denotations
    };
    let Ok(denotations) = convert(context, goal, requirements, semantic_axioms) else {
        return false;
    };
    denotations
        .check_certificate(context, &certificate.proof)
        .is_ok()
}
