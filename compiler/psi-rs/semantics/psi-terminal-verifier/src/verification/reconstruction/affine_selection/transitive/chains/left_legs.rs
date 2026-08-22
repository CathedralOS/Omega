//! Source-ordered left legs for independent two-citation reconstruction.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::bounds;

pub(super) fn any<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut join: impl FnMut(&'a Proposition, &'a ScalarTerm, &'a ScalarTerm) -> bool,
) -> bool {
    bounds::ordered(requirements, semantic_axioms)
        .filter(|(_, _, middle)| bounds::is_value(middle))
        .any(|(fact, left, middle)| join(fact, left, middle))
}
