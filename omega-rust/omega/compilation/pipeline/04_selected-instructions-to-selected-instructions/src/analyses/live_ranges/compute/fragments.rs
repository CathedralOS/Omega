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

/// Stream before/after membership directly into block-local ranges. Liveness
/// owns instruction order, so a repeated point is a duplicate and an adjacent
/// point extends the last fragment. There is no per-point tree to build, clone,
/// and collapse, nor a register-by-block rescan to recover the same ordering.
pub(super) fn register_fragments(
    function: usize,
    liveness: &selected_instructions::FunctionLiveness,
    registers: &BTreeSet<VirtualRegisterId>,
) -> Result<std::collections::BTreeMap<VirtualRegisterId, Vec<LiveRangeFragment>>, LiveRangeError> {
    let mut fragments = registers
        .iter()
        .map(|register| (*register, Vec::new()))
        .collect::<std::collections::BTreeMap<_, _>>();
    for block in &liveness.blocks {
        for instruction in &block.instructions {
            for (live, accessed, point) in [
                (
                    &instruction.virtual_live_in,
                    &instruction.virtual_uses,
                    before_point(function, instruction.position)?,
                ),
                (
                    &instruction.virtual_live_out,
                    &instruction.virtual_defs,
                    after_point(function, instruction.position)?,
                ),
            ] {
                for register in live.iter().chain(accessed) {
                    if let Some(rows) = fragments.get_mut(register) {
                        append_point(function, rows, block.block, point)?;
                    }
                }
            }
        }
    }
    Ok(fragments)
}

/// Append one point from an ordered block traversal, tolerating membership
/// duplicates (a value can be both live and explicitly accessed).
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
