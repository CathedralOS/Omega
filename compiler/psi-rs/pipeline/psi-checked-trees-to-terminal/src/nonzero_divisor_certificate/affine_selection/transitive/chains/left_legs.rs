//! Source-ordered left legs for affine certificate production.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::super::integer_evidence::Citation;
use super::super::super::{bounds, eligibility};

pub(super) fn find<'a, T>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut join: impl FnMut(Citation, &'a Proposition, &'a ScalarTerm) -> Option<T>,
) -> Option<T> {
    for (citation, fact, _, middle) in bounds::ordered(assumptions, semantic_axioms) {
        if !eligibility::is_value(middle) {
            continue;
        }
        if let Some(result) = join(citation, fact, middle) {
            return Some(result);
        }
    }
    None
}
