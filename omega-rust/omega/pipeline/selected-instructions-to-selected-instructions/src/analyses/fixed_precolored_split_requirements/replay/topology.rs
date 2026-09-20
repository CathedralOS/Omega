//! Independently reconstructed disjoint union of source-rooted fragment
//! trees, keyed by block identity rather than positional assumptions.

use std::collections::{HashMap, HashSet};

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
    if range.fragments.is_empty() {
        return Err(
            FixedPrecoloredSplitRequirementError::MissingSourceFragment { function, register },
        );
    }
    let mut incoming = HashMap::new();
    for edge in &range.edge_connectors {
        if incoming.insert(edge.target.0, *edge).is_some() {
            return cross_block(function, register);
        }
    }

    let mut cursor = 0usize;
    let mut admitted = HashSet::new();
    let mut result = Vec::with_capacity(range.fragments.len());
    for source in range.fragments.iter() {
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
        // Every fragment's block is probed once under the target-keyed dedup,
        // so any connector left after the loop targets a block holding no
        // fragment: a transport exit where the argument register's range ends
        // at the edge while the bound parameter's range opens inside.
        let edge = incoming.remove(&source.block.0);
        match edge {
            Some(connector) if admitted.contains(&connector.source.0) => {}
            None => {}
            _ => return cross_block(function, register),
        }
        admitted.insert(source.block.0);
        result.push(FragmentInput {
            source,
            points,
            incoming: edge,
        });
    }
    if cursor != legality.points.len() {
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
