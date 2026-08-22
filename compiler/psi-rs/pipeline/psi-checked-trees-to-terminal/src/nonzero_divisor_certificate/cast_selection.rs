//! Side-local evidence selection for exact integer-cast bounds.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

use super::integer_evidence::cited_facts;
use super::{alias_transport, cast_custody};

mod literal;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    if !matches!(goal, Proposition::LessOrEqual(_, _)) {
        return None;
    }
    for (citation, root_bound) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(root_left, root_right) = root_bound else {
            continue;
        };
        for root in [root_left, root_right]
            .into_iter()
            .filter(|root| matches!(root, psi_core::ScalarTerm::Value { .. }))
        {
            if let Some(proof) = cast_custody::prove_from_root(
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
    literal::prove(context, goal, assumptions, semantic_axioms)
        .or_else(|| prove_alias_substituted_cast_bound(context, goal, assumptions, semantic_axioms))
}

fn prove_alias_substituted_cast_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    alias_transport::prove_one(assumptions, semantic_axioms, |root, root_bound| {
        cast_custody::prove_from_root(
            context,
            goal,
            assumptions,
            semantic_axioms,
            root,
            root_bound,
        )
    })
    .or_else(|| alias_transport::prove_stronger_cast(context, goal, assumptions, semantic_axioms))
    .or_else(|| {
        alias_transport::prove_landed_literal_cast(context, goal, assumptions, semantic_axioms)
    })
    .or_else(|| prove_two_alias_substituted_cast_bound(context, goal, assumptions, semantic_axioms))
}

fn prove_two_alias_substituted_cast_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    alias_transport::prove_two(assumptions, semantic_axioms, |root, root_bound| {
        cast_custody::prove_from_root(
            context,
            goal,
            assumptions,
            semantic_axioms,
            root,
            root_bound,
        )
    })
}
