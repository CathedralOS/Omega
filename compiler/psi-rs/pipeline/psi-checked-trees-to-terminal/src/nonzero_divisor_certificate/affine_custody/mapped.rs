//! Producer-local exact affine mapping strictly before one source boundary.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::ProofNode;

use super::{DefinitionIndex, candidates};

mod completion;

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn prove_mapped_to_target_before(
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
