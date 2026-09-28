//! Exactly two value-alias substitutions for canonical order production.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, ScalarTerm};

mod candidates;
mod completion;

pub(super) fn prove(
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    complete: impl FnMut(&ScalarTerm, ProofNode) -> Option<ProofNode>,
) -> Option<ProofNode> {
    candidates::find(assumptions, semantic_axioms, complete)
}
