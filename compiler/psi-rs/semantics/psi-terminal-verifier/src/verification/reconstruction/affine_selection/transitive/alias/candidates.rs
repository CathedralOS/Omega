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
    equalities::ordered(requirements, semantic_axioms)
        .filter(|(_, root, alias)| {
            root != alias
                && matches!(root, ScalarTerm::Value { .. })
                && matches!(alias, ScalarTerm::Value { .. })
        })
        .any(|(_, root, alias)| {
            chains.any(|left_fact, right_fact| {
                let Proposition::LessOrEqual(left, _) = left_fact else {
                    unreachable!("only integer chains are enumerated")
                };
                let Proposition::LessOrEqual(_, right) = right_fact else {
                    unreachable!("only integer chains are enumerated")
                };
                complete(root, alias, left, right)
            })
        })
}
