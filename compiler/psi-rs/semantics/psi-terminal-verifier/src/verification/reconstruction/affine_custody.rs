//! Independent affine-witness completion for obligation reconstruction.
//!
//! Evidence selection remains in the parent verifier. This module owns the
//! bounded witness frontier, exact mapped bound, and closed relaxation replay.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{check_integer_affine_bound_conversion, check_integer_affine_witness};

mod candidates;
mod completion;
mod definition_index;
mod frontier;
mod relaxation;

pub(super) use definition_index::DefinitionIndex;

pub(super) fn retained_from_root(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    root_bound: &Proposition,
) -> bool {
    candidates::any(
        context,
        goal,
        semantic_axioms,
        definitions,
        root,
        |witness| completion::retained(context, goal, semantic_axioms, root_bound, &witness),
    )
}

pub(super) fn retained_from_root_after(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    minimum_axiom: usize,
    root_bound: &Proposition,
) -> bool {
    candidates::any(
        context,
        goal,
        semantic_axioms,
        definitions,
        root,
        |witness| {
            witness
                .definition_axioms
                .iter()
                .all(|&index| index > minimum_axiom)
                && witness
                    .literal_axioms
                    .iter()
                    .flatten()
                    .all(|&index| index > minimum_axiom)
                && completion::retained(context, goal, semantic_axioms, root_bound, &witness)
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn retained_mapped_to_target_before(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    target: &ScalarTerm,
    maximum_axiom: usize,
    root_bound: &Proposition,
) -> Option<Proposition> {
    candidates::find_target(
        context,
        semantic_axioms,
        definitions,
        root,
        target,
        |witness| {
            if !witness
                .definition_axioms
                .iter()
                .all(|&index| index < maximum_axiom)
                || !witness
                    .literal_axioms
                    .iter()
                    .flatten()
                    .all(|&index| index < maximum_axiom)
            {
                return None;
            }
            let form = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
            let mapped = relaxation::mapped_bound(&form, root_bound)?;
            check_integer_affine_bound_conversion(&form, root_bound, &mapped)
                .is_ok()
                .then_some(mapped)
        },
    )
}
