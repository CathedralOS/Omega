//! Exactly two value-alias substitutions for canonical order production.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::{ProofNode, ProofRule};

use super::super::integer_evidence::cited_facts;
use super::index::{distinct_same_carrier_values, indexed_bounds, substitute_bound_endpoint};

pub(super) fn prove(
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    mut complete: impl FnMut(&ScalarTerm, ProofNode) -> Option<ProofNode>,
) -> Option<ProofNode> {
    let bounds_by_endpoint = indexed_bounds(assumptions, semantic_axioms);
    for (outer_citation, outer_equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(outer_left, outer_right) = outer_equality else {
            continue;
        };
        for (root, middle_alias) in [(outer_left, outer_right), (outer_right, outer_left)] {
            if !distinct_same_carrier_values(root, middle_alias) {
                continue;
            }
            for (inner_citation, inner_equality) in cited_facts(assumptions, semantic_axioms) {
                if std::ptr::eq(outer_equality, inner_equality) {
                    continue;
                }
                let Proposition::Equal(inner_left, inner_right) = inner_equality else {
                    continue;
                };
                let bound_alias = if inner_left == middle_alias {
                    inner_right
                } else if inner_right == middle_alias {
                    inner_left
                } else {
                    continue;
                };
                if bound_alias == root || !distinct_same_carrier_values(middle_alias, bound_alias) {
                    continue;
                }
                let Some(bounds) = bounds_by_endpoint.get(bound_alias) else {
                    continue;
                };
                for &(relation_citation, relation, endpoint) in bounds {
                    let middle_bound = ProofNode {
                        conclusion: substitute_bound_endpoint(relation, middle_alias, endpoint),
                        rule: ProofRule::IntegerLessOrEqualSubstitution {
                            relation: Box::new(relation_citation.proof(relation)),
                            equality: Box::new(inner_citation.proof(inner_equality)),
                            endpoint,
                        },
                    };
                    let root_bound = ProofNode {
                        conclusion: substitute_bound_endpoint(relation, root, endpoint),
                        rule: ProofRule::IntegerLessOrEqualSubstitution {
                            relation: Box::new(middle_bound),
                            equality: Box::new(outer_citation.proof(outer_equality)),
                            endpoint,
                        },
                    };
                    if let Some(proof) = complete(root, root_bound) {
                        return Some(proof);
                    }
                }
            }
        }
    }
    None
}
