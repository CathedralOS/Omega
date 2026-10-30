//! Physical coverage of established scalar and fragmented storage.
//!
//! This inventory permits observation only into unrestricted non-atomic bits.
//! It carries no value predicate, writable restoration, or access authority.

use super::MutableRecordRepresentation;
use std::ops::Range;

pub(super) struct InitializedStorageBits {
    ranges: Vec<Range<usize>>,
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
            if leaf.byte_projection {
                let start = leaf.offset.checked_mul(8)?;
                let end = leaf.offset.checked_add(leaf.size)?.checked_mul(8)?;
                if end > limit {
                    return None;
                }
                if start < end {
                    ranges.push(start..end);
                }
            } else {
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
                    ranges.push(offset.checked_add(first)?..offset.checked_add(last)?);
                }
            }
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
        Some(Self { ranges })
    }

    pub(super) fn contains_bytes(&self, offset: usize, size: usize) -> bool {
        let Some((start, end)) = offset
            .checked_mul(8)
            .zip(offset.checked_add(size).and_then(|end| end.checked_mul(8)))
        else {
            return false;
        };
        let next = self.ranges.partition_point(|range| range.start <= start);
        next.checked_sub(1)
            .is_some_and(|previous| end <= self.ranges[previous].end)
    }
}
