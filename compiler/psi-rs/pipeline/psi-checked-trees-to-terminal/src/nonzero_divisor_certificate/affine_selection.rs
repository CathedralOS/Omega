//! Side-local selection of retained evidence for bounded affine proofs.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

use super::integer_evidence::cited_facts;
use super::{affine_custody, alias_transport};

mod literal;
mod transitive;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    for (citation, root_bound) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(root_left, root_right) = root_bound else {
            continue;
        };
        for root in [root_left, root_right]
            .into_iter()
            .filter(|root| matches!(root, psi_core::ScalarTerm::Value { .. }))
        {
            if let Some(proof) = affine_custody::prove_from_root(
                context,
                goal,
                assumptions,
                semantic_axioms,
                root,
                citation.proof(root_bound),
            ) {
                return Some(proof);
            }
        }
    }
    literal::prove_landed_literal_affine_bound(context, goal, assumptions, semantic_axioms)
        .or_else(|| {
            prove_alias_substituted_affine_bound(context, goal, assumptions, semantic_axioms)
        })
        .or_else(|| {
            transitive::prove_transitively_reconstructed_affine_bound(
                context,
                goal,
                assumptions,
                semantic_axioms,
            )
        })
        .or_else(|| {
            transitive::prove_transitively_alias_substituted_affine_bound(
                context,
                goal,
                assumptions,
                semantic_axioms,
            )
        })
        .or_else(|| {
            prove_two_alias_substituted_affine_bound(context, goal, assumptions, semantic_axioms)
        })
}

fn prove_alias_substituted_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    alias_transport::prove_one(assumptions, semantic_axioms, |root, root_bound| {
        affine_custody::prove_from_root(
            context,
            goal,
            assumptions,
            semantic_axioms,
            root,
            root_bound,
        )
    })
}

/// Transport one exact retained bound through exactly two distinct value
/// equalities before constructing the affine proof. The equality walk is fixed
/// at depth two; this does not recurse or enumerate a general alias graph.
fn prove_two_alias_substituted_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    alias_transport::prove_two(assumptions, semantic_axioms, |root, root_bound| {
        affine_custody::prove_from_root(
            context,
            goal,
            assumptions,
            semantic_axioms,
            root,
            root_bound,
        )
    })
}
