//! Block domains, virtual fragments, connectors, and point arithmetic.
#[cfg(test)]
use super::BTreeSet;
use super::{
    BlockLiveness, BlockPointDomain, LiveRangeEdgeConnector, LiveRangeFragment, LiveRangePoint,
    LivenessPosition, RegisterOperandAccess, SelectedBlockId, VirtualRegisterId,
};
use crate::LiveRangeError;

pub(super) fn block_domain(
    function: usize,
    block: &BlockLiveness,
) -> Result<BlockPointDomain, LiveRangeError> {
    let first = block
        .instructions
        .first()
        .ok_or(LiveRangeError::BlockDomainMismatch {
            function,
            block: block.block.0,
        })?;
    let last = block
        .instructions
        .last()
        .expect("nonempty block established above");
    Ok(BlockPointDomain {
        block: block.block,
        source_block: block.source_block,
        start: before_point(function, first.position)?,
        end: after_point(function, last.position)?
            .0
            .checked_add(1)
            .map(LiveRangePoint)
            .ok_or(LiveRangeError::PointOverflow { function })?,
    })
}

/// Derive block-local virtual register fragments in $O(\text{instructions} + \text{operands})$
/// time by tracking each register's open interval end during a reverse block walk,
/// visiting only block exits/entries and instructions that explicitly define or use
/// the register instead of scanning the full live set at every instruction.
pub(super) fn register_fragments(
    function: usize,
    liveness: &selected_instructions::FunctionLiveness,
    register_count: usize,
    register_index: &[u32],
) -> Result<Vec<Vec<LiveRangeFragment>>, LiveRangeError> {
    let mut fragments = vec![Vec::<LiveRangeFragment>::new(); register_count];
    let mut open_end = vec![u32::MAX; register_count];
    let mut touched_in_block = Vec::<(u32, u32)>::new();
    for block in &liveness.blocks {
        let Some(first) = block.instructions.first() else {
            continue;
        };
        let last = block.instructions.last().unwrap();
        let block_start = before_point(function, first.position)?.0;
        let block_end = after_point(function, last.position)?
            .0
            .checked_add(1)
            .ok_or(LiveRangeError::PointOverflow { function })?;
        for &register in &block.virtual_live_out {
            if let Some(&idx) = register_index.get(register.0 as usize)
                && idx != u32::MAX
            {
                open_end[idx as usize] = block_end;
            }
        }
        for instruction in block.instructions.iter().rev() {
            let _before = before_point(function, instruction.position)?.0;
            let after = after_point(function, instruction.position)?.0;
            let after_end = after
                .checked_add(1)
                .ok_or(LiveRangeError::PointOverflow { function })?;
            if instruction.virtual_defs.is_empty() && instruction.virtual_uses.is_empty() {
                let ops = liveness
                    .operand_positions
                    .span_or_empty(instruction.operands);
                for (op_idx, op) in ops.iter().enumerate() {
                    if !matches!(
                        op.access,
                        RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                    ) {
                        continue;
                    }
                    let register = op.virtual_register;
                    if ops[..op_idx].iter().any(|prev| {
                        prev.virtual_register == register
                            && matches!(
                                prev.access,
                                RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                            )
                    }) {
                        continue;
                    }
                    if let Some(&idx) = register_index.get(register.0 as usize)
                        && idx != u32::MAX
                    {
                        if open_end[idx as usize] == u32::MAX {
                            open_end[idx as usize] = after_end;
                        }
                        let also_used = op.access == RegisterOperandAccess::UseDef
                            || ops.iter().any(|other| {
                                other.virtual_register == register
                                    && matches!(
                                        other.access,
                                        RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
                                    )
                            });
                        if !also_used {
                            let slot = &mut fragments[idx as usize];
                            if slot.last().map(|f| f.block) != Some(block.block) {
                                touched_in_block.push((idx, slot.len() as u32));
                            }
                            slot.push(LiveRangeFragment {
                                block: block.block,
                                start: LiveRangePoint(after),
                                end: LiveRangePoint(open_end[idx as usize]),
                            });
                            open_end[idx as usize] = u32::MAX;
                        }
                    }
                }
                for op in ops {
                    if matches!(
                        op.access,
                        RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
                    ) && let Some(&idx) = register_index.get(op.virtual_register.0 as usize)
                        && idx != u32::MAX
                        && open_end[idx as usize] == u32::MAX
                    {
                        open_end[idx as usize] = after;
                    }
                }
            } else {
                for &register in &instruction.virtual_defs {
                    if let Some(&idx) = register_index.get(register.0 as usize)
                        && idx != u32::MAX
                    {
                        if open_end[idx as usize] == u32::MAX {
                            open_end[idx as usize] = after_end;
                        }
                        if !instruction.virtual_uses.contains(&register) {
                            let slot = &mut fragments[idx as usize];
                            if slot.last().map(|f| f.block) != Some(block.block) {
                                touched_in_block.push((idx, slot.len() as u32));
                            }
                            slot.push(LiveRangeFragment {
                                block: block.block,
                                start: LiveRangePoint(after),
                                end: LiveRangePoint(open_end[idx as usize]),
                            });
                            open_end[idx as usize] = u32::MAX;
                        }
                    }
                }
                for &register in &instruction.virtual_uses {
                    if let Some(&idx) = register_index.get(register.0 as usize)
                        && idx != u32::MAX
                        && open_end[idx as usize] == u32::MAX
                    {
                        open_end[idx as usize] = after;
                    }
                }
            }
        }
        for &register in &block.virtual_live_in {
            if let Some(&idx) = register_index.get(register.0 as usize)
                && idx != u32::MAX
                && open_end[idx as usize] != u32::MAX
            {
                let slot = &mut fragments[idx as usize];
                if slot.last().map(|f| f.block) != Some(block.block) {
                    touched_in_block.push((idx, slot.len() as u32));
                }
                slot.push(LiveRangeFragment {
                    block: block.block,
                    start: LiveRangePoint(block_start),
                    end: LiveRangePoint(open_end[idx as usize]),
                });
                open_end[idx as usize] = u32::MAX;
            }
        }
        for (idx, first_frag) in touched_in_block.drain(..) {
            fragments[idx as usize][first_frag as usize..].reverse();
        }
    }
    Ok(fragments)
}

/// Append one point from an ordered block traversal, tolerating membership
/// duplicates (a value can be both live and explicitly accessed).
#[cfg(test)]
pub(super) fn append_point(
    function: usize,
    fragments: &mut Vec<LiveRangeFragment>,
    block: SelectedBlockId,
    point: LiveRangePoint,
) -> Result<(), LiveRangeError> {
    let end = point
        .0
        .checked_add(1)
        .map(LiveRangePoint)
        .ok_or(LiveRangeError::PointOverflow { function })?;
    if let Some(last) = fragments.last_mut()
        && last.block == block
        && point <= last.end
    {
        last.end = last.end.max(end);
    } else {
        fragments.push(LiveRangeFragment {
            block,
            start: point,
            end,
        });
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn fragments_from_points(
    block: SelectedBlockId,
    points: BTreeSet<LiveRangePoint>,
) -> Vec<LiveRangeFragment> {
    let mut fragments = Vec::new();
    let mut iterator = points.into_iter();
    let Some(first) = iterator.next() else {
        return fragments;
    };
    let mut start = first;
    let mut last = first;
    for point in iterator {
        if last.0.checked_add(1) == Some(point.0) {
            last = point;
        } else {
            fragments.push(LiveRangeFragment {
                block,
                start,
                end: LiveRangePoint(last.0 + 1),
            });
            start = point;
            last = point;
        }
    }
    fragments.push(LiveRangeFragment {
        block,
        start,
        end: LiveRangePoint(last.0 + 1),
    });
    fragments
}

/// Every pair of virtual registers with overlapping fragments in one block:
/// `lower` is the register earlier in `rows`.
pub(super) fn interference(rows: &[super::VirtualLiveRange]) -> Vec<super::VirtualInterference> {
    if rows.is_empty() {
        return Vec::new();
    }
    let rows_monotonic = rows
        .windows(2)
        .all(|pair| pair[0].virtual_register < pair[1].virtual_register);
    let mut max_block_id = 0usize;
    for range in rows {
        for fragment in &range.fragments {
            if fragment.start < fragment.end {
                max_block_id = max_block_id.max(fragment.block.0 as usize);
            }
        }
    }
    if rows_monotonic && max_block_id < 65536 {
        let mut dense_by_block =
            vec![Vec::<(u32, u32, u32, VirtualRegisterId)>::new(); max_block_id + 1];
        for (row, range) in rows.iter().enumerate() {
            let vreg = range.virtual_register;
            for fragment in &range.fragments {
                if fragment.start < fragment.end {
                    dense_by_block[fragment.block.0 as usize].push((
                        fragment.start.0,
                        fragment.end.0,
                        row as u32,
                        vreg,
                    ));
                }
            }
        }
        let mut raw_pairs: Vec<(u32, VirtualRegisterId)> = Vec::with_capacity(rows.len() * 2);
        let mut bucket_counts = vec![0u32; rows.len()];
        let mut active: Vec<(u32, u32, VirtualRegisterId)> = Vec::new();
        for fragments in &mut dense_by_block {
            if fragments.len() < 2 {
                continue;
            }
            fragments.sort_unstable_by_key(|&(start, _, row, _)| (start, row));
            active.clear();
            for &(start, end, row, vreg) in fragments.iter() {
                active.retain(|&(live_end, _, _)| start < live_end);
                for &(_, live_row, live_vreg) in &active {
                    if live_row != row {
                        let (lo_row, hi_vreg) = if live_row < row {
                            (live_row, vreg)
                        } else {
                            (row, live_vreg)
                        };
                        bucket_counts[lo_row as usize] += 1;
                        raw_pairs.push((lo_row, hi_vreg));
                    }
                }
                active.push((end, row, vreg));
            }
        }
        let total_pairs = raw_pairs.len();
        let mut bucket_offsets = vec![0u32; rows.len() + 1];
        for i in 0..rows.len() {
            bucket_offsets[i + 1] = bucket_offsets[i] + bucket_counts[i];
        }
        let mut cursors = bucket_offsets.clone();
        let mut flat_buckets = vec![VirtualRegisterId(0); total_pairs];
        for (lo_row, hi_vreg) in raw_pairs {
            let cursor = &mut cursors[lo_row as usize];
            flat_buckets[*cursor as usize] = hi_vreg;
            *cursor += 1;
        }
        let max_vreg = rows.last().map_or(0, |r| r.virtual_register.0 as usize);
        let mut seen = vec![0u32; max_vreg + 1];
        let mut stamp = 0u32;
        let mut pairs = Vec::with_capacity(total_pairs);
        for lo_row in 0..rows.len() {
            let start = bucket_offsets[lo_row] as usize;
            let end = bucket_offsets[lo_row + 1] as usize;
            if start == end {
                continue;
            }
            let bucket = &mut flat_buckets[start..end];
            let mut valid_len = bucket.len();
            if valid_len > 1 {
                if valid_len <= 16 {
                    if bucket.windows(2).any(|w| w[0] >= w[1]) {
                        bucket.sort_unstable();
                        let mut write_idx = 1usize;
                        for read_idx in 1..valid_len {
                            if bucket[read_idx] != bucket[write_idx - 1] {
                                bucket[write_idx] = bucket[read_idx];
                                write_idx += 1;
                            }
                        }
                        valid_len = write_idx;
                    }
                } else {
                    stamp += 1;
                    let mut write_idx = 0usize;
                    let mut monotonic = true;
                    let mut prev = 0u32;
                    for idx in 0..valid_len {
                        let hi = bucket[idx];
                        let hi_idx = hi.0 as usize;
                        if seen[hi_idx] != stamp {
                            seen[hi_idx] = stamp;
                            if write_idx > 0 && hi.0 < prev {
                                monotonic = false;
                            }
                            prev = hi.0;
                            bucket[write_idx] = hi;
                            write_idx += 1;
                        }
                    }
                    valid_len = write_idx;
                    if !monotonic {
                        bucket[..valid_len].sort_unstable();
                    }
                }
            }
            let lower = rows[lo_row].virtual_register;
            for &higher in &bucket[..valid_len] {
                pairs.push(super::VirtualInterference { lower, higher });
            }
        }
        return pairs;
    }
    let mut by_block =
        std::collections::BTreeMap::<SelectedBlockId, Vec<(u32, u32, u32, VirtualRegisterId)>>::new(
        );
    for (row, range) in rows.iter().enumerate() {
        let vreg = range.virtual_register;
        for fragment in &range.fragments {
            if fragment.start < fragment.end {
                by_block.entry(fragment.block).or_default().push((
                    fragment.start.0,
                    fragment.end.0,
                    row as u32,
                    vreg,
                ));
            }
        }
    }
    let mut pairs = Vec::new();
    let mut active: Vec<(u32, u32, VirtualRegisterId)> = Vec::new();
    for fragments in by_block.values_mut() {
        fragments.sort_unstable_by_key(|&(start, _, row, _)| (start, row));
        active.clear();
        for &(start, end, row, vreg) in fragments.iter() {
            active.retain(|&(live_end, _, _)| start < live_end);
            for &(_, live_row, live_vreg) in &active {
                if live_row != row {
                    let (lower, higher) = if live_row < row {
                        (live_vreg, vreg)
                    } else {
                        (vreg, live_vreg)
                    };
                    pairs.push(super::VirtualInterference { lower, higher });
                }
            }
            active.push((end, row, vreg));
        }
    }
    pairs.sort_unstable();
    pairs.dedup();
    pairs
}

/// The `(lower, higher)` row pairs owning overlapping fragments in one block.
#[cfg(test)]
fn overlapping_rows(rows: &[&[LiveRangeFragment]]) -> BTreeSet<(usize, usize)> {
    let mut by_block =
        std::collections::BTreeMap::<SelectedBlockId, Vec<(LiveRangeFragment, usize)>>::new();
    for (row, fragments) in rows.iter().enumerate() {
        for fragment in fragments.iter() {
            by_block
                .entry(fragment.block)
                .or_default()
                .push((*fragment, row));
        }
    }
    let mut pairs = BTreeSet::new();
    let mut active: Vec<(LiveRangeFragment, usize)> = Vec::new();
    for fragments in by_block.values_mut() {
        fragments.sort_by_key(|(fragment, row)| (fragment.start, *row));
        active.clear();
        for &(fragment, row) in fragments.iter() {
            active.retain(|(live, _)| fragment.start < live.end);
            for &(live, live_row) in &active {
                if live_row != row && live.start < fragment.end && fragment.start < live.end {
                    pairs.insert((live_row.min(row), live_row.max(row)));
                }
            }
            active.push((fragment, row));
        }
    }
    pairs
}

pub(super) fn connector(
    source: SelectedBlockId,
    edge: &selected_instructions::SuccessorLiveness,
) -> LiveRangeEdgeConnector {
    LiveRangeEdgeConnector {
        source,
        terminator: edge.terminator,
        polarity_ordinal: edge.polarity_ordinal,
        psi_edge: edge.psi_edge,
        target: edge.target,
    }
}

pub(super) fn operand_point(
    function: usize,
    position: LivenessPosition,
    access: RegisterOperandAccess,
) -> Result<LiveRangePoint, LiveRangeError> {
    match access {
        RegisterOperandAccess::Use => before_point(function, position),
        RegisterOperandAccess::Def => after_point(function, position),
        RegisterOperandAccess::UseDef => Err(LiveRangeError::UnsupportedUseDef {
            function,
            instruction: 0,
            operand: 0,
        }),
    }
}

pub(super) fn before_point(
    function: usize,
    position: LivenessPosition,
) -> Result<LiveRangePoint, LiveRangeError> {
    position
        .0
        .checked_mul(2)
        .map(LiveRangePoint)
        .ok_or(LiveRangeError::PointOverflow { function })
}

pub(super) fn after_point(
    function: usize,
    position: LivenessPosition,
) -> Result<LiveRangePoint, LiveRangeError> {
    position
        .0
        .checked_mul(2)
        .and_then(|point| point.checked_add(1))
        .map(LiveRangePoint)
        .ok_or(LiveRangeError::PointOverflow { function })
}

#[cfg(test)]
mod tests {
    use super::{LiveRangeFragment, LiveRangePoint, SelectedBlockId, overlapping_rows};

    #[test]
    fn ordered_membership_matches_point_set_fragments() {
        // Exhaust every hole/adjacency pattern, repeated membership and block
        // boundary; block identity must keep even adjacent ranges separate.
        for membership in 0u32..1024 {
            let points = (0..10)
                .filter(|point| membership & (1 << point) != 0)
                .map(LiveRangePoint)
                .collect::<std::collections::BTreeSet<_>>();
            let mut actual = Vec::new();
            let mut expected = Vec::new();
            for block in [SelectedBlockId(9), SelectedBlockId(2)] {
                for point in &points {
                    super::append_point(0, &mut actual, block, *point).unwrap();
                    super::append_point(0, &mut actual, block, *point).unwrap();
                }
                expected.extend(super::fragments_from_points(block, points.clone()));
            }
            assert_eq!(actual, expected);
        }
        assert!(matches!(
            super::append_point(
                3,
                &mut Vec::new(),
                SelectedBlockId(0),
                LiveRangePoint(u32::MAX)
            ),
            Err(super::LiveRangeError::PointOverflow { function: 3 })
        ));
    }

    /// The sweep returns exactly the pairs a test of every row pair returns,
    /// including empty and touching fragments and rows spanning many blocks.
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
                .map(|_| {
                    (0..next(5))
                        .map(|_| {
                            let start = next(40);
                            LiveRangeFragment {
                                block: SelectedBlockId(next(3)),
                                start: LiveRangePoint(start),
                                end: LiveRangePoint(start + next(12)),
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let views = rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
            let mut expected = std::collections::BTreeSet::new();
            for (lower, left) in rows.iter().enumerate() {
                for (higher, right) in rows.iter().enumerate().skip(lower + 1) {
                    if left.iter().any(|left| {
                        right.iter().any(|right| {
                            left.block == right.block
                                && left.start < right.end
                                && right.start < left.end
                        })
                    }) {
                        expected.insert((lower, higher));
                    }
                }
            }
            assert_eq!(overlapping_rows(&views), expected);
        }
    }
}
