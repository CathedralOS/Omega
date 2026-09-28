//! Producer-local completion of one enumerated affine witness.

use proof_admission::{IntegerAffineWitness, ProofNode};
use semantic_vocabulary::{Proposition, PropositionContext};

use super::DefinitionIndex;

mod direct;
mod relaxed;

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root_bound: &ProofNode,
    witness: IntegerAffineWitness,
) -> Option<ProofNode> {
    // The kernel's IntegerAffineBound rule replays this same witness check
    // before anything else, so a witness that fails here cannot produce an
    // acceptable certificate below. The shared table replays it once per
    // distinct word rather than once per candidate, and the checked form
    // feeds both the direct conversion and the relaxed completion.
    let form = definitions.affine_form(context, semantic_axioms, &witness)?;
    if let Some(direct) = direct::prove(
        context,
        goal,
        assumptions,
        semantic_axioms,
        definitions,
        root_bound,
        &witness,
        &form,
    ) {
        return Some(direct);
    }
    relaxed::prove(
        context,
        goal,
        assumptions,
        semantic_axioms,
        definitions,
        &form,
        root_bound,
        witness,
    )
}
