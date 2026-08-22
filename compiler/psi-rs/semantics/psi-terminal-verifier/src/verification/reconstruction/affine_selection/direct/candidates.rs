//! Source-ordered direct retained-bound candidates for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

use super::super::eligibility;

pub(super) fn any<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut complete: impl FnMut(&'a ScalarTerm, &'a Proposition) -> bool,
) -> bool {
    requirements
        .iter()
        .chain(semantic_axioms)
        .filter_map(|fact| match fact {
            Proposition::LessOrEqual(left, right) => Some((fact, left, right)),
            _ => None,
        })
        .any(|(root_bound, root_left, root_right)| {
            eligibility::ordered_value_endpoints(root_left, root_right)
                .any(|root| complete(root, root_bound))
        })
}
