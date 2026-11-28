/*
Layout-independent instruction encoding (`SelectedFormEncoding`), row spans (`SelectedFormEncodingRow`),
and the deduplicated byte-and-footprint store (`SelectedFormEncodedRowStore`).

One `SelectedFormEncoding` owns a single contiguous byte buffer and a deduplicated decoded-footprint
table (`SharedFootprints`). Each `SelectedFormEncodingRow` references its instruction bytes via an
offset span (`SelectedFormEncodedByteSpan`) and its decoded register/effect footprint via a handle
(`SelectedFormDecodedFootprintHandle`), avoiding per-row heap allocations and allowing cloned
encodings to share immutable footprints while detaching only mutated slots via `Arc::make_mut`.
*/

/*
Why sparse fixup and address payloads are boxed out of line:
Every ordinary instruction row carries a `SelectedFormEncodingState`, a `SelectedFormMachineDisposition`,
and an optional `ResolvedPhysicalAddress`. Boxing the rare fixup, rewrite, and address payloads
keeps the common `SelectedFormEncodingRow` stride compact without changing value equality or
cloning. Later stages share the encoding through an `Arc` and compare it; it carries no content hash.
*/

#[path = "encoding_footprint_bucket.rs"]
mod footprint_bucket;

use crate::SelectedFormInternalMachineFixup;
use post_allocation_machine::PostAllocationMachinePlan;
use register_model::{RegisterUnitId, RegisterViewId};
use selected_instructions::{MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionId};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Weak};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFormEncoding {
    pub machine: Arc<PostAllocationMachinePlan>,
    pub rows: Vec<SelectedFormEncodingRow>,
    pub encoded: Arc<SelectedFormEncodedRowStore>,
    pub frame: Option<Arc<crate::TargetFrameLayoutPlan>>,
    pub counts: SelectedFormEncodingCounts,
}

impl SelectedFormEncoding {
    pub fn machine(&self) -> &Arc<PostAllocationMachinePlan> {
        &self.machine
    }

    pub fn rows(&self) -> &[SelectedFormEncodingRow] {
        &self.rows
    }

    pub fn encoded(&self) -> &SelectedFormEncodedRowStore {
        &self.encoded
    }

    pub const fn counts(&self) -> SelectedFormEncodingCounts {
        self.counts
    }
}

/// Names the encoding a later record was built from without keeping it
/// alive: ordinary placement releases the encoded program before it builds an
/// object, while layouts, exit contracts and relaxation records still say
/// which encoding they came from. Two handles agree when they name the same
/// encoding, or when both encodings are still alive and equal. A handle to a
/// released encoding agrees only with itself, so a check against another
/// encoding fails rather than passes.
#[derive(Clone)]
pub struct SelectedFormEncodingHandle(Weak<SelectedFormEncoding>);

impl SelectedFormEncodingHandle {
    pub fn of(encoding: &Arc<SelectedFormEncoding>) -> Self {
        Self(Arc::downgrade(encoding))
    }

    /// Whether this handle names `encoding`: the same encoding, or an equal
    /// one while the named encoding is still alive.
    pub fn names(&self, encoding: &SelectedFormEncoding) -> bool {
        std::ptr::eq(self.0.as_ptr(), encoding)
            || self.0.upgrade().is_some_and(|named| *named == *encoding)
    }
}

impl PartialEq for SelectedFormEncodingHandle {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.0, &other.0)
            || match (self.0.upgrade(), other.0.upgrade()) {
                (Some(left), Some(right)) => left == right,
                _ => false,
            }
    }
}

impl Eq for SelectedFormEncodingHandle {}

impl fmt::Debug for SelectedFormEncodingHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SelectedFormEncodingHandle")
            .field(&self.0.as_ptr())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredControlEncodingReason {
    RequiresResolvedBranchLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFormDecodedFootprint {
    pub register_reads: Vec<RegisterViewId>,
    pub register_writes: Vec<RegisterViewId>,
    pub implicit_defs: Vec<RegisterUnitId>,
    pub implicit_clobbers: Vec<RegisterUnitId>,
    pub encoded: MachineEncodedEffects,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelectedFormEncodedByteSpan {
    pub offset: u64,
    pub byte_count: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SelectedFormDecodedFootprintHandle(u64);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectedFormEncodedRowStore {
    bytes: Vec<u8>,
    footprints: SharedFootprints,
}

impl SelectedFormEncodedRowStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_row_capacity(row_count: usize) -> Self {
        Self {
            bytes: Vec::new(),
            footprints: SharedFootprints::with_row_capacity(row_count),
        }
    }

    pub fn intern(
        &mut self,
        bytes: &[u8],
        footprint: SelectedFormDecodedFootprint,
    ) -> (
        SelectedFormEncodedByteSpan,
        SelectedFormDecodedFootprintHandle,
    ) {
        let span = SelectedFormEncodedByteSpan {
            offset: self.bytes.len() as u64,
            byte_count: bytes.len() as u64,
        };
        self.bytes.extend_from_slice(bytes);
        let index = self.footprints.intern(footprint);
        let handle = SelectedFormDecodedFootprintHandle(index as u64 + 1);
        (span, handle)
    }

    pub fn bytes(&self, span: SelectedFormEncodedByteSpan) -> &[u8] {
        let Ok(offset) = usize::try_from(span.offset) else {
            return &[];
        };
        let Ok(byte_count) = usize::try_from(span.byte_count) else {
            return &[];
        };
        offset
            .checked_add(byte_count)
            .and_then(|end| self.bytes.get(offset..end))
            .unwrap_or(&[])
    }

    pub fn bytes_mut(&mut self, span: SelectedFormEncodedByteSpan) -> &mut [u8] {
        let Ok(offset) = usize::try_from(span.offset) else {
            return &mut [];
        };
        let Ok(byte_count) = usize::try_from(span.byte_count) else {
            return &mut [];
        };
        match offset.checked_add(byte_count) {
            Some(end) => self.bytes.get_mut(offset..end).unwrap_or(&mut []),
            None => &mut [],
        }
    }

    pub fn footprint(
        &self,
        handle: SelectedFormDecodedFootprintHandle,
    ) -> Option<&SelectedFormDecodedFootprint> {
        self.footprints.get(Self::index(handle)?)
    }

    pub(crate) fn share_footprint(
        &self,
        handle: SelectedFormDecodedFootprintHandle,
    ) -> Option<Arc<SelectedFormDecodedFootprint>> {
        self.footprints.rows.get(Self::index(handle)?).cloned()
    }

    pub fn footprint_mut(
        &mut self,
        handle: SelectedFormDecodedFootprintHandle,
    ) -> Option<&mut SelectedFormDecodedFootprint> {
        let index = Self::index(handle)?;
        self.footprints.get_mut(index)
    }

    pub fn encoded(
        &self,
        row: &SelectedFormEncodingRow,
    ) -> Option<(&[u8], &SelectedFormDecodedFootprint)> {
        Some((self.bytes(row.code), self.footprint(row.footprint)?))
    }

    fn index(handle: SelectedFormDecodedFootprintHandle) -> Option<usize> {
        usize::try_from(handle.0.checked_sub(1)?).ok()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedFormEncodingState {
    Encoded,
    DeferredControl {
        reason: DeferredControlEncodingReason,
        alternatives: std::sync::Arc<crate::EncodedControlAlternatives>,
    },
    UnresolvedInternalMachineCall {
        fixup: Box<SelectedFormInternalMachineFixup>,
    },
    UnresolvedNormalizedForeignCall {
        fixup: Box<crate::SelectedFormNormalizedForeignCallFixup>,
    },
    UnresolvedImageAddress {
        fixup: Box<crate::SelectedFormImageAddressFixup>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedFormMachineDisposition {
    RetainedV1,
    Aarch64ElidedCompareI64ZeroV1 {
        consumer: SelectedInstructionId,
    },
    Aarch64FusedBranchNonZeroToCbnzV1 {
        compare: SelectedInstructionId,
        source_read: Box<post_allocation_machine::QualifiedPhysicalRead>,
    },
    Aarch64ElidedSameViewCopyI64V1 {
        consumer: SelectedInstructionId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFormEncodingRow {
    pub instruction: SelectedInstructionId,
    pub alternative: MachineAlternativeKey,
    pub machine_disposition: SelectedFormMachineDisposition,
    pub state: SelectedFormEncodingState,
    pub address: Option<Box<ResolvedPhysicalAddress>>,
    pub code: SelectedFormEncodedByteSpan,
    pub footprint: SelectedFormDecodedFootprintHandle,
}

impl SelectedFormEncodingRow {
    pub fn address(&self) -> Option<ResolvedPhysicalAddress> {
        self.address.as_deref().copied()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelectedFormEncodingCounts {
    pub ordinary_encoded: u64,
    pub ordinary_deferred_control: u64,
    pub ordinary_encoded_call_templates: u64,
    pub ordinary_deferred_internal_control: u64,
    pub ordinary_internal_fixups: u64,
    pub ordinary_normalized_foreign_import_fixups: u64,
    pub ordinary_image_address_fixups: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedPhysicalAddress {
    pub symbolic: post_allocation_machine::PhysicalAddressOperation,
    pub displacement: i64,
    pub frame_alignment_bytes: u16,
}

#[derive(Clone, Default)]
pub(super) struct SharedFootprints {
    rows: Vec<Arc<SelectedFormDecodedFootprint>>,
    interned: HashMap<[u8; 32], Vec<Weak<SelectedFormDecodedFootprint>>>, // Weak entries allow detached/mutated proposals to drop without poisoning shared buckets.
}

impl SharedFootprints {
    pub(super) fn with_row_capacity(row_count: usize) -> Self {
        Self {
            rows: Vec::with_capacity(row_count),
            interned: HashMap::new(),
        }
    }

    pub(super) fn intern(&mut self, footprint: SelectedFormDecodedFootprint) -> usize {
        let digest = footprint_bucket::footprint_bucket(&footprint);
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
mod footprint_tests {
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
