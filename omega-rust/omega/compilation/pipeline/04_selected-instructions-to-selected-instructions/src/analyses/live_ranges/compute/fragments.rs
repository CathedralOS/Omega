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

pub(super) fn fragments_overlap(left: &[LiveRangeFragment], right: &[LiveRangeFragment]) -> bool {
    left.iter().any(|left| {
        right.iter().any(|right| {
            left.block == right.block && left.start < right.end && right.start < left.end
        })
    })
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
