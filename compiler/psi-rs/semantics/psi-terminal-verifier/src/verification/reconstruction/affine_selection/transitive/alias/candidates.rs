//! Source-ordered one-alias transitive candidates for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::{eligibility, equalities};
use super::super::TwoCitationChains;

pub(super) fn any<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut complete: impl FnMut(&'a ScalarTerm, &'a ScalarTerm, &'a ScalarTerm, &'a ScalarTerm) -> bool,
) -> bool {
    let chains = TwoCitationChains::new(requirements, semantic_axioms);
    equalities::ordered(requirements, semantic_axioms)
        .filter(|(_, root, alias)| eligibility::distinct_value_alias(root, alias))
        .any(|(_, root, alias)| chains.any(|left, right| complete(root, alias, left, right)))
}
