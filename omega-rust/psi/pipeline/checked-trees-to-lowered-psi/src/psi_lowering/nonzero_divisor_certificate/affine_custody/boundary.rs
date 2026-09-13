//! Producer-local affine completion strictly after one source boundary.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::{DefinitionIndex, candidates};

mod completion;

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn prove_from_root_after(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    minimum_axiom: usize,
    root_bound: ProofNode,
) -> Option<ProofNode> {
    candidates::find_after(
        context,
        goal,
        semantic_axioms,
        definitions,
        root,
        minimum_axiom,
        |witness| {
            completion::prove(
                context,
                goal,
                assumptions,
                semantic_axioms,
                minimum_axiom,
                &root_bound,
                witness,
            )
        },
    )
}
