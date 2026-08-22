//! Source-ordered left legs for independent two-citation reconstruction.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::eligibility;

pub(super) fn any<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut join: impl FnMut(&'a Proposition, &'a ScalarTerm) -> bool,
) -> bool {
    requirements
        .iter()
        .chain(semantic_axioms)
        .filter_map(|fact| match fact {
            Proposition::LessOrEqual(_, middle) if eligibility::is_value(middle) => {
                Some((fact, middle))
            }
            _ => None,
        })
        .any(|(fact, middle)| join(fact, middle))
}
