//! Fixed two-citation transitive affine evidence construction.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::{ProofNode, ProofRule};

use super::super::affine_custody;

mod alias;
mod chains;

use chains::TwoCitationChains;

/// Reconstruct one affine-root bound through exactly two ordered citations and
/// one exact value equality. This deliberately calls the affine constructor
/// directly: it does not recurse through the general integer-bound search, so
/// neither equality chains nor longer order paths are admitted here.
pub(super) fn prove_transitively_alias_substituted_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    alias::prove(context, goal, assumptions, semantic_axioms)
}

pub(super) fn prove_transitively_reconstructed_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    TwoCitationChains::new(assumptions, semantic_axioms).find(
        |left_citation, left_fact, right_citation, right_fact| {
            let Proposition::LessOrEqual(left, _) = left_fact else {
                unreachable!("only integer chains are enumerated")
            };
            let Proposition::LessOrEqual(_, right) = right_fact else {
                unreachable!("only integer chains are enumerated")
            };
            let root_bound = ProofNode {
                conclusion: Proposition::LessOrEqual(left.clone(), right.clone()),
                rule: ProofRule::IntegerLessOrEqualTransitivity {
                    left_less_or_equal_middle: Box::new(left_citation.proof(left_fact)),
                    middle_less_or_equal_right: Box::new(right_citation.proof(right_fact)),
                },
            };
            for root in [left, right]
                .into_iter()
                .filter(|root| matches!(root, psi_core::ScalarTerm::Value { .. }))
            {
                if let Some(proof) = affine_custody::prove_from_root(
                    context,
                    goal,
                    assumptions,
                    semantic_axioms,
                    root,
                    root_bound.clone(),
                ) {
                    return Some(proof);
                }
            }
            None
        },
    )
}
