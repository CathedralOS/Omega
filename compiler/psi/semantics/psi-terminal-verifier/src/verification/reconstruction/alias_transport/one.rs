//! Independent exactly-one-alias retained order selection.

use psi_core::{Proposition, ScalarTerm};

mod candidates;
mod completion;

pub(super) fn retained(
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    complete: impl FnMut(&ScalarTerm, &Proposition) -> bool,
) -> bool {
    candidates::retained(requirements, semantic_axioms, complete)
}
