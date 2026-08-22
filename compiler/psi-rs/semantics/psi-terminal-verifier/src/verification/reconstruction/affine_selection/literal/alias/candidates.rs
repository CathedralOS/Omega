//! Source-ordered one-alias literal candidates for independent reconstruction.

use std::collections::BTreeMap;

use psi_core::{Proposition, ScalarTerm, ScalarType};

pub(super) struct LiteralAliasCandidates<'a> {
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    literals_by_alias: BTreeMap<ScalarTerm, Vec<(&'a Proposition, &'a ScalarTerm)>>,
}

impl<'a> LiteralAliasCandidates<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        let mut literals_by_alias = BTreeMap::<_, Vec<_>>::new();
        for equality in requirements.iter().chain(semantic_axioms) {
            let Proposition::Equal(left, right) = equality else {
                continue;
            };
            for (alias, literal) in [(left, right), (right, left)] {
                if matches!(alias, ScalarTerm::Value { .. }) && literal.integer_value().is_some() {
                    literals_by_alias
                        .entry(alias.clone())
                        .or_default()
                        .push((equality, literal));
                }
            }
        }
        Self {
            requirements,
            semantic_axioms,
            literals_by_alias,
        }
    }

    pub(super) fn any(
        &self,
        mut complete: impl FnMut(&'a ScalarTerm, &'a ScalarTerm) -> bool,
    ) -> bool {
        for outer_equality in self.requirements.iter().chain(self.semantic_axioms) {
            let Proposition::Equal(outer_left, outer_right) = outer_equality else {
                continue;
            };
            for (root, alias) in [(outer_left, outer_right), (outer_right, outer_left)] {
                if root == alias
                    || !matches!(root, ScalarTerm::Value { .. })
                    || !matches!(alias, ScalarTerm::Value { .. })
                    || root.scalar_type() != alias.scalar_type()
                {
                    continue;
                }
                let Some(landings) = self.literals_by_alias.get(alias) else {
                    continue;
                };
                for &(inner_equality, literal) in landings {
                    if std::ptr::eq(outer_equality, inner_equality) {
                        continue;
                    }
                    let Some((integer_type, _)) = literal.integer_value() else {
                        unreachable!("literal index contains only integer landings")
                    };
                    if root.scalar_type() == ScalarType::Integer(integer_type)
                        && complete(root, literal)
                    {
                        return true;
                    }
                }
            }
        }
        false
    }
}
