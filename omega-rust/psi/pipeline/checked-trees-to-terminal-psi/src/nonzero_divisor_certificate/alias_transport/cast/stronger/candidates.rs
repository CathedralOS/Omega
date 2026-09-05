//! Producer-local ordered stronger-alias fact candidates.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, ScalarTerm};

use super::super::super::super::integer_evidence::cited_facts;
use super::super::super::distinct_same_carrier_values;

mod bound;

pub(super) fn find<T>(
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    mut complete: impl FnMut(
        &ScalarTerm,
        &ScalarTerm,
        &ScalarTerm,
        usize,
        ProofNode,
        ProofNode,
    ) -> Option<T>,
) -> Option<T> {
    for (equality_citation, equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(equality_left, equality_right) = equality else {
            continue;
        };
        for (root, alias) in [
            (equality_left, equality_right),
            (equality_right, equality_left),
        ] {
            if !distinct_same_carrier_values(root, alias) {
                continue;
            }
            for (bound_citation, bound) in cited_facts(assumptions, semantic_axioms) {
                let Proposition::LessOrEqual(bound_left, bound_right) = bound else {
                    continue;
                };
                let Some((literal, endpoint)) = bound::select(root, alias, bound_left, bound_right)
                else {
                    continue;
                };
                if let Some(result) = complete(
                    root,
                    alias,
                    literal,
                    endpoint,
                    bound_citation.proof(bound),
                    equality_citation.proof(equality),
                ) {
                    return Some(result);
                }
            }
        }
    }
    None
}
