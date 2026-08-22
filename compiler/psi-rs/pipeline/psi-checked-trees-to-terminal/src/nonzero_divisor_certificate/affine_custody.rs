//! Affine-witness completion for canonical order certificates.
//!
//! Evidence selection remains in the parent producer. This module owns the
//! bounded witness frontier, exact mapped bound, and optional closed relaxation
//! that complete one already-constructed affine root bound.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::ProofNode;

mod candidates;
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
    candidates::find(
        context,
        goal,
        semantic_axioms,
        definitions,
        root,
        |witness| {
            completion::prove(
                context,
                goal,
                assumptions,
                semantic_axioms,
                &root_bound,
                witness,
            )
        },
    )
}

pub(super) fn prove_from_root_after(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    minimum_axiom: usize,
    root_bound: ProofNode,
) -> Option<ProofNode> {
    candidates::find(
        context,
        goal,
        semantic_axioms,
        definitions,
        root,
        |witness| {
            (witness
                .definition_axioms
                .iter()
                .all(|&index| index > minimum_axiom)
                && witness
                    .literal_axioms
                    .iter()
                    .flatten()
                    .all(|&index| index > minimum_axiom))
            .then(|| {
                completion::prove(
                    context,
                    goal,
                    assumptions,
                    semantic_axioms,
                    &root_bound,
                    witness,
                )
            })
            .flatten()
        },
    )
}
