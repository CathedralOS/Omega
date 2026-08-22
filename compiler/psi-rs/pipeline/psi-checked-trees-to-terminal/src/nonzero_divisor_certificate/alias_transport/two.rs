//! Exactly two value-alias substitutions for canonical order production.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::ProofNode;

mod candidates;
mod completion;

pub(super) fn prove(
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    complete: impl FnMut(&ScalarTerm, ProofNode) -> Option<ProofNode>,
) -> Option<ProofNode> {
    candidates::find(assumptions, semantic_axioms, complete)
}
