//! Source-ordered one-alias transitive candidates for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

use super::super::TwoCitationChains;

pub(super) fn any<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut complete: impl FnMut(&'a ScalarTerm, &'a ScalarTerm, &'a ScalarTerm, &'a ScalarTerm) -> bool,
) -> bool {
    let chains = TwoCitationChains::new(requirements, semantic_axioms);
    requirements
        .iter()
        .chain(semantic_axioms)
        .filter_map(|equality| match equality {
            Proposition::Equal(left, right) => Some((left, right)),
            _ => None,
        })
        .any(|(equality_left, equality_right)| {
            [
                (equality_left, equality_right),
                (equality_right, equality_left),
            ]
            .into_iter()
            .filter(|(root, alias)| {
                root != alias
                    && matches!(root, ScalarTerm::Value { .. })
                    && matches!(alias, ScalarTerm::Value { .. })
            })
            .any(|(root, alias)| {
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
        })
}
