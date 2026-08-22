//! Fixed-depth value-alias transport for canonical order certificates.
//!
//! The one- and two-alias shapes are intentionally separate entry points. This
//! module exposes neither a hop-count parameter nor recursive graph search.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::ProofNode;

mod cast;
mod index;
mod one;
mod two;

use index::distinct_same_carrier_values;

pub(super) use cast::{prove_landed_literal_cast, prove_stronger_cast};

pub(super) fn prove_one(
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    complete: impl FnMut(&ScalarTerm, ProofNode) -> Option<ProofNode>,
) -> Option<ProofNode> {
    one::prove(assumptions, semantic_axioms, complete)
}

pub(super) fn prove_two(
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    complete: impl FnMut(&ScalarTerm, ProofNode) -> Option<ProofNode>,
) -> Option<ProofNode> {
    two::prove(assumptions, semantic_axioms, complete)
}
