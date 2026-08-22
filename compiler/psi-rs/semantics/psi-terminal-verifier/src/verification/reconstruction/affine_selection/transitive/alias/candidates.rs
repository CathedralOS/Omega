//! Source-ordered one-alias transitive candidates for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::equalities;
use super::super::TwoCitationChains;

pub(super) fn any<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut complete: impl FnMut(&'a ScalarTerm, &'a ScalarTerm, &'a ScalarTerm, &'a ScalarTerm) -> bool,
) -> bool {
    let chains = TwoCitationChains::new(requirements, semantic_axioms);
    equalities::value_aliases(requirements, semantic_axioms)
        .any(|(_, root, alias)| chains.any(|left, right| complete(root, alias, left, right)))
}
