//! Producer-local completion of one post-boundary affine witness.

use proof_admission::{IntegerAffineWitness, ProofNode};
use semantic_vocabulary::{Proposition, PropositionContext};

use super::super::DefinitionIndex;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    minimum_axiom: usize,
    root_bound: &ProofNode,
    witness: IntegerAffineWitness,
) -> Option<ProofNode> {
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
        super::super::completion::prove(
            context,
            goal,
            assumptions,
            semantic_axioms,
            definitions,
            root_bound,
            witness,
        )
    })
    .flatten()
}
