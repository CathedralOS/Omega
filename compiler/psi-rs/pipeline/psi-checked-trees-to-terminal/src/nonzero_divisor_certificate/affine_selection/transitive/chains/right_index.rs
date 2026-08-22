//! Source-ordered right-leg index for affine certificate production.

use std::collections::BTreeMap;

use psi_core::{Proposition, ScalarTerm};

use super::super::super::super::integer_evidence::{Citation, cited_facts};

pub(super) struct RightLegIndex<'a> {
    by_left_endpoint: BTreeMap<ScalarTerm, Vec<(Citation, &'a Proposition)>>,
}

impl<'a> RightLegIndex<'a> {
    pub(super) fn new(assumptions: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        let mut by_left_endpoint = BTreeMap::<_, Vec<_>>::new();
        for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
            let Proposition::LessOrEqual(left, _) = fact else {
                continue;
            };
            if matches!(left, ScalarTerm::Value { .. }) {
                by_left_endpoint
                    .entry(left.clone())
                    .or_default()
                    .push((citation, fact));
            }
        }
        Self { by_left_endpoint }
    }

    pub(super) fn candidates(&self, left_endpoint: &ScalarTerm) -> &[(Citation, &'a Proposition)] {
        self.by_left_endpoint
            .get(left_endpoint)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}
