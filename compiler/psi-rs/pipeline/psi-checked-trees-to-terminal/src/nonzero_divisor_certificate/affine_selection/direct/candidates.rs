//! Source-ordered direct retained-bound candidates for certificate production.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::integer_evidence::{Citation, cited_facts};

pub(super) fn find<'a, T>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut complete: impl FnMut(&'a ScalarTerm, &'a Proposition, Citation) -> Option<T>,
) -> Option<T> {
    for (citation, root_bound) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(root_left, root_right) = root_bound else {
            continue;
        };
        for root in [root_left, root_right]
            .into_iter()
            .filter(|root| matches!(root, ScalarTerm::Value { .. }))
        {
            if let Some(result) = complete(root, root_bound, citation) {
                return Some(result);
            }
        }
    }
    None
}
