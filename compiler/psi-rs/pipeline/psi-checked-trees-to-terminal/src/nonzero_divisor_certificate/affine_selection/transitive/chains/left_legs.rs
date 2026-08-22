//! Source-ordered left legs for affine certificate production.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::super::integer_evidence::{Citation, cited_facts};

pub(super) fn find<'a, T>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut join: impl FnMut(Citation, &'a Proposition, &'a ScalarTerm) -> Option<T>,
) -> Option<T> {
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(_, middle) = fact else {
            continue;
        };
        if !matches!(middle, ScalarTerm::Value { .. }) {
            continue;
        }
        if let Some(result) = join(citation, fact, middle) {
            return Some(result);
        }
    }
    None
}
