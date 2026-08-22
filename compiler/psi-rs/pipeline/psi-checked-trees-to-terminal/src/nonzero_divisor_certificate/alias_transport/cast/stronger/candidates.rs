//! Producer-local ordered stronger-alias fact candidates.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::ProofNode;

use super::super::super::super::integer_evidence::cited_facts;
use super::super::super::distinct_same_carrier_values;

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
                let (literal, endpoint) = if bound_left == alias {
                    (bound_right, 0)
                } else if bound_right == alias {
                    (bound_left, 1)
                } else {
                    continue;
                };
                let Some((integer_type, _)) = literal.integer_value() else {
                    continue;
                };
                if root.scalar_type() != psi_core::ScalarType::Integer(integer_type) {
                    continue;
                }
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
