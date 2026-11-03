//! One history query's scalar premises, each stored once.
//!
//! A payload history replays the same few propositions at every position it
//! visits: it copies them into each successor, intersects them at joins, sorts
//! them after every transfer and substitutes them across every block arrival.
//! Comparing and cloning whole expression trees for each of those steps was
//! most of the history's cost. The table interns every premise the replay
//! creates, so two rows are equal exactly when they are the same row, and a
//! roster copy copies handles rather than trees.
//!
//! Published rosters still follow the complete proposition order, because
//! readers (proof inputs, payload contracts) observe that order. Each row
//! carries a rank whose order matches the proposition order of every other
//! row in its table. Comparing two handles compares two integers. Inserting a
//! row between two adjacent ranks with no gap renumbers the table, preserving
//! every relative order, so existing sorted rosters stay sorted.
//!
//! Handles from different tables are never compared: each table belongs to
//! one history and its retained snapshots.

use semantic_vocabulary::{Proposition, ScalarTerm, ValueId};
use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use crate::substitution::{substitute_proposition_values, substitute_scalar_term_values};

/// Gap left between adjacent ranks after renumbering. Ordinary replay
/// appends new premises after older ones, which never consumes a gap.
const RANK_SPACING: u64 = 1 << 20;

/// A handle to one interned premise. Equality is identity within the table and
/// order is the row's rank, which agrees with the proposition order.
#[derive(Clone)]
pub(in crate::descriptors::descriptor_results) struct SharedPremise(Arc<PremiseRow>);

struct PremiseRow {
    proposition: Proposition,
    /// Every value the proposition names, sorted and unique.
    values: Vec<ValueId>,
    /// Whether the proposition names no structural or opaque identity.
    closed: bool,
    /// Whether substitution leaves this proposition unchanged when it names
    /// none of the substituted values. Substitution also normalizes a few
    /// forms, such as the operand order of symmetric float comparisons.
    substitution_stable: bool,
    /// Position in the table's proposition order. Only renumbering changes it,
    /// and renumbering preserves the order of every pair of rows.
    rank: AtomicU64,
}

impl SharedPremise {
    fn rank(&self) -> u64 {
        self.0.rank.load(AtomicOrdering::Relaxed)
    }

    pub(super) fn mentions_any(&self, definitions: &[ValueId]) -> bool {
        definitions
            .iter()
            .any(|value| self.0.values.binary_search(value).is_ok())
    }

    pub(super) fn survives_payload_retirement(&self, definitions: &[ValueId]) -> bool {
        self.0.closed && !self.mentions_any(definitions)
    }

    #[cfg(test)]
    pub(super) fn new(proposition: Proposition) -> Self {
        thread_local! {
            static TABLE: std::cell::RefCell<PremiseTable> =
                std::cell::RefCell::new(PremiseTable::default());
        }
        TABLE.with(|table| table.borrow_mut().intern(proposition))
    }
}

impl Borrow<Proposition> for SharedPremise {
    fn borrow(&self) -> &Proposition {
        &self.0.proposition
    }
}

impl AsRef<Proposition> for SharedPremise {
    fn as_ref(&self) -> &Proposition {
        &self.0.proposition
    }
}

impl PartialEq for SharedPremise {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for SharedPremise {}
impl PartialOrd for SharedPremise {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for SharedPremise {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank().cmp(&other.rank())
    }
}

/// The interned premises of one history. The set is ordered by rank, which is
/// also proposition order, so it can be searched by proposition.
#[derive(Default)]
pub(in crate::descriptors::descriptor_results) struct PremiseTable {
    rows: BTreeSet<SharedPremise>,
}

impl PremiseTable {
    /// The table's row for this proposition, created on first use.
    pub(super) fn intern(&mut self, proposition: Proposition) -> SharedPremise {
        if let Some(existing) = self.rows.get(&proposition) {
            return existing.clone();
        }
        self.insert(proposition)
    }

    /// Like [`Self::intern`], cloning the proposition only for a new row.
    pub(super) fn intern_ref(&mut self, proposition: &Proposition) -> SharedPremise {
        if let Some(existing) = self.rows.get(proposition) {
            return existing.clone();
        }
        self.insert(proposition.clone())
    }

    fn insert(&mut self, proposition: Proposition) -> SharedPremise {
        let rank = match self.free_rank(&proposition) {
            Some(rank) => rank,
            None => {
                self.renumber();
                self.free_rank(&proposition)
                    .expect("renumbered premise ranks leave a gap between neighbors")
            }
        };
        let mut values = Vec::new();
        let closed = proposition.visit_value_ids(|value| values.push(value));
        values.sort_unstable();
        values.dedup();
        let substitution_stable =
            substitute_proposition_values(&proposition, &BTreeMap::new()) == proposition;
        let premise = SharedPremise(Arc::new(PremiseRow {
            proposition,
            values,
            closed,
            substitution_stable,
            rank: AtomicU64::new(rank),
        }));
        self.rows.insert(premise.clone());
        premise
    }

    /// A rank strictly between the proposition's ordered neighbors, if any.
    fn free_rank(&self, proposition: &Proposition) -> Option<u64> {
        let below = self
            .rows
            .range::<Proposition, _>((Bound::Unbounded, Bound::Excluded(proposition)))
            .next_back()
            .map(SharedPremise::rank);
        let above = self
            .rows
            .range::<Proposition, _>((Bound::Excluded(proposition), Bound::Unbounded))
            .next()
            .map(SharedPremise::rank);
        match (below, above) {
            (None, None) => Some(RANK_SPACING),
            (Some(below), None) => below.checked_add(RANK_SPACING),
            (below, Some(above)) => {
                let below = below.unwrap_or(0);
                (above - below >= 2).then(|| below + (above - below) / 2)
            }
        }
    }

    fn renumber(&mut self) {
        for (index, premise) in self.rows.iter().enumerate() {
            let rank = (index as u64 + 1) * RANK_SPACING;
            premise.0.rank.store(rank, AtomicOrdering::Relaxed);
        }
    }

    /// The premises that survive a block arrival, in the namespace of the
    /// arriving block. Each binding substitutes its actual value by the new
    /// formal; a premise naming a rebound formal without a substitute is
    /// dropped. Each binding also contributes its own equalities. A premise
    /// naming no substituted value keeps its row, because substitution would
    /// rebuild the same proposition.
    pub(super) fn arrival_requirements(
        &mut self,
        requirements: &[SharedPremise],
        bindings: &[(ValueId, ScalarTerm)],
        parameters: &[ValueId],
    ) -> Vec<SharedPremise> {
        let mut substitutions = BTreeMap::new();
        for (parameter, actual) in bindings {
            if let ScalarTerm::Value { id, scalar_type } = actual {
                substitutions
                    .entry(*id)
                    .or_insert_with(|| ScalarTerm::value(*parameter, *scalar_type));
            }
        }
        let surviving =
            |value: ValueId| !parameters.contains(&value) || substitutions.contains_key(&value);
        let mut result = Vec::with_capacity(requirements.len() + 2 * bindings.len());
        for premise in requirements {
            let row = &premise.0;
            if row.values.iter().any(|value| !surviving(*value)) {
                continue;
            }
            let unchanged = row.closed
                && row.substitution_stable
                && !row
                    .values
                    .iter()
                    .any(|value| substitutions.contains_key(value));
            result.push(if unchanged {
                premise.clone()
            } else {
                self.intern(substitute_proposition_values(
                    &row.proposition,
                    &substitutions,
                ))
            });
        }
        for (parameter, actual) in bindings {
            if actual.any_value_id(|value| !surviving(value)) {
                continue;
            }
            let value = substitute_scalar_term_values(actual, &substitutions);
            result.push(self.intern(Proposition::Equal(
                ScalarTerm::value(*parameter, value.scalar_type()),
                value,
            )));
            // Non-rebound names still denote the same captured incoming value.
            if !actual.any_value_id(|value| parameters.contains(&value)) {
                result.push(self.intern(Proposition::Equal(
                    ScalarTerm::value(*parameter, actual.scalar_type()),
                    actual.clone(),
                )));
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::ScalarType;

    fn condition(value: u64) -> Proposition {
        Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::value(ValueId::new(value).unwrap(), ScalarType::Boolean),
        )
    }

    #[test]
    fn handle_order_matches_proposition_order_through_renumbering() {
        let mut table = PremiseTable::default();
        // Alternate insertions at both ends and between the two most recent
        // neighbors, so some gaps close and force the table to renumber.
        let mut values = Vec::new();
        let (mut low, mut high) = (1_000u64, 1_001u64);
        for round in 0..80 {
            if round % 2 == 0 {
                low -= 1;
                values.push(low);
            } else {
                high += 1;
                values.push(high);
            }
        }
        values.extend((0..64).map(|step| 500 + step));
        let handles = values
            .iter()
            .map(|value| table.intern(condition(*value)))
            .collect::<Vec<_>>();
        let mut by_handle = handles.clone();
        by_handle.sort();
        let mut by_proposition = values
            .iter()
            .map(|value| condition(*value))
            .collect::<Vec<_>>();
        by_proposition.sort();
        assert_eq!(
            by_handle
                .iter()
                .map(|premise| premise.as_ref().clone())
                .collect::<Vec<_>>(),
            by_proposition
        );
        // Equal propositions share one row; different rows never compare equal.
        for (value, handle) in values.iter().zip(&handles) {
            assert!(table.intern(condition(*value)) == *handle);
            assert!(table.intern_ref(&condition(*value)) == *handle);
        }
        assert!(handles[0] != handles[1]);
    }

    #[test]
    fn arrival_keeps_unrelated_rows_and_substitutes_rebound_values() {
        let mut table = PremiseTable::default();
        let unrelated = table.intern(condition(1));
        let rebound = table.intern(condition(2));
        let retired = table.intern(condition(3));
        let actual = ValueId::new(2).unwrap();
        let formal = ValueId::new(20).unwrap();
        let arrived = table.arrival_requirements(
            &[unrelated.clone(), rebound, retired],
            &[(formal, ScalarTerm::value(actual, ScalarType::Boolean))],
            &[ValueId::new(3).unwrap(), formal],
        );
        assert!(arrived[0] == unrelated);
        assert_eq!(arrived[1].as_ref(), &condition(20));
        assert_eq!(
            arrived[2..]
                .iter()
                .map(|premise| premise.as_ref().clone())
                .collect::<Vec<_>>(),
            vec![
                Proposition::Equal(
                    ScalarTerm::value(formal, ScalarType::Boolean),
                    ScalarTerm::value(formal, ScalarType::Boolean),
                ),
                Proposition::Equal(
                    ScalarTerm::value(formal, ScalarType::Boolean),
                    ScalarTerm::value(actual, ScalarType::Boolean),
                ),
            ]
        );
        assert_eq!(arrived.len(), 4);
    }
}
