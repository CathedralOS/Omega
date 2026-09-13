//! Producer-local exact affine mapping strictly before one source boundary.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::{DefinitionIndex, candidates};

mod completion;

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn prove_mapped_to_target_before(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    target: &ScalarTerm,
    maximum_axiom: usize,
    root_bound: &ProofNode,
) -> Option<ProofNode> {
    candidates::find_target_before(
        context,
        semantic_axioms,
        definitions,
        root,
        target,
        maximum_axiom,
        |witness| {
            completion::prove(
                context,
                assumptions,
                semantic_axioms,
                maximum_axiom,
                root_bound,
                witness,
            )
        },
    )
}
