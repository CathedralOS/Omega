//! Source-ordered direct retained-bound candidates for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

use super::super::bounds;

pub(super) fn any<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut complete: impl FnMut(&'a ScalarTerm, &'a Proposition) -> bool,
) -> bool {
    bounds::ordered(requirements, semantic_axioms).any(|(root_bound, root_left, root_right)| {
        bounds::value_endpoints(root_left, root_right).any(|root| complete(root, root_bound))
    })
}
