//! Producer-local ordered alias-landed-literal candidates.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, ScalarTerm};

use super::super::super::super::integer_evidence::cited_facts;
use super::super::super::distinct_same_carrier_values;

pub(super) fn find<T>(
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    mut complete: impl FnMut(&ScalarTerm, &ScalarTerm, &ScalarTerm, ProofNode, ProofNode) -> Option<T>,
) -> Option<T> {
    for (root_citation, root_equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(root_left, root_right) = root_equality else {
            continue;
        };
        for (root, alias) in [(root_left, root_right), (root_right, root_left)] {
            if !distinct_same_carrier_values(root, alias) {
                continue;
            }
            for (literal_citation, literal_equality) in cited_facts(assumptions, semantic_axioms) {
                if std::ptr::eq(root_equality, literal_equality) {
                    continue;
                }
                let Proposition::Equal(literal_left, literal_right) = literal_equality else {
                    continue;
                };
                let literal = if literal_left == alias {
                    literal_right
                } else if literal_right == alias {
                    literal_left
                } else {
                    continue;
                };
                let Some((integer_type, _)) = literal.integer_value() else {
                    continue;
                };
                if root.scalar_type() != semantic_vocabulary::ScalarType::Integer(integer_type) {
                    continue;
                }
                if let Some(result) = complete(
                    root,
                    alias,
                    literal,
                    root_citation.proof(root_equality),
                    literal_citation.proof(literal_equality),
                ) {
                    return Some(result);
                }
            }
        }
    }
    None
}
