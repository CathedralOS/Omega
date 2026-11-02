//! Row-local slots over shared immutable decoded evidence.
//!
//! A decoded footprint contains several operand/effect vectors. Keeping one
//! owned copy per instruction, and then cloning entire encodings for layout,
//! scales that evidence with row count even when its contents are identical.
//! Content sharing changes storage only: every row retains a distinct slot,
//! canonical identity still encodes its complete payload, and every admission
//! still replays its bytes. A mutable proposal detaches its slot so corruption
//! of one row cannot change another row or a prior encoding.

use super::{SelectedFormDecodedFootprint, identity::footprint_bucket};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Weak};

#[derive(Clone, Default)]
pub(super) struct SharedFootprints {
    rows: Vec<Arc<SelectedFormDecodedFootprint>>,
    // Weak entries cannot keep detached/mutated proposals alive. Arc::make_mut
    // either leaves a shared original intact or dissociates its weak entries.
    // The digest only selects candidates; complete payload equality decides.
    interned: HashMap<[u8; 32], Vec<Weak<SelectedFormDecodedFootprint>>>,
}

impl SharedFootprints {
    pub(super) fn with_row_capacity(row_count: usize) -> Self {
        Self {
            rows: Vec::with_capacity(row_count),
            interned: HashMap::new(),
        }
    }

    pub(super) fn intern(&mut self, footprint: SelectedFormDecodedFootprint) -> usize {
        let digest = footprint_bucket(&footprint);
        self.intern_in_bucket(footprint, digest)
    }

    fn intern_in_bucket(
        &mut self,
        footprint: SelectedFormDecodedFootprint,
        digest: [u8; 32],
    ) -> usize {
        let bucket = self.interned.entry(digest).or_default();
        bucket.retain(|candidate| candidate.strong_count() != 0);
        let shared = bucket
            .iter()
            .filter_map(Weak::upgrade)
            .find(|candidate| candidate.as_ref() == &footprint)
            .unwrap_or_else(|| {
                let shared = Arc::new(footprint);
                bucket.push(Arc::downgrade(&shared));
                shared
            });
        let index = self.rows.len();
        self.rows.push(shared);
        index
    }

    pub(super) fn get(&self, index: usize) -> Option<&SelectedFormDecodedFootprint> {
        self.rows.get(index).map(AsRef::as_ref)
    }

    pub(super) fn get_mut(&mut self, index: usize) -> Option<&mut SelectedFormDecodedFootprint> {
        self.rows.get_mut(index).map(Arc::make_mut)
    }
}

// The construction index is not retained evidence. Equality and presentation
// describe the same ordered row values whether or not their allocations share.
impl PartialEq for SharedFootprints {
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows
    }
}

impl Eq for SharedFootprints {}

impl fmt::Debug for SharedFootprints {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.rows.fmt(formatter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use register_model::RegisterViewId;
    use selected_instructions::MachineEncodedEffects;

    fn footprint(view: u16) -> SelectedFormDecodedFootprint {
        SelectedFormDecodedFootprint {
            register_reads: vec![RegisterViewId(view)],
            register_writes: vec![],
            implicit_defs: vec![],
            implicit_clobbers: vec![],
            encoded: MachineEncodedEffects::fallthrough_v1(vec![0], vec![]),
        }
    }

    #[test]
    fn shared_rows_and_cloned_encodings_mutate_independently() {
        let mut store = SharedFootprints::default();
        let first = store.intern(footprint(1));
        let second = store.intern(footprint(1));
        assert_ne!(first, second);
        assert!(Arc::ptr_eq(&store.rows[first], &store.rows[second]));
        let original = store.clone();
        store.get_mut(first).unwrap().register_reads[0] = RegisterViewId(2);
        assert_eq!(store.get(first), Some(&footprint(2)));
        assert_eq!(store.get(second), Some(&footprint(1)));
        assert_eq!(original.get(first), Some(&footprint(1)));
    }

    #[test]
    fn digest_collision_does_not_identify_different_evidence() {
        let mut store = SharedFootprints::default();
        let first = store.intern_in_bucket(footprint(1), [0; 32]);
        let second = store.intern_in_bucket(footprint(2), [0; 32]);
        let repeated = store.intern_in_bucket(footprint(1), [0; 32]);
        assert_eq!(store.get(first), Some(&footprint(1)));
        assert_eq!(store.get(second), Some(&footprint(2)));
        assert!(Arc::ptr_eq(&store.rows[first], &store.rows[repeated]));
        assert!(!Arc::ptr_eq(&store.rows[first], &store.rows[second]));
    }

    #[test]
    fn mutated_unique_row_cannot_poison_its_old_content_bucket() {
        let mut store = SharedFootprints::default();
        let changed = store.intern(footprint(1));
        store.get_mut(changed).unwrap().register_reads[0] = RegisterViewId(2);
        let original = store.intern(footprint(1));
        assert_eq!(store.get(changed), Some(&footprint(2)));
        assert_eq!(store.get(original), Some(&footprint(1)));
    }
}
