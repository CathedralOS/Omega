//! Identity-preserving scalar value flow, without arithmetic denotation.
//! Every predecessor must supply the same typed identity. Unknowns and cycles
//! withhold evidence; one invocation memoizes the immutable graph it observes.

use semantic_vocabulary::ValueId;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
pub enum UnchangedScalarOrigin<Identity> {
    Known(Identity),
    Incoming(Vec<ValueId>),
}

/// Completed answers belong to one immutable origin graph, including unknowns.
/// A cycle never supplies evidence. A new graph must start a new query table.
pub(crate) struct UnchangedScalarIdentities<Identity> {
    completed: BTreeMap<ValueId, Option<Identity>>,
    active: BTreeSet<ValueId>,
}

impl<Identity> Default for UnchangedScalarIdentities<Identity> {
    fn default() -> Self {
        Self {
            completed: BTreeMap::new(),
            active: BTreeSet::new(),
        }
    }
}

impl<Identity: Clone + Eq> UnchangedScalarIdentities<Identity> {
    pub(crate) fn resolve(
        &mut self,
        value: ValueId,
        origin: &mut impl FnMut(ValueId) -> Option<UnchangedScalarOrigin<Identity>>,
    ) -> Option<Identity> {
        if let Some(result) = self.completed.get(&value) {
            return result.clone();
        }
        if !self.active.insert(value) {
            return None;
        }
        let result = (|| match origin(value)? {
            UnchangedScalarOrigin::Known(identity) => Some(identity),
            UnchangedScalarOrigin::Incoming(values) => {
                let mut values = values.into_iter();
                let first = self.resolve(values.next()?, origin)?;
                for value in values {
                    if self.resolve(value, origin)? != first {
                        return None;
                    }
                }
                Some(first)
            }
        })();
        self.active.remove(&value);
        self.completed.insert(value, result.clone());
        result
    }
}

pub fn unchanged_scalar_identity<Identity: Clone + Eq>(
    value: ValueId,
    mut origin: impl FnMut(ValueId) -> Option<UnchangedScalarOrigin<Identity>>,
) -> Option<Identity> {
    UnchangedScalarIdentities::default().resolve(value, &mut origin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_constant_queries_share_reconverging_origins_across_roots() {
        let id = |n| ValueId::new(n).unwrap();
        let mut calls = 0;
        let mut query = UnchangedScalarIdentities::default();
        for root in (1..=512).rev() {
            assert_eq!(
                query.resolve(id(root), &mut |value| {
                    calls += 1;
                    Some(if value == id(1) {
                        UnchangedScalarOrigin::Known(7)
                    } else {
                        UnchangedScalarOrigin::Incoming(vec![id(value.get() - 1); 2])
                    })
                }),
                Some(7)
            );
        }
        assert_eq!(
            calls, 512,
            "one origin observation per immutable graph value"
        );
    }

    #[test]
    fn scalar_constant_queries_preserve_cycles_unknowns_and_disagreement() {
        let id = |n| ValueId::new(n).unwrap();
        let mut query = UnchangedScalarIdentities::default();
        let mut origin = |value: ValueId| {
            Some(match value.get() {
                1 => UnchangedScalarOrigin::Known(7),
                2 => UnchangedScalarOrigin::Known(9),
                3 => UnchangedScalarOrigin::Incoming(vec![id(1), id(2)]),
                4 => UnchangedScalarOrigin::Incoming(vec![id(5), id(1)]),
                5 => UnchangedScalarOrigin::Incoming(vec![id(4)]),
                6 => UnchangedScalarOrigin::Incoming(Vec::new()),
                _ => return None,
            })
        };
        for root in [3, 4, 5, 6, u64::MAX] {
            assert_eq!(query.resolve(id(root), &mut origin), None);
        }
        assert_eq!(query.resolve(id(1), &mut origin), Some(7));
        assert_eq!(query.resolve(id(2), &mut origin), Some(9));
        // Reordering roots cannot turn a cycle or a conflicting predecessor
        // into evidence by borrowing another completed query's answer.
        for root in [u64::MAX, 6, 5, 4, 3] {
            assert_eq!(query.resolve(id(root), &mut origin), None);
        }
        assert_eq!(
            UnchangedScalarIdentities::default()
                .resolve(id(1), &mut |_| Some(UnchangedScalarOrigin::Known(42))),
            Some(42)
        );
    }
}
