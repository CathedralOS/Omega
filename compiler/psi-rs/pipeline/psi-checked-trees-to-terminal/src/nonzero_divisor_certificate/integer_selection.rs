//! Canonical fixed-integer proposition and bound selection.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

use super::affine_custody::DefinitionIndex;

mod bound;
mod dispatch;
mod exact;
mod logical;
mod order;
mod substitution;

pub(super) fn build(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let mut definitions = DefinitionIndex::new(semantic_axioms);
    build_with_definitions(
        context,
        goal,
        assumptions,
        semantic_axioms,
        &mut definitions,
    )
}

fn build_with_definitions(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    if let Some(proof) = exact::prove(goal, assumptions, semantic_axioms) {
        return Some(proof);
    }
    if let Some(proof) =
        dispatch::prove_atomic(context, goal, assumptions, semantic_axioms, definitions)
    {
        return proof;
    }
    match goal {
        Proposition::Conjunction(conjuncts) => {
            logical::prove_conjunction(goal, conjuncts, |part| {
                build_with_definitions(context, part, assumptions, semantic_axioms, definitions)
            })
        }
        Proposition::Disjunction(disjuncts) => {
            logical::prove_disjunction(goal, disjuncts, |part| {
                build_with_definitions(context, part, assumptions, semantic_axioms, definitions)
            })
        }
        _ => None,
    }
}
