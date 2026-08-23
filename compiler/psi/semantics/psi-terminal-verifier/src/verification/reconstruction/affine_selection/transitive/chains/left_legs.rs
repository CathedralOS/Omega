//! Source-ordered left legs for independent two-citation reconstruction.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::bounds;

pub(super) fn any<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut join: impl FnMut(&'a Proposition, &'a ScalarTerm, &'a ScalarTerm) -> bool,
) -> bool {
    bounds::with_value_right(requirements, semantic_axioms)
        .any(|(fact, left, middle)| join(fact, left, middle))
}
