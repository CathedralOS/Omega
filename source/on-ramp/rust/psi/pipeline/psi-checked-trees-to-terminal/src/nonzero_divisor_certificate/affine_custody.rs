//! Affine-witness completion for canonical order certificates.
//!
//! Evidence selection remains in the parent producer. This module owns the
//! bounded witness frontier, exact mapped bound, and optional closed relaxation
//! that complete one already-constructed affine root bound.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_admission::ProofNode;

mod boundary;
mod candidates;
mod completion;
mod definition_index;
mod frontier;
mod mapped;
mod relaxation;

pub(super) use boundary::prove_from_root_after;
pub(super) use definition_index::DefinitionIndex;
pub(super) use mapped::prove_mapped_to_target_before;

pub(super) fn prove_from_root(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
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
