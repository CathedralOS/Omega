//! Independent point, fragment, block-domain, and edge reconstruction.

use std::collections::BTreeSet;

use crate::{
    BlockPointDomain, LiveRangeEdgeConnector, LiveRangeError, LiveRangeFragment, LiveRangePoint,
};
use register_model::RegisterOperandAccess;
use selected_instructions::SelectedBlockId;

pub(super) fn block_domains(
    function: usize,
    live: &crate::FunctionLiveness,
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
    operand: &crate::OperandPosition,
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

pub(super) fn overlaps(left: &[LiveRangeFragment], right: &[LiveRangeFragment]) -> bool {
    for first in left {
        for second in right {
            if first.block == second.block
                && first.start.0 < second.end.0
                && second.start.0 < first.end.0
            {
                return true;
            }
        }
    }
    false
}
