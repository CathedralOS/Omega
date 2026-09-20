//! Exact source-fragment topology and legality-point partitioning.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    FixedPrecoloredSplitRequirementError, LiveRangeEdgeConnector, LiveRangeFragment,
    VirtualLiveRange, VirtualPointLegality, VirtualRegisterAllocationLegality,
};

pub(super) struct FragmentInput<'a> {
    pub(super) fragment: &'a LiveRangeFragment,
    pub(super) points: &'a [VirtualPointLegality],
    pub(super) incoming: Option<LiveRangeEdgeConnector>,
}

/// Admit a disjoint union of source-rooted fragment trees. A fragment whose
/// block carries no incoming connector opens a fresh component: edge
/// parameter bindings hand the value to a different register at the boundary,
/// so a block parameter's fragment is live-in with no connector even when it
/// is not the range's first fragment. The mirrored case is a connector that
/// ends at a block holding no fragment — the argument register's range simply
/// terminates at that edge — and is tolerated once every fragment is placed.
/// Joins (two connectors into one target), cycles, and connectors arriving
/// from a not-yet-admitted fragment still fail closed.
pub(super) fn tree<'a>(
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
    let mut connectors = BTreeMap::new();
    for connector in &range.edge_connectors {
        if connectors.insert(connector.target, *connector).is_some() {
            return Err(
                FixedPrecoloredSplitRequirementError::UnsupportedCrossBlockRange {
                    function,
                    register,
                },
            );
        }
    }

    let mut point_offset = 0usize;
    let mut admitted = BTreeSet::new();
    let mut inputs = Vec::with_capacity(range.fragments.len());
    for fragment in &range.fragments {
        let width =
            fragment.end.0.checked_sub(fragment.start.0).ok_or(
                FixedPrecoloredSplitRequirementError::IntervalOverflow { function, register },
            )?;
        let width = usize::try_from(width).map_err(|_| {
            FixedPrecoloredSplitRequirementError::IntervalOverflow { function, register }
        })?;
        let end = point_offset
            .checked_add(width)
            .ok_or(FixedPrecoloredSplitRequirementError::IntervalOverflow { function, register })?;
        let points = legality.points.get(point_offset..end).ok_or(
            FixedPrecoloredSplitRequirementError::NonCanonicalPointDomain {
                function,
                register,
                point: fragment.start.0,
            },
        )?;
        point_offset = end;
        // The connector dedup above keys by target, and every fragment's block
        // is probed exactly once, so anything left in `connectors` after this
        // loop necessarily targets a block with no fragment — a transport
        // exit — and is dropped.
        match connectors.remove(&fragment.block) {
            Some(connector) if admitted.contains(&connector.source) => {
                inputs.push(FragmentInput {
                    fragment,
                    points,
                    incoming: Some(connector),
                });
            }
            None => {
                inputs.push(FragmentInput {
                    fragment,
                    points,
                    incoming: None,
                });
            }
            _ => {
                return Err(
                    FixedPrecoloredSplitRequirementError::UnsupportedCrossBlockRange {
                        function,
                        register,
                    },
                );
            }
        }
        admitted.insert(fragment.block);
    }
    if point_offset != legality.points.len() {
        return Err(
            FixedPrecoloredSplitRequirementError::UnsupportedCrossBlockRange { function, register },
        );
    }
    Ok(inputs)
}
