//! One row per value that a proof context's facts name, not one per value of
//! the machine.
//!
//! A machine's value namespace can be very large. system's Windows removal walk
//! lends a `[WindowsRemovalFrame; 4096]` through every join of its nested
//! machines, and each join snapshots every element, so one checked machine
//! has about 66,000 values. The facts of one proof context name a few
//! hundred of them. The prover used to give every context, and every case
//! branch it opened, dense rows over the whole namespace: 400 to 600 contexts
//! per machine, each allocating and filling several 66,000-row tables. In a
//! copy of that walk each machine took 1.2–2.3 GB and 7–9 seconds to check,
//! and checking system for four targets at once reached 48.7 GB.
//!
//! `ValueRows` keeps a dense row for each value in the context's sorted
//! family, found by binary search, and a sorted overflow for any other value
//! a query writes. Every value of the namespace still reads as present: a
//! value without a row reads as `default`, exactly as a freshly filled dense
//! row would, and a value outside the namespace reads as absent.

use std::ops::Range;
use std::sync::Arc;

use semantic_vocabulary::{Proposition, ValueId};

use super::ProofValues;

/// The values a context's facts name, sorted and without repeats, inside
/// the namespace of values the proof owner supplies.
#[derive(Clone, Debug, Default)]
pub(crate) struct ValueFamily {
    namespace: Range<u64>,
    members: Arc<[ValueId]>,
}

impl ValueFamily {
    /// Every value `propositions` name. A proposition whose visit is
    /// incomplete adds what it exposed; any value it hides still gets a
    /// row in the overflow the first time a query writes it.
    pub(crate) fn of<'a>(
        values: &dyn ProofValues,
        propositions: impl IntoIterator<Item = &'a Proposition>,
    ) -> Self {
        let namespace = match values.identity(0) {
            Some(first) => first.get()..first.get().saturating_add(values.count() as u64),
            None => 0..0,
        };
        let mut members = Vec::new();
        for proposition in propositions {
            proposition.visit_value_ids(|value| {
                if namespace.contains(&value.get()) {
                    members.push(value);
                }
            });
        }
        members.sort_unstable();
        members.dedup();
        Self {
            namespace,
            members: members.into(),
        }
    }

    pub(crate) fn rows<T: Clone>(&self, default: T) -> ValueRows<T> {
        ValueRows {
            family: self.clone(),
            rows: vec![default.clone(); self.members.len()],
            overflow: Vec::new(),
            default,
        }
    }
}

/// A row for each value of the namespace; see the module comment.
#[derive(Clone, Debug)]
pub(crate) struct ValueRows<T> {
    family: ValueFamily,
    /// One row per family member, in member order.
    rows: Vec<T>,
    /// Rows written for values outside the family, sorted by value.
    overflow: Vec<(ValueId, T)>,
    default: T,
}

impl<T: Clone + Default> Default for ValueRows<T> {
    /// An empty namespace: every value reads as absent.
    fn default() -> Self {
        ValueFamily::default().rows(T::default())
    }
}

impl<T: Clone> ValueRows<T> {
    pub(crate) fn get(&self, value: ValueId) -> Option<&T> {
        if !self.family.namespace.contains(&value.get()) {
            return None;
        }
        if let Ok(position) = self.family.members.binary_search(&value) {
            return self.rows.get(position);
        }
        Some(
            match self
                .overflow
                .binary_search_by_key(&value, |(value, _)| *value)
            {
                Ok(position) => &self.overflow[position].1,
                Err(_) => &self.default,
            },
        )
    }

    pub(crate) fn get_mut(&mut self, value: ValueId) -> Option<&mut T> {
        if !self.family.namespace.contains(&value.get()) {
            return None;
        }
        if let Ok(position) = self.family.members.binary_search(&value) {
            return self.rows.get_mut(position);
        }
        let position = match self
            .overflow
            .binary_search_by_key(&value, |(value, _)| *value)
        {
            Ok(position) => position,
            Err(position) => {
                self.overflow
                    .insert(position, (value, self.default.clone()));
                position
            }
        };
        Some(&mut self.overflow[position].1)
    }

    /// Fresh rows over the same family, each reading `default`.
    pub(crate) fn like<U: Clone>(&self, default: U) -> ValueRows<U> {
        self.family.rows(default)
    }

    /// How many rows this table holds: one per family member, and one per
    /// other value a query wrote.
    #[cfg(test)]
    pub(crate) fn row_count(&self) -> usize {
        self.rows.len() + self.overflow.len()
    }

    /// Each family member and its row, in value order. Values outside the
    /// family are not listed.
    #[cfg(test)]
    pub(crate) fn members(&self) -> impl Iterator<Item = (ValueId, &T)> {
        self.family.members.iter().copied().zip(&self.rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Namespace(std::ops::RangeInclusive<u64>);

    impl ProofValues for Namespace {
        fn count(&self) -> usize {
            self.0.clone().count()
        }
        fn identity(&self, position: usize) -> Option<ValueId> {
            ValueId::new(self.0.start() + position as u64).filter(|id| self.0.contains(&id.get()))
        }
        fn scalar_type(&self, _: ValueId) -> Option<semantic_vocabulary::ScalarType> {
            None
        }
    }

    fn value(raw: u64) -> ValueId {
        ValueId::new(raw).unwrap()
    }

    fn named(raw: u64) -> Proposition {
        Proposition::Equal(
            semantic_vocabulary::ScalarTerm::value(
                value(raw),
                semantic_vocabulary::ScalarType::Boolean,
            ),
            semantic_vocabulary::ScalarTerm::Boolean(true),
        )
    }

    #[test]
    fn a_value_without_a_row_reads_as_a_fresh_dense_row() {
        // Facts name 7 and 3 out of the namespace 1..=100 000.
        let facts = [named(7), named(3), named(7)];
        let family = ValueFamily::of(&Namespace(1..=100_000), &facts);
        let mut rows = family.rows(None::<usize>);
        assert_eq!(rows.rows.len(), 2, "only the named values get dense rows");
        assert_eq!(rows.get(value(50_000)), Some(&None));
        assert_eq!(rows.get(value(100_001)), None);
        *rows.get_mut(value(7)).unwrap() = Some(1);
        *rows.get_mut(value(90_000)).unwrap() = Some(2);
        *rows.get_mut(value(40)).unwrap() = Some(3);
        assert_eq!(rows.get(value(7)), Some(&Some(1)));
        assert_eq!(rows.get(value(90_000)), Some(&Some(2)));
        assert_eq!(rows.get(value(40)), Some(&Some(3)));
        assert_eq!(rows.get(value(3)), Some(&None));
        assert!(rows.get_mut(value(100_001)).is_none());
        let flags = rows.like(false);
        assert_eq!(flags.get(value(7)), Some(&false));
        assert_eq!(flags.get(value(90_000)), Some(&false));
        assert_eq!(
            rows.members()
                .map(|(value, _)| value.get())
                .collect::<Vec<_>>(),
            [3, 7]
        );
    }

    #[test]
    fn an_empty_namespace_has_no_values() {
        let rows = ValueRows::<Option<usize>>::default();
        assert_eq!(rows.get(value(1)), None);
    }
}
