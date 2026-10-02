//! Independent point, fragment, block-domain, and edge reconstruction.

use std::collections::BTreeSet;

use crate::LiveRangeError;
use register_model::RegisterOperandAccess;
use selected_instructions::SelectedBlockId;
use selected_instructions::{
    BlockPointDomain, LiveRangeEdgeConnector, LiveRangeFragment, LiveRangePoint,
};

pub(super) fn block_domains(
    function: usize,
    live: &selected_instructions::FunctionLiveness,
) -> Result<Vec<BlockPointDomain>, LiveRangeError> {
    live.blocks
        .iter()
        .map(|block| {
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
                .expect("first established nonempty");
            Ok(BlockPointDomain {
                block: block.block,
                source_block: block.source_block,
                start: checked_before(function, first.position.0)?,
                end: LiveRangePoint(
                    checked_after(function, last.position.0)?
                        .0
                        .checked_add(1)
                        .ok_or(LiveRangeError::PointOverflow { function })?,
                ),
            })
        })
        .collect()
}

pub(super) fn operand_point(
    function: usize,
    operand: &selected_instructions::OperandPosition,
) -> Result<LiveRangePoint, LiveRangeError> {
    match operand.access {
        RegisterOperandAccess::Use => checked_before(function, operand.position.0),
        RegisterOperandAccess::Def => checked_after(function, operand.position.0),
        RegisterOperandAccess::UseDef => Err(LiveRangeError::UnsupportedUseDef {
            function,
            instruction: operand.instruction.0,
            operand: operand.operand,
        }),
    }
}

pub(super) fn checked_before(
    function: usize,
    position: u32,
) -> Result<LiveRangePoint, LiveRangeError> {
    position
        .checked_mul(2)
        .map(LiveRangePoint)
        .ok_or(LiveRangeError::PointOverflow { function })
}

pub(super) fn checked_after(
    function: usize,
    position: u32,
) -> Result<LiveRangePoint, LiveRangeError> {
    position
        .checked_mul(2)
        .and_then(|point| point.checked_add(1))
        .map(LiveRangePoint)
        .ok_or(LiveRangeError::PointOverflow { function })
}

pub(super) fn append_maximal(
    block: SelectedBlockId,
    occupied: BTreeSet<LiveRangePoint>,
    output: &mut Vec<LiveRangeFragment>,
) {
    let mut points = occupied.into_iter();
    let Some(first) = points.next() else { return };
    let mut start = first;
    let mut previous = first;
    for current in points {
        if previous.0.checked_add(1) == Some(current.0) {
            previous = current;
            continue;
        }
        output.push(LiveRangeFragment {
            block,
            start,
            end: LiveRangePoint(previous.0 + 1),
        });
        start = current;
        previous = current;
    }
    output.push(LiveRangeFragment {
        block,
        start,
        end: LiveRangePoint(previous.0 + 1),
    });
}

pub(super) fn edge_row(
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

pub(super) fn overlaps(left: &[LiveRangeFragment], right: &[LiveRangeFragment]) -> bool {
    // These are independently reconstructed fragments, not submitted evidence.
    // Revalidated liveness numbers instructions densely across the function;
    // append_maximal emits disjoint intervals in that same execution order.
    // Walk those coordinates, not block IDs (which need not describe execution
    // order). An interval ending first cannot meet any later opposing interval.
    let mut left_remaining = left;
    let mut right_remaining = right;
    while let (Some(first), Some(second)) = (left_remaining.first(), right_remaining.first()) {
        if first.block == second.block && first.start < second.end && second.start < first.end {
            return true;
        }
        if first.end <= second.end {
            left_remaining = &left_remaining[1..];
        } else {
            right_remaining = &right_remaining[1..];
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_overlap_matches_point_membership_across_blocks_and_gaps() {
        // Exhaust every membership mask over two four-point block domains.
        // Reversed sparse block IDs ensure the walk follows points, not IDs.
        let ranges = (0u16..256)
            .map(|mask| {
                let mut fragments = Vec::new();
                for (offset, block) in [(0, SelectedBlockId(u32::MAX)), (4, SelectedBlockId(7))] {
                    let occupied = (offset..offset + 4)
                        .filter(|point| mask & (1 << point) != 0)
                        .map(LiveRangePoint)
                        .collect();
                    append_maximal(block, occupied, &mut fragments);
                }
                fragments
            })
            .collect::<Vec<_>>();
        for (left_mask, left) in ranges.iter().enumerate() {
            for (right_mask, right) in ranges.iter().enumerate() {
                assert_eq!(
                    overlaps(left, right),
                    left_mask & right_mask != 0,
                    "left mask {left_mask}, right mask {right_mask}"
                );
            }
        }
    }

    #[test]
    fn ordered_overlap_retains_block_identity_and_half_open_endpoint() {
        let interval = LiveRangeFragment {
            block: SelectedBlockId(9),
            start: LiveRangePoint(u32::MAX - 2),
            end: LiveRangePoint(u32::MAX),
        };
        assert!(overlaps(&[interval], &[interval]));
        let mut other = interval;
        other.block = SelectedBlockId(10);
        assert!(!overlaps(&[interval], &[other]));
        let earlier = LiveRangeFragment {
            block: interval.block,
            start: LiveRangePoint(0),
            end: interval.start,
        };
        assert!(!overlaps(&[earlier], &[interval]));
        assert!(!overlaps(&[interval], &[earlier]));
    }
}
