//! Source-ordered direct landed-literal candidates for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

mod eligibility;
mod equalities;

use equalities::OrientedEqualities;

pub(super) struct DirectLiteralCandidates<'a> {
    equalities: OrientedEqualities<'a>,
}

impl<'a> DirectLiteralCandidates<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            equalities: OrientedEqualities::new(requirements, semantic_axioms),
        }
    }

    pub(super) fn any(
        &self,
        mut complete: impl FnMut(&'a ScalarTerm, &'a ScalarTerm) -> bool,
    ) -> bool {
        self.equalities
            .any(|root, literal| eligibility::eligible(root, literal) && complete(root, literal))
    }
}
