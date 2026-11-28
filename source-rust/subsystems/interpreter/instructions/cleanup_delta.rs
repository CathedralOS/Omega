use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::values::TerminalStructuralValue;
use crate::engine::custody::{AffineFrontier, StructuralHomes};
use crate::engine::execution::TerminalExecution;
use crate::storage::runtime_rows::RuntimeRows;
use semantic_vocabulary::{ClaimId, PlaceId};
use terminal_psi::StructuralAffineDiscard;

/// Homes only disappear during cleanup. Keep the two kinds of removal distinct:
/// an owned call can transfer the structural value before a packed home retires.
pub(super) struct CleanupHomes<'a, 'program> {
    execution: &'a TerminalExecution<'program>,
    removed: RuntimeRows<PlaceId, ()>,
    retired_arrays: RuntimeRows<PlaceId, ()>,
}

impl<'a, 'program> CleanupHomes<'a, 'program> {
    pub(super) fn new(
        execution: &'a TerminalExecution<'program>,
    ) -> Result<Self, TerminalInterpretError> {
        let (smaller, larger) = if execution.linear_homes.len() < execution.structural_values.len()
        {
            (&execution.linear_homes, &execution.structural_values)
        } else {
            (&execution.structural_values, &execution.linear_homes)
        };
        if smaller.keys().any(|place| larger.contains_key(place)) {
            return Err(malformed());
        }
        Ok(Self {
            execution,
            removed: RuntimeRows::new(execution.account.as_ref()),
            retired_arrays: RuntimeRows::new(execution.account.as_ref()),
        })
    }

    pub(super) fn remove(&mut self, place: PlaceId) -> Result<(), TerminalInterpretError> {
        self.removed.insert(place, ())?;
        Ok(())
    }

    pub(super) fn retire(&mut self, place: PlaceId) -> Result<(), TerminalInterpretError> {
        let packed = self.execution.scalar_array_values.contains_key(&place)
            && !self.retired_arrays.contains_key(&place);
        let structural = !self.removed.contains_key(&place)
            && (self.execution.structural_values.contains_key(&place)
                || self.execution.linear_homes.contains_key(&place));
        if crate::engine::custody::retiring_structural_home(place, structural, packed)? {
            self.removed.insert(place, ())?;
        } else {
            self.retired_arrays.insert(place, ())?;
        }
        Ok(())
    }
}

impl StructuralHomes for CleanupHomes<'_, '_> {
    fn home(&self, place: PlaceId) -> Option<&TerminalStructuralValue> {
        if self.removed.contains_key(&place) {
            return None;
        }
        self.execution.structural_values.get(&place)
    }
}

pub(super) struct CleanupFrontier<'a, F> {
    pub(super) base: &'a F,
    pub(super) changes: RuntimeRows<StructuralAffineDiscard, bool>,
}

impl<F: AffineFrontier> AffineFrontier for CleanupFrontier<'_, F> {
    fn entries(&self) -> impl Iterator<Item = &StructuralAffineDiscard> {
        let mut kept = self
            .base
            .entries()
            .filter(|entry| !self.changes.contains_key(entry))
            .peekable();
        let mut added = self
            .changes
            .iter()
            .filter_map(|(entry, added)| added.then_some(entry))
            .peekable();
        // Both inputs are ordered and disjoint. Custody comparisons and the
        // choice of a containing projection use the live set's canonical order.
        std::iter::from_fn(move || match (kept.peek(), added.peek()) {
            (Some(left), Some(right)) if left <= right => kept.next(),
            (_, Some(_)) => added.next(),
            _ => kept.next(),
        })
    }

    fn contains_entry(&self, entry: &StructuralAffineDiscard) -> bool {
        self.changes
            .get(entry)
            .copied()
            .unwrap_or_else(|| self.base.contains_entry(entry))
    }

    fn add(&mut self, entry: StructuralAffineDiscard) -> Result<bool, TerminalInterpretError> {
        if self.contains_entry(&entry) {
            return Ok(false);
        }
        if self.changes.remove(&entry).is_none() {
            self.changes.insert(entry, true)?;
        }
        Ok(true)
    }

    fn discard(&mut self, entry: &StructuralAffineDiscard) -> Result<(), TerminalInterpretError> {
        if self.changes.get(entry) == Some(&true) {
            self.changes.remove(entry);
        } else if self.base.contains_entry(entry) {
            self.changes.insert(entry.clone(), false)?;
        }
        Ok(())
    }

    fn discard_matching(
        &mut self,
        matches: impl Fn(&StructuralAffineDiscard) -> bool,
    ) -> Result<(), TerminalInterpretError> {
        self.changes
            .retain(|entry, added| !*added || !matches(entry));
        for entry in self.base.entries().filter(|entry| matches(entry)) {
            if !self.changes.contains_key(entry) {
                self.changes.insert(entry.clone(), false)?;
            }
        }
        Ok(())
    }
}

/// Projected edge operands transfer before the first cleanup call. Their
/// validated edits survive preflight and become visible only after edge charge.
pub(crate) struct ProjectedTransfers {
    pub(super) frontier: RuntimeRows<StructuralAffineDiscard, bool>,
    pub(super) claims: RuntimeRows<ClaimId, ()>,
}

impl ProjectedTransfers {
    pub(super) fn commit(mut self, execution: &mut TerminalExecution) {
        while let Some((entry, added)) = self.frontier.pop_first() {
            if added {
                execution.live_affine_frontier.insert(entry);
            } else {
                execution.live_affine_frontier.remove(&entry);
            }
        }
        for id in self.claims.keys() {
            execution.live_claims.remove(id);
        }
    }
}
