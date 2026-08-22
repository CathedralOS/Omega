//! Source-ordered left legs for affine certificate production.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::super::integer_evidence::Citation;
use super::super::super::bounds;

pub(super) fn find<'a, T>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut join: impl FnMut(Citation, &'a Proposition, &'a ScalarTerm, &'a ScalarTerm) -> Option<T>,
) -> Option<T> {
    bounds::with_value_right(assumptions, semantic_axioms)
        .find_map(|(citation, fact, left, middle)| join(citation, fact, left, middle))
}
