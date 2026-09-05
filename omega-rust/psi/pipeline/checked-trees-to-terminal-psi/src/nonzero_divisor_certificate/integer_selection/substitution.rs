//! Fixed one- and two-equality integer-bound substitution proofs.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext};

use super::super::affine_custody::DefinitionIndex;

mod one;
mod relation;
mod two;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    prove_with_cast_mode(
        context,
        goal,
        assumptions,
        semantic_axioms,
        definitions,
        true,
    )
}

pub(super) fn prove_without_cast(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    prove_with_cast_mode(
        context,
        goal,
        assumptions,
        semantic_axioms,
        definitions,
        false,
    )
}

fn prove_with_cast_mode(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    allow_cast: bool,
) -> Option<ProofNode> {
    one::prove(
        context,
        goal,
        assumptions,
        semantic_axioms,
        definitions,
        allow_cast,
    )
    .or_else(|| {
        two::prove(
            context,
            goal,
            assumptions,
            semantic_axioms,
            definitions,
            allow_cast,
        )
    })
}
