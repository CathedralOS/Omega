//! Independent exact retained proposition reconstruction.

use psi_core::Proposition;

pub(super) fn retained(
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    requirements
        .iter()
        .chain(semantic_axioms)
        .any(|fact| fact == goal)
}
