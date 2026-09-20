//! Fixed-shape transport certificates for successor-rewritten path facts.
//!
//! An established roster fact restated to a successor's parameters is not a
//! fresh premise: the roster already contains the source fact and this edge's
//! binding equalities `parameter = argument`. The parameter form follows by
//! value-equality transport — under those equations the source and the
//! restated fact carry the same denotation — so reconstruction re-decides a
//! fixed-shape certificate before classifying the emission: the source fact
//! cited as one semantic axiom, the edge's binding equalities cited as
//! semantic axioms in roster order. A certificate the checker rejects leaves
//! the emission under `fact:successor-parameter-binding`'s licensed premise
//! introductions rather than failing the module.
//!
//! The generation-time check runs in production regardless; only the test
//! module reads the classification back through [`RewrittenSuccessorFact`].

use proof_admission::{ProofNode, ProofRule, check_certificate};
use semantic_vocabulary::{Proposition, PropositionContext};

#[cfg(test)]
mod tests;

/// One rewritten path fact tagged by how the generator discharged it.
///
/// `certified` is `true` when the fixed-shape `ValueEqualityTransport`
/// certificate — the roster's own fact transported through this successor's
/// binding equalities — was re-decided by the certificate checker before the
/// fact joined the roster (`fact:successor-path-transport`). `false` marks a
/// licensed premise introduction retained under
/// `fact:successor-parameter-binding`.
#[derive(Debug)]
pub(in super::super) struct RewrittenSuccessorFact {
    /// The substituted proposition appended to the successor's axiom roster.
    #[allow(dead_code)]
    pub proposition: Proposition,
    /// Whether the certificate checker accepted the emission's derivation.
    #[allow(dead_code)]
    pub certified: bool,
}

/// Re-decide the fixed-shape transport certificate for one rewritten
/// successor fact.
///
/// `source_index` names the established fact's position in `axioms` and
/// `equality_indices` name this successor's binding equalities in the same
/// roster. A stale index, an empty equality list, or a rejected certificate
/// yields `false`: the emission is correct either way and simply keeps its
/// licensed-introduction classification.
pub(in super::super) fn successor_rewrite_certified(
    context: &PropositionContext,
    axioms: &[Proposition],
    source_index: usize,
    equality_indices: &[usize],
    rewritten: &Proposition,
) -> bool {
    let Some(source) = axioms.get(source_index) else {
        return false;
    };
    if equality_indices.is_empty() {
        return false;
    }
    let equalities = equality_indices
        .iter()
        .map(|&index| {
            axioms.get(index).map(|conclusion| ProofNode {
                conclusion: conclusion.clone(),
                rule: ProofRule::SemanticAxiom { index },
            })
        })
        .collect::<Option<Vec<_>>>();
    let Some(equalities) = equalities else {
        return false;
    };
    let certificate = ProofNode {
        conclusion: rewritten.clone(),
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(ProofNode {
                conclusion: source.clone(),
                rule: ProofRule::SemanticAxiom {
                    index: source_index,
                },
            }),
            equalities,
        },
    };
    check_certificate(context, rewritten, &[], axioms, &certificate).is_ok()
}
