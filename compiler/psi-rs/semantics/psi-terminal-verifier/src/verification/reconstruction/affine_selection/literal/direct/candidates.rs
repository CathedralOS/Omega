//! Source-ordered direct landed-literal candidates for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

use super::super::{eligibility, equalities::OrientedEqualities};

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
        self.equalities.any(|_, root, literal| {
            eligibility::exact_value_binding(root, literal) && complete(root, literal)
        })
    }
}
