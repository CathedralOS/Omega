//! Source-ordered left legs for independent two-citation reconstruction.

use psi_core::{Proposition, ScalarTerm};

pub(super) struct LeftLegs<'a> {
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
}

impl<'a> LeftLegs<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            requirements,
            semantic_axioms,
        }
    }

    pub(super) fn any(
        &self,
        mut join: impl FnMut(&'a Proposition, &'a ScalarTerm) -> bool,
    ) -> bool {
        self.requirements
            .iter()
            .chain(self.semantic_axioms)
            .filter_map(|fact| match fact {
                Proposition::LessOrEqual(_, middle)
                    if matches!(middle, ScalarTerm::Value { .. }) =>
                {
                    Some((fact, middle))
                }
                _ => None,
            })
            .any(|(fact, middle)| join(fact, middle))
    }
}
