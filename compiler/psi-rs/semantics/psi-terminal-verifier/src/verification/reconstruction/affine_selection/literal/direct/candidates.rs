//! Source-ordered direct landed-literal candidates for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::equalities;

pub(super) fn any<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut complete: impl FnMut(&'a ScalarTerm, &'a ScalarTerm) -> bool,
) -> bool {
    equalities::exact_value_bindings(requirements, semantic_axioms)
        .any(|(_, root, literal)| complete(root, literal))
}
