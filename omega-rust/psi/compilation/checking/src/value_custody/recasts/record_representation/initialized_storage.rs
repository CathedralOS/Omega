//! Physical coverage of established scalar and fragmented storage.
//!
//! This inventory permits observation only into unrestricted non-atomic bits.
//! It carries no value predicate or access authority. Each implication checks
//! target value validity independently; mutable views require both directions.

use super::{MutableRecordLeaf, MutableRecordRepresentation};
use std::ops::Range;

pub(super) struct InitializedStorageBits {
    ranges: Vec<Range<usize>>,
    limit: usize,
}

impl InitializedStorageBits {
    pub(super) fn prepare(source: &MutableRecordRepresentation) -> Option<Self> {
        let limit = source.size.checked_mul(8)?;
        let count = source.leaves.iter().try_fold(0usize, |count, leaf| {
            count.checked_add(if leaf.byte_projection {
                1
            } else {
                leaf.bit_fragments.len()
            })
        })?;
        let mut ranges = Vec::<Range<usize>>::new();
        ranges.try_reserve_exact(count).ok()?;
        for leaf in &source.leaves {
            if leaf.facts.atomic {
                continue;
            }
            visit_leaf_ranges(leaf, limit, |range| {
                ranges.push(range);
                Some(())
            })?;
        }

        ranges.sort_unstable_by_key(|range| (range.start, range.end));
        let mut retained = 0usize;
        for position in 0..ranges.len() {
            if retained > 0 && ranges[position].start <= ranges[retained - 1].end {
                ranges[retained - 1].end = ranges[retained - 1].end.max(ranges[position].end);
            } else {
                ranges.swap(retained, position);
                retained += 1;
            }
        }
        ranges.truncate(retained);
        Some(Self { ranges, limit })
    }

    pub(super) fn contains_leaf(&self, leaf: &MutableRecordLeaf) -> bool {
        visit_leaf_ranges(leaf, self.limit, |range| {
            let next = self
                .ranges
                .partition_point(|source| source.start <= range.start);
            next.checked_sub(1)
                .filter(|&previous| range.end <= self.ranges[previous].end)
                .map(|_| ())
        })
        .is_some()
    }
}

/// Enumerate only the bits the existing normalized scalar recipe owns. A
/// fragmented leaf does not observe unused container bits or intervening holes.
fn visit_leaf_ranges(
    leaf: &MutableRecordLeaf,
    limit: usize,
    mut visit: impl FnMut(Range<usize>) -> Option<()>,
) -> Option<()> {
    if leaf.byte_projection {
        let start = leaf.offset.checked_mul(8)?;
        let end = leaf.offset.checked_add(leaf.size)?.checked_mul(8)?;
        if end > limit {
            return None;
        }
        if start < end {
            visit(start..end)?;
        }
    } else {
        if leaf.bit_fragments.is_empty() {
            return None;
        }
        for fragment in &leaf.bit_fragments {
            let offset = leaf
                .offset
                .checked_add(fragment.container_byte_offset)?
                .checked_mul(8)?;
            let width = usize::from(fragment.container_width_bits);
            let first = usize::from(fragment.destination_lsb);
            let last = first.checked_add(usize::from(fragment.width))?;
            if fragment.width == 0 || last > width || offset.checked_add(width)? > limit {
                return None;
            }
            visit(offset.checked_add(first)?..offset.checked_add(last)?)?;
        }
    }
    Some(())
}
