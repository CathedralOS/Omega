//! Producer-local affine custody for one completed fixed-depth alias walk.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::affine_custody::{self, DefinitionIndex};

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    root_bound: ProofNode,
) -> Option<ProofNode> {
    affine_custody::prove_from_root(
        context,
        goal,
        assumptions,
        semantic_axioms,
        definitions,
        root,
        root_bound,
    )
}
