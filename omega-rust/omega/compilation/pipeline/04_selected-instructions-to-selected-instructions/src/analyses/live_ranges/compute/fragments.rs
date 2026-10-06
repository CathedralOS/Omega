//! Block domains, virtual fragments, connectors, and point arithmetic.
use super::{
    BTreeSet, BlockLiveness, BlockPointDomain, LiveRangeEdgeConnector, LiveRangeFragment,
    LiveRangePoint, LivenessPosition, RegisterOperandAccess, SelectedBlockId, VirtualRegisterId,
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

/// Each selected register's live points, by block position in `liveness`,
/// gathered in one pass over every instruction's live and access sets. A
/// register live into or used by an instruction is live just before it; one
/// live out of or defined by it is live just after. A register's range reads
/// its own points from this rather than rescanning every instruction's sets.
pub(super) fn register_points(
    function: usize,
    liveness: &selected_instructions::FunctionLiveness,
    registers: &BTreeSet<VirtualRegisterId>,
) -> Result<
    std::collections::BTreeMap<
        VirtualRegisterId,
        std::collections::BTreeMap<usize, BTreeSet<LiveRangePoint>>,
    >,
    LiveRangeError,
> {
    let mut points = std::collections::BTreeMap::<
        VirtualRegisterId,
        std::collections::BTreeMap<usize, BTreeSet<LiveRangePoint>>,
    >::new();
    for (block_index, block) in liveness.blocks.iter().enumerate() {
        for instruction in &block.instructions {
            let sides = [
                (
                    &instruction.virtual_live_in,
                    &instruction.virtual_uses,
                    before_point
                        as fn(usize, LivenessPosition) -> Result<LiveRangePoint, LiveRangeError>,
                ),
                (
                    &instruction.virtual_live_out,
                    &instruction.virtual_defs,
                    after_point,
                ),
            ];
            for (live, accessed, side_point) in sides {
                let mut members = live
                    .iter()
                    .chain(accessed)
                    .filter(|register| registers.contains(register))
                    .peekable();
                if members.peek().is_none() {
                    continue;
                }
                let point = side_point(function, instruction.position)?;
                for register in members {
                    points
                        .entry(*register)
                        .or_default()
                        .entry(block_index)
                        .or_default()
                        .insert(point);
                }
            }
        }
    }
    Ok(points)
}

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
pub(super) fn interference(
    rows: &[super::VirtualLiveRange],
) -> BTreeSet<super::VirtualInterference> {
    let fragments = rows
        .iter()
        .map(|range| range.fragments.as_slice())
        .collect::<Vec<_>>();
    overlapping_rows(&fragments)
        .into_iter()
        .map(|(lower, higher)| super::VirtualInterference {
            lower: rows[lower].virtual_register,
            higher: rows[higher].virtual_register,
        })
        .collect()
}

/// The `(lower, higher)` row pairs owning overlapping fragments in one block.
/// Each block's fragments are swept in start order against the ones still
/// live; a fragment ending at or before the current start overlaps nothing
/// that starts later and leaves the active set, and each remaining candidate
/// takes the exact half-open overlap test. A large function costs its sorted
/// sweep plus the live pressure at each point, not one comparison per row
/// pair, and yields the same pairs as testing every row pair.
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
