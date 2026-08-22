//! Source-ordered one-alias transitive candidates for certificate production.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::ProofNode;

use super::super::super::super::integer_evidence::cited_facts;
use super::super::TwoCitationChains;

pub(super) fn find<'a, T>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut complete: impl FnMut(
        &'a ScalarTerm,
        &'a ScalarTerm,
        &'a ScalarTerm,
        &'a ScalarTerm,
        ProofNode,
        ProofNode,
        ProofNode,
    ) -> Option<T>,
) -> Option<T> {
    let chains = TwoCitationChains::new(assumptions, semantic_axioms);
    for (equality_citation, equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(equality_left, equality_right) = equality else {
            continue;
        };
        for (root, alias) in [
            (equality_left, equality_right),
            (equality_right, equality_left),
        ] {
            if root == alias
                || !matches!(root, ScalarTerm::Value { .. })
                || !matches!(alias, ScalarTerm::Value { .. })
            {
                continue;
            }
            let result = chains.find(|left_citation, left_fact, right_citation, right_fact| {
                let Proposition::LessOrEqual(left, _) = left_fact else {
                    unreachable!("only integer chains are enumerated")
                };
                let Proposition::LessOrEqual(_, right) = right_fact else {
                    unreachable!("only integer chains are enumerated")
                };
                complete(
                    root,
                    alias,
                    left,
                    right,
                    left_citation.proof(left_fact),
                    right_citation.proof(right_fact),
                    equality_citation.proof(equality),
                )
            });
            if result.is_some() {
                return result;
            }
        }
    }
    None
}
