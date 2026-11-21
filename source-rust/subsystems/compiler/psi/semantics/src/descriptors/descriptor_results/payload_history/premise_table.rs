use semantic_vocabulary::{Proposition, ScalarTerm, ValueId};
use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use crate::substitution::{
    substitute_proposition_values, substitute_scalar_term_values,
};

const RANK_SPACING: u64 = 1 << 20;

#[derive(Clone)]
pub(crate) struct SharedPremise(Arc<PremiseRow>);

struct PremiseRow {
    proposition: Proposition,
    values: Vec<ValueId>,
    closed: bool,
    substitution_stable: bool,
    rank: AtomicU64,
}

impl SharedPremise {
    fn rank(&self) -> u64 {
        self.0.rank.load(AtomicOrdering::Relaxed)
    }

    pub(crate) fn mentions_any(&self, definitions: &[ValueId]) -> bool {
        definitions
            .iter()
            .any(|value| self.0.values.binary_search(value).is_ok())
    }

    pub(crate) fn survives_payload_retirement(&self, definitions: &[ValueId]) -> bool {
        self.0.closed && !self.mentions_any(definitions)
    }

    #[cfg(test)]
    pub(crate) fn new(proposition: Proposition) -> Self {
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

#[derive(Default)]
pub(crate) struct PremiseTable {
    rows: BTreeSet<SharedPremise>,
}

impl PremiseTable {
    pub(crate) fn intern(&mut self, proposition: Proposition) -> SharedPremise {
        if let Some(existing) = self.rows.get(&proposition) {
            return existing.clone();
        }
        self.insert(proposition)
    }

    pub(crate) fn intern_ref(&mut self, proposition: &Proposition) -> SharedPremise {
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
            substitute_proposition_values(&proposition, &BTreeMap::new())
                == proposition;
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

    pub(crate) fn arrival_requirements(
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
        let surviving = |value: ValueId| {
            !parameters.contains(&value) || substitutions.contains_key(&value)
        };
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
mod tests;
