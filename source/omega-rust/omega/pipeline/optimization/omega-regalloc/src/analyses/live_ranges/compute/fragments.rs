//! Block domains, virtual fragments, connectors, and point arithmetic.

use super::*;

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

pub(super) fn virtual_fragments(
    function: usize,
    block: &BlockLiveness,
    register: VirtualRegisterId,
) -> Result<Vec<LiveRangeFragment>, LiveRangeError> {
    let mut points = BTreeSet::new();
    for instruction in &block.instructions {
        if instruction.virtual_live_in.contains(&register)
            || instruction.virtual_uses.contains(&register)
        {
            points.insert(before_point(function, instruction.position)?);
        }
        if instruction.virtual_live_out.contains(&register)
            || instruction.virtual_defs.contains(&register)
        {
            points.insert(after_point(function, instruction.position)?);
        }
    }
    Ok(fragments_from_points(block.block, points))
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
    edge: &crate::SuccessorLiveness,
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
