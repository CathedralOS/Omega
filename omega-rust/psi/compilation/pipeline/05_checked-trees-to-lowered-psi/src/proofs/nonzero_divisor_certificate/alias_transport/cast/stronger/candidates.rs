//! Producer-local ordered stronger-alias fact candidates.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType};

use super::super::super::super::integer_evidence::cited_facts;
use super::super::super::distinct_same_carrier_subjects;
use super::super::super::index::indexed_relations;

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
    let relations_by_endpoint = indexed_relations(assumptions, semantic_axioms);
    for (equality_citation, equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(equality_left, equality_right) = equality else {
            continue;
        };
        for (root, alias) in [
            (equality_left, equality_right),
            (equality_right, equality_left),
        ] {
            if !distinct_same_carrier_subjects(root, alias) {
                continue;
            }
            for &(bound_citation, bound, literal, endpoint) in
                relations_by_endpoint.get(alias).into_iter().flatten()
            {
                let Some((integer_type, _)) = literal.integer_value() else {
                    continue;
                };
                if root.scalar_type() != ScalarType::Integer(integer_type) {
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
