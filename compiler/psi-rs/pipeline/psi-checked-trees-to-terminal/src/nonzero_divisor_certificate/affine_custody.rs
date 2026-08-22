//! Affine-witness completion for canonical order certificates.
//!
//! Evidence selection remains in the parent producer. This module owns the
//! bounded witness frontier, exact mapped bound, and optional closed relaxation
//! that complete one already-constructed affine root bound.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{
    IntegerAffineWitness, ProofNode, ProofRule, check_certificate, check_integer_affine_witness,
};

mod frontier;
mod relaxation;

pub(super) use frontier::DefinitionIndex;

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
            let direct = ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(root_bound.clone()),
                    witness: witness.clone(),
                },
            };
            if check_certificate(context, goal, assumptions, semantic_axioms, &direct).is_ok() {
                return Some(direct);
            }

            let Ok(form) = check_integer_affine_witness(context, semantic_axioms, &witness) else {
                continue;
            };
            let Some(relaxed) = relaxation::prove(goal, &form, &root_bound, witness) else {
                continue;
            };
            if check_certificate(context, goal, assumptions, semantic_axioms, &relaxed).is_ok() {
                return Some(relaxed);
            }
        }
    }
    None
}
