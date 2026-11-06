//! A judge's stored observations, found by exact coordinate.
//!
//! Every judge retains one entry observation per scalar slot of its admitted
//! inputs before reading the body. C layout planning passes a 32-field schema
//! beside a 64-entry plan, about 4,300 slots, and each retention first looked
//! for an existing observation at the same coordinate by scanning all earlier
//! ones. That made each judge's entry walk quadratic: about 0.5 s per judge,
//! and over a minute of checking for one program that demands those layouts.
//!
//! The values keep their order, so every reader that iterates them sees what
//! it saw before. Lookup returns the first value with an equal coordinate,
//! exactly as the scan did. Coordinates never change after a value is
//! stored; only `push`, `retain_mut` and construction change which positions
//! exist, and each keeps the index exact.
//!
//! Branches share unchanged observations. Copying the collection copies its
//! ordered row references and coordinate index, while updating a row first
//! copies that row if another branch still uses it. Reading one place therefore
//! does not copy every stored term. All comparisons still inspect row contents.

use super::{CapturedStorageCoordinate, CapturedStorageSegment, CapturedStorageValue};
use std::collections::HashMap;
use std::sync::Arc;

/// No earlier position shares this coordinate key.
const NO_POSITION: u32 = u32::MAX;

#[derive(Clone, Default)]
pub(in crate::proof_contracts::contract_entailment::structural_judgment) struct StoredValues {
    values: Vec<Arc<CapturedStorageValue>>,
    /// The last position whose coordinate has each key.
    last_with_key: HashMap<u64, u32>,
    /// For each position, the previous position whose coordinate has the same
    /// key, or `NO_POSITION`. Equal keys do not imply equal coordinates, so
    /// lookup compares exactly.
    previous_with_key: Vec<u32>,
}

impl StoredValues {
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn from_values(
        values: Vec<CapturedStorageValue>,
    ) -> Self {
        let mut stored = Self {
            values: values.into_iter().map(Arc::new).collect(),
            ..Self::default()
        };
        stored.rebuild_index();
        stored
    }

    /// The first position holding `coordinate`.
    fn position(&self, coordinate: &CapturedStorageCoordinate) -> Option<usize> {
        let mut candidate = *self.last_with_key.get(&coordinate.search_key())?;
        let mut first = None;
        while candidate != NO_POSITION {
            let position = candidate as usize;
            if self.values[position].coordinate == *coordinate {
                first = Some(position);
            }
            candidate = self.previous_with_key[position];
        }
        first
    }

    /// Every value stored at `coordinate`, in stored order, for readers that
    /// select among them by more than the coordinate.
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn values_at(
        &self,
        coordinate: &CapturedStorageCoordinate,
    ) -> impl Iterator<Item = &CapturedStorageValue> {
        let mut positions = Vec::new();
        let mut candidate = self
            .last_with_key
            .get(&coordinate.search_key())
            .copied()
            .unwrap_or(NO_POSITION);
        while candidate != NO_POSITION {
            let position = candidate as usize;
            if self.values[position].coordinate == *coordinate {
                positions.push(position);
            }
            candidate = self.previous_with_key[position];
        }
        positions
            .into_iter()
            .rev()
            .map(|position| self.values[position].as_ref())
    }

    /// The first value stored at `coordinate`.
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn value_at(
        &self,
        coordinate: &CapturedStorageCoordinate,
    ) -> Option<&CapturedStorageValue> {
        Some(self.values[self.position(coordinate)?].as_ref())
    }

    /// The first value stored at `coordinate`, for updating its observations.
    pub(super) fn value_at_mut(
        &mut self,
        coordinate: &CapturedStorageCoordinate,
    ) -> Option<&mut CapturedStorageValue> {
        let position = self.position(coordinate)?;
        Some(Arc::make_mut(&mut self.values[position]))
    }

    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn len(
        &self,
    ) -> usize {
        self.values.len()
    }

    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn is_empty(
        &self,
    ) -> bool {
        self.values.is_empty()
    }

    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = &CapturedStorageValue> + ExactSizeIterator {
        self.values.iter().map(Arc::as_ref)
    }

    #[cfg(test)]
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn first(
        &self,
    ) -> Option<&CapturedStorageValue> {
        self.values.first().map(Arc::as_ref)
    }

    /// Callers may update each value's observations in place; none rewrites
    /// its coordinate, which the index is keyed by, or reorders the values.
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn iter_mut(
        &mut self,
    ) -> impl DoubleEndedIterator<Item = &mut CapturedStorageValue> + ExactSizeIterator {
        self.values.iter_mut().map(Arc::make_mut)
    }

    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn push(
        &mut self,
        value: CapturedStorageValue,
    ) {
        let position =
            u32::try_from(self.values.len()).expect("stored observations fit a u32 position");
        let previous = self
            .last_with_key
            .insert(value.coordinate.search_key(), position)
            .unwrap_or(NO_POSITION);
        self.previous_with_key.push(previous);
        self.values.push(Arc::new(value));
    }

    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn retain_mut(
        &mut self,
        mut keep: impl FnMut(&mut CapturedStorageValue) -> bool,
    ) {
        let before = self.values.len();
        self.values.retain_mut(|value| keep(Arc::make_mut(value)));
        if self.values.len() != before {
            self.rebuild_index();
        }
    }

    fn rebuild_index(&mut self) {
        self.last_with_key.clear();
        self.previous_with_key.clear();
        for (position, value) in self.values.iter().enumerate() {
            let position = u32::try_from(position).expect("stored observations fit a u32 position");
            let previous = self
                .last_with_key
                .insert(value.coordinate.search_key(), position)
                .unwrap_or(NO_POSITION);
            self.previous_with_key.push(previous);
        }
    }
}

impl PartialEq for StoredValues {
    fn eq(&self, other: &Self) -> bool {
        self.values.len() == other.values.len()
            && self
                .iter()
                .zip(other.iter())
                .all(|(left, right)| left == right)
    }
}

impl std::ops::Index<usize> for StoredValues {
    type Output = CapturedStorageValue;

    fn index(&self, index: usize) -> &Self::Output {
        self.values[index].as_ref()
    }
}

impl<'values> IntoIterator for &'values mut StoredValues {
    type Item = &'values mut CapturedStorageValue;
    type IntoIter = std::iter::Map<
        std::slice::IterMut<'values, Arc<CapturedStorageValue>>,
        fn(&'values mut Arc<CapturedStorageValue>) -> &'values mut CapturedStorageValue,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.values.iter_mut().map(Arc::make_mut)
    }
}

impl CapturedStorageCoordinate {
    /// Equal coordinates have equal keys. Roots and place segments contribute
    /// their exact integer identities; an index selector contributes the key
    /// structurally equal terms share.
    fn search_key(&self) -> u64 {
        let (kind, index, generation) = self.root.group_key();
        let mut key = mix(
            u64::from(kind),
            (u64::from(index) << 32) | u64::from(generation),
        );
        for segment in &self.segments {
            match segment {
                CapturedStorageSegment::Place(place) => {
                    let (kind, first, second) = place.order_key();
                    key = mix(mix(mix(key, u64::from(kind)), first), second);
                }
                CapturedStorageSegment::Index(term) => {
                    key = mix(
                        mix(key, u64::MAX),
                        super::super::rewrite_table::structural_term_key(term),
                    );
                }
            }
        }
        key
    }
}

fn mix(key: u64, value: u64) -> u64 {
    (key.rotate_left(5) ^ value).wrapping_mul(0x517c_c1b7_2722_0a95)
}

#[cfg(test)]
mod tests;
