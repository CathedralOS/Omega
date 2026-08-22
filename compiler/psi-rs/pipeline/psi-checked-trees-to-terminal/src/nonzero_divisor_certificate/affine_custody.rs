//! Affine-witness completion for canonical order certificates.
//!
//! Evidence selection remains in the parent producer. This module owns the
//! bounded witness frontier, exact mapped bound, and optional closed relaxation
//! that complete one already-constructed affine root bound.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{ProofNode, ProofRule, check_certificate, check_integer_affine_witness};

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

#[allow(clippy::too_many_arguments)]
pub(super) fn prove_mapped_to_target_before(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    target: &ScalarTerm,
    maximum_axiom: usize,
    root_bound: &ProofNode,
) -> Option<ProofNode> {
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
            let conclusion = relaxation::mapped_bound(&form, &root_bound.conclusion)?;
            let proof = ProofNode {
                conclusion: conclusion.clone(),
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(root_bound.clone()),
                    witness,
                },
            };
            check_certificate(context, &conclusion, assumptions, semantic_axioms, &proof)
                .is_ok()
                .then_some(proof)
        },
    )
}
