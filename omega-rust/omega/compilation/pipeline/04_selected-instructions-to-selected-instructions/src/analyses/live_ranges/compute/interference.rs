//! Exact virtual interference without retaining repeated overlap occurrences.
use super::{LiveRangeFragment, SelectedBlockId, VirtualInterference, VirtualLiveRange};
use std::collections::BTreeMap;

/// A sorted block's implicit balanced interval tree. Each midpoint owns the
/// maximum end of its slice; the index needs no separate nodes or edge rows.
struct Interval {
    start: u32,
    end: u32,
    row: u32,
    max_end: u32,
}

const _: () = assert!(std::mem::size_of::<Interval>() == 16);

fn prepare(intervals: &mut [Interval]) -> u32 {
    if intervals.is_empty() {
        return 0;
    }
    let middle = intervals.len() / 2;
    let (left, rest) = intervals.split_at_mut(middle);
    let (node, right) = rest.split_first_mut().expect("nonempty interval slice");
    node.max_end = node.end.max(prepare(left)).max(prepare(right));
    node.max_end
}

fn append_overlaps(
    intervals: &[Interval],
    fragment: &LiveRangeFragment,
    lower_row: usize,
    rows: &[VirtualLiveRange],
    seen: &mut [usize],
    pairs: &mut Vec<VirtualInterference>,
) {
    if intervals.is_empty() {
        return;
    }
    let middle = intervals.len() / 2;
    let node = &intervals[middle];
    if node.max_end <= fragment.start.0 {
        return;
    }
    append_overlaps(&intervals[..middle], fragment, lower_row, rows, seen, pairs);
    // Starts are ordered, so this node and its right subtree cannot overlap.
    if node.start >= fragment.end.0 {
        return;
    }
    let higher_row = node.row as usize;
    if lower_row < higher_row && fragment.start.0 < node.end && seen[higher_row] != lower_row {
        seen[higher_row] = lower_row;
        pairs.push(VirtualInterference {
            lower: rows[lower_row].virtual_register,
            higher: rows[higher_row].virtual_register,
        });
    }
    append_overlaps(
        &intervals[middle + 1..],
        fragment,
        lower_row,
        rows,
        seen,
        pairs,
    );
}

/// Every distinct row pair with overlapping nonempty half-open fragments in
/// one block. Repeated fragments and overlaps in other blocks add no storage.
pub(super) fn interference(rows: &[VirtualLiveRange]) -> Vec<VirtualInterference> {
    let mut blocks = BTreeMap::<SelectedBlockId, Vec<Interval>>::new();
    for (row, range) in rows.iter().enumerate() {
        for fragment in &range.fragments {
            if fragment.start < fragment.end {
                blocks.entry(fragment.block).or_default().push(Interval {
                    start: fragment.start.0,
                    end: fragment.end.0,
                    row: row as u32,
                    max_end: 0,
                });
            }
        }
    }
    for intervals in blocks.values_mut() {
        intervals.sort_unstable_by_key(|node| (node.start, node.row));
        prepare(intervals);
    }

    let mut pairs = Vec::new();
    let mut seen = vec![usize::MAX; rows.len()];
    for (row, range) in rows.iter().enumerate() {
        let first_pair = pairs.len();
        for fragment in &range.fragments {
            if fragment.start < fragment.end {
                append_overlaps(
                    &blocks[&fragment.block],
                    fragment,
                    row,
                    rows,
                    &mut seen,
                    &mut pairs,
                );
            }
        }
        pairs[first_pair..].sort_unstable();
    }
    // Valid canonical rows are already ordered. Preserve the former general
    // path's full-ID ordering and deduplication for unordered/duplicate IDs too.
    if !rows
        .windows(2)
        .all(|pair| pair[0].virtual_register < pair[1].virtual_register)
    {
        pairs.sort_unstable();
        pairs.dedup();
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;
    use selected_instructions::{LiveRangePoint, VirtualRegisterId};

    fn range(id: u32, fragments: Vec<LiveRangeFragment>) -> VirtualLiveRange {
        VirtualLiveRange {
            virtual_register: VirtualRegisterId(id),
            class: register_model::RegisterClassId(0),
            occurrences: Vec::new(),
            fixed_constraints: Vec::new(),
            fragments,
            edge_connectors: Vec::new(),
        }
    }

    fn expected(rows: &[VirtualLiveRange]) -> Vec<VirtualInterference> {
        let mut pairs = Vec::new();
        for (lower, left) in rows.iter().enumerate() {
            for right in rows.iter().skip(lower + 1) {
                if left.fragments.iter().any(|left| {
                    left.start < left.end
                        && right.fragments.iter().any(|right| {
                            right.start < right.end
                                && left.block == right.block
                                && left.start < right.end
                                && right.start < left.end
                        })
                }) {
                    pairs.push(VirtualInterference {
                        lower: left.virtual_register,
                        higher: right.virtual_register,
                    });
                }
            }
        }
        pairs.sort_unstable();
        pairs.dedup();
        pairs
    }

    /// Exercise the production graph builder against independent row-pair
    /// intersection, including holes, duplicate fragments and sparse identities.
    #[test]
    fn sweep_matches_every_row_pair() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = |bound: u32| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % u64::from(bound)) as u32
        };
        for _ in 0..200 {
            let rows = (0..next(24) + 1)
                .map(|row| {
                    let fragments = (0..next(5))
                        .map(|_| {
                            let start = next(40);
                            LiveRangeFragment {
                                block: SelectedBlockId(next(3) * 100_003),
                                start: LiveRangePoint(start),
                                end: LiveRangePoint(start + next(12)),
                            }
                        })
                        .collect::<Vec<_>>();
                    range(row * 100_003, fragments)
                })
                .collect::<Vec<_>>();
            assert_eq!(interference(&rows), expected(&rows));
            let mut dense_blocks = rows.clone();
            for row in &mut dense_blocks {
                for fragment in &mut row.fragments {
                    fragment.block.0 %= 3;
                }
            }
            assert_eq!(interference(&dense_blocks), expected(&dense_blocks));
            let reversed = rows.into_iter().rev().collect::<Vec<_>>();
            assert_eq!(interference(&reversed), expected(&reversed));
        }
        let fragments = vec![
            LiveRangeFragment {
                block: SelectedBlockId(u32::MAX),
                start: LiveRangePoint(0),
                end: LiveRangePoint(10),
            };
            128
        ];
        let rows = vec![
            range(u32::MAX, fragments.clone()),
            range(0, fragments.clone()),
            range(u32::MAX, fragments),
            range(9, Vec::new()),
        ];
        assert_eq!(interference(&rows), expected(&rows));
        assert!(interference(&[]).is_empty());
    }
}
