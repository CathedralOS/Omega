//! Identity-preserving scalar value flow, without arithmetic denotation.
//! Every predecessor must supply the same typed identity. Unknowns and cycles
//! withhold evidence; one invocation owns the immutable graph it observes.

use semantic_vocabulary::ValueId;

#[derive(Clone)]
pub enum UnchangedScalarOrigin<Identity> {
    Known(Identity),
    Incoming(Vec<ValueId>),
}

// The callback can expose sparse or absent IDs, including IDs outside any
// machine's declarations. Keep only observed identifiers in a flat ordered
// roster; a row's state belongs to this immutable graph, not another query.
enum ScalarIdentityState<Identity> {
    Active,
    Complete(Option<Identity>),
}

pub(crate) struct UnchangedScalarIdentities<Identity> {
    rows: Vec<(ValueId, ScalarIdentityState<Identity>)>,
}

impl<Identity> Default for UnchangedScalarIdentities<Identity> {
    fn default() -> Self {
        Self { rows: Vec::new() }
    }
}

impl<Identity: Clone + Eq> UnchangedScalarIdentities<Identity> {
    pub(crate) fn resolve(
        &mut self,
        value: ValueId,
        origin: &mut impl FnMut(ValueId) -> Option<UnchangedScalarOrigin<Identity>>,
    ) -> Option<Identity> {
        match self.rows.binary_search_by_key(&value, |(id, _)| *id) {
            Ok(index) => {
                return match &self.rows[index].1 {
                    ScalarIdentityState::Active => None,
                    ScalarIdentityState::Complete(result) => result.clone(),
                };
            }
            Err(index) => self
                .rows
                .insert(index, (value, ScalarIdentityState::Active)),
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
        // Descendants can insert earlier IDs; locate this retained row again.
        let index = self
            .rows
            .binary_search_by_key(&value, |(id, _)| *id)
            .expect("the active scalar identity row is retained");
        self.rows[index].1 = ScalarIdentityState::Complete(result.clone());
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
    fn sparse_external_identities_do_not_allocate_through_their_numeric_value() {
        let high = ValueId::new(u64::MAX).unwrap();
        let low = ValueId::new(1).unwrap();
        let absent = ValueId::new(u64::MAX - 1).unwrap();
        let mut query = UnchangedScalarIdentities::default();
        assert_eq!(
            query.resolve(high, &mut |_| Some(UnchangedScalarOrigin::Known(42))),
            Some(42)
        );
        assert_eq!(
            query.resolve(low, &mut |_| Some(UnchangedScalarOrigin::Known(7))),
            Some(7)
        );
        assert_eq!(
            query.resolve(high, &mut |_| panic!("completed identity is observed once")),
            Some(42)
        );
        assert_eq!(query.resolve(absent, &mut |_| None), None);
        assert_eq!(
            query.resolve(absent, &mut |_| panic!("missing identity is observed once")),
            None
        );
    }

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
