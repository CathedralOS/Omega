//! Exactly one value-alias substitution for canonical order production.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, ScalarTerm};

mod candidates;
mod completion;

pub(super) fn prove(
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    complete: impl FnMut(&ScalarTerm, ProofNode) -> Option<ProofNode>,
) -> Option<ProofNode> {
    candidates::prove(assumptions, semantic_axioms, complete)
}
