//! Source-ordered one-alias candidates for certificate production.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::ProofNode;

use super::super::super::integer_evidence::cited_facts;
use super::super::index::{distinct_same_carrier_values, indexed_bounds};
use super::completion;

pub(super) fn prove(
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    mut complete: impl FnMut(&ScalarTerm, ProofNode) -> Option<ProofNode>,
) -> Option<ProofNode> {
    let bounds_by_endpoint = indexed_bounds(assumptions, semantic_axioms);
    for (equality_citation, equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(left, right) = equality else {
            continue;
        };
        for (root, alias) in [(left, right), (right, left)] {
            if !distinct_same_carrier_values(root, alias) {
                continue;
            }
            let Some(bounds) = bounds_by_endpoint.get(alias) else {
                continue;
            };
            for &(relation_citation, relation, endpoint) in bounds {
                let proof = completion::prove(
                    relation,
                    root,
                    endpoint,
                    relation_citation.proof(relation),
                    equality_citation.proof(equality),
                );
                if let Some(proof) = complete(root, proof) {
                    return Some(proof);
                }
            }
        }
    }
    None
}
