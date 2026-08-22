//! Source-ordered left legs for independent two-citation reconstruction.

use psi_core::{Proposition, ScalarTerm};

pub(super) fn any<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut join: impl FnMut(&'a Proposition, &'a ScalarTerm) -> bool,
) -> bool {
    requirements
        .iter()
        .chain(semantic_axioms)
        .filter_map(|fact| match fact {
            Proposition::LessOrEqual(_, middle) if matches!(middle, ScalarTerm::Value { .. }) => {
                Some((fact, middle))
            }
            _ => None,
        })
        .any(|(fact, middle)| join(fact, middle))
}
