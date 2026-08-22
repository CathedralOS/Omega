//! Producer-local affine completion strictly after one source boundary.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::ProofNode;

use super::{DefinitionIndex, candidates, completion};

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn prove_from_root_after(
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
