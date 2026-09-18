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

/// Admit a one-block range or a source-rooted fragment tree: every non-first
/// fragment takes exactly one incoming edge connector whose source is an
/// *earlier* fragment's block. The source-block fanout is the shallow case;
/// deeper chains and trees qualify the same way because partition order keeps
/// every parent fragment's closing domain available before its children.
/// Joined, cyclic, or sourceless connectors stay unsupported.
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
    if (range.fragments.len() == 1 && !connectors.is_empty())
        || (range.fragments.len() > 1 && connectors.len() != range.fragments.len() - 1)
    {
        return Err(
            FixedPrecoloredSplitRequirementError::UnsupportedCrossBlockRange { function, register },
        );
    }

    let mut point_offset = 0usize;
    let mut admitted = BTreeSet::new();
    let mut inputs = Vec::with_capacity(range.fragments.len());
    for (fragment_offset, fragment) in range.fragments.iter().enumerate() {
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
        let incoming = if fragment_offset == 0 {
            None
        } else {
            connectors.remove(&fragment.block)
        };
        match incoming {
            Some(connector) if admitted.contains(&connector.source) => {}
            None if fragment_offset == 0 => {}
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
        inputs.push(FragmentInput {
            fragment,
            points,
            incoming,
        });
    }
    if point_offset != legality.points.len() || !connectors.is_empty() {
        return Err(
            FixedPrecoloredSplitRequirementError::UnsupportedCrossBlockRange { function, register },
        );
    }
    Ok(inputs)
}
