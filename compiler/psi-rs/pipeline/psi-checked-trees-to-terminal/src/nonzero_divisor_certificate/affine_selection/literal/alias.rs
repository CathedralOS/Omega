//! One-intermediate-alias literal landing for affine certificate production.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

use super::super::super::integer_evidence::cited_facts;

mod completion;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    for (outer_citation, outer_equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(outer_left, outer_right) = outer_equality else {
            continue;
        };
        for (root, alias) in [(outer_left, outer_right), (outer_right, outer_left)] {
            if root == alias
                || !matches!(root, psi_core::ScalarTerm::Value { .. })
                || !matches!(alias, psi_core::ScalarTerm::Value { .. })
                || root.scalar_type() != alias.scalar_type()
            {
                continue;
            }
            for (inner_citation, inner_equality) in cited_facts(assumptions, semantic_axioms) {
                if std::ptr::eq(outer_equality, inner_equality) {
                    continue;
                }
                let Proposition::Equal(inner_left, inner_right) = inner_equality else {
                    continue;
                };
                let literal = if inner_left == alias {
                    inner_right
                } else if inner_right == alias {
                    inner_left
                } else {
                    continue;
                };
                let Some((integer_type, _)) = literal.integer_value() else {
                    continue;
                };
                if root.scalar_type() != psi_core::ScalarType::Integer(integer_type) {
                    continue;
                }
                if let Some(proof) = completion::prove(
                    context,
                    goal,
                    assumptions,
                    semantic_axioms,
                    root,
                    alias,
                    literal,
                    outer_citation.proof(outer_equality),
                    inner_citation.proof(inner_equality),
                ) {
                    return Some(proof);
                }
            }
        }
    }
    None
}
