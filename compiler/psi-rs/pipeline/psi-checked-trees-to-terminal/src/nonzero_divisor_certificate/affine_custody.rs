//! Affine-witness completion for canonical order certificates.
//!
//! Evidence selection remains in the parent producer. This module owns the
//! bounded witness frontier, exact mapped bound, and optional closed relaxation
//! that complete one already-constructed affine root bound.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{IntegerAffineWitness, ProofNode};

mod completion;
mod definition_index;
mod frontier;
mod relaxation;

pub(super) use definition_index::DefinitionIndex;

pub(super) fn prove_from_root(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    root_bound: ProofNode,
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    for target in [goal_left, goal_right]
        .into_iter()
        .filter(|target| matches!(target, ScalarTerm::Value { .. }))
    {
        for definition_axioms in
            frontier::definition_words(context, semantic_axioms, definitions, root)
        {
            let witness = IntegerAffineWitness {
                root: root.clone(),
                target: target.clone(),
                definition_axioms,
            };
            if let Some(proof) = completion::prove(
                context,
                goal,
                assumptions,
                semantic_axioms,
                &root_bound,
                witness,
            ) {
                return Some(proof);
            }
        }
    }
    None
}
