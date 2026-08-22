//! Source-ordered one-alias transitive candidates for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

use super::super::TwoCitationChains;

pub(super) struct AliasedTransitiveCandidates<'a> {
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    chains: TwoCitationChains<'a>,
}

impl<'a> AliasedTransitiveCandidates<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            requirements,
            semantic_axioms,
            chains: TwoCitationChains::new(requirements, semantic_axioms),
        }
    }

    pub(super) fn any(
        &self,
        mut complete: impl FnMut(&'a ScalarTerm, &'a ScalarTerm, &'a ScalarTerm, &'a ScalarTerm) -> bool,
    ) -> bool {
        self.requirements
            .iter()
            .chain(self.semantic_axioms)
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
                    self.chains.any(|left_fact, right_fact| {
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
}
