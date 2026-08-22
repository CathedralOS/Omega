//! Source-ordered left legs for independent two-citation reconstruction.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::{bounds, eligibility};

pub(super) fn any<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut join: impl FnMut(&'a Proposition, &'a ScalarTerm) -> bool,
) -> bool {
    bounds::ordered(requirements, semantic_axioms)
        .filter(|(_, _, middle)| eligibility::is_value(middle))
        .map(|(fact, _, middle)| (fact, middle))
        .any(|(fact, middle)| join(fact, middle))
}
