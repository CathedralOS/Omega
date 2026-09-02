//! Independently reconstructed one-block or single-entry-fanout topology.

use std::collections::HashMap;

use crate::{
    FixedPrecoloredSplitRequirementError, LiveRangeEdgeConnector, LiveRangeFragment,
    VirtualLiveRange, VirtualPointLegality, VirtualRegisterAllocationLegality,
};

pub(super) struct FragmentInput<'a> {
    pub(super) source: &'a LiveRangeFragment,
    pub(super) points: &'a [VirtualPointLegality],
    pub(super) incoming: Option<LiveRangeEdgeConnector>,
}

pub(super) fn reconstruct<'a>(
    function: usize,
    range: &'a VirtualLiveRange,
    legality: &'a VirtualRegisterAllocationLegality,
) -> Result<Vec<FragmentInput<'a>>, FixedPrecoloredSplitRequirementError> {
    let register = range.virtual_register.0;
    let Some(entry) = range.fragments.first() else {
        return Err(
            FixedPrecoloredSplitRequirementError::MissingSourceFragment { function, register },
        );
    };
    let mut incoming = HashMap::new();
    for edge in &range.edge_connectors {
        if edge.source != entry.block || incoming.insert(edge.target.0, *edge).is_some() {
            return cross_block(function, register);
        }
    }
    if incoming.len() != range.fragments.len().saturating_sub(1) {
        return cross_block(function, register);
    }

    let mut cursor = 0usize;
    let mut result = Vec::with_capacity(range.fragments.len());
    for (index, source) in range.fragments.iter().enumerate() {
        let width =
            source.end.0.checked_sub(source.start.0).ok_or(
                FixedPrecoloredSplitRequirementError::IntervalOverflow { function, register },
            )?;
        let width = usize::try_from(width).map_err(|_| {
            FixedPrecoloredSplitRequirementError::IntervalOverflow { function, register }
        })?;
        let limit = cursor
            .checked_add(width)
            .ok_or(FixedPrecoloredSplitRequirementError::IntervalOverflow { function, register })?;
        let points = legality.points.get(cursor..limit).ok_or(
            FixedPrecoloredSplitRequirementError::NonCanonicalPointDomain {
                function,
                register,
                point: source.start.0,
            },
        )?;
        cursor = limit;
        let edge = (index != 0)
            .then(|| incoming.remove(&source.block.0))
            .flatten();
        if index != 0 && edge.is_none() {
            return cross_block(function, register);
        }
        result.push(FragmentInput {
            source,
            points,
            incoming: edge,
        });
    }
    if cursor != legality.points.len() || !incoming.is_empty() {
        return cross_block(function, register);
    }
    Ok(result)
}

fn cross_block<T>(
    function: usize,
    register: u32,
) -> Result<T, FixedPrecoloredSplitRequirementError> {
    Err(FixedPrecoloredSplitRequirementError::UnsupportedCrossBlockRange { function, register })
}
