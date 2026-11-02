//! Exact source-fragment topology and legality-point partitioning.

use std::collections::{BTreeMap, BTreeSet};

use crate::FixedPrecoloredSplitRequirementError;
use register_homes::{VirtualLegalitySpan, VirtualRegisterAllocationLegality};
use selected_instructions::{LiveRangeEdgeConnector, LiveRangeFragment, VirtualLiveRange};

pub(super) struct FragmentInput<'a> {
    pub(super) fragment: &'a LiveRangeFragment,
    pub(super) spans: &'a [VirtualLegalitySpan],
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

    let mut span_offset = 0usize;
    let mut admitted = BTreeSet::new();
    let mut inputs = Vec::with_capacity(range.fragments.len());
    for fragment in &range.fragments {
        if fragment.start > fragment.end {
            return Err(FixedPrecoloredSplitRequirementError::IntervalOverflow {
                function,
                register,
            });
        }
        let first_span = span_offset;
        let mut expected = fragment.start;
        while expected < fragment.end {
            let span = legality
                .spans
                .get(span_offset)
                .filter(|span| {
                    span.block == fragment.block
                        && span.start == expected
                        && span.start < span.end
                        && span.end <= fragment.end
                })
                .ok_or(
                    FixedPrecoloredSplitRequirementError::NonCanonicalPointDomain {
                        function,
                        register,
                        point: expected.0,
                    },
                )?;
            expected = span.end;
            span_offset += 1;
        }
        let spans = &legality.spans[first_span..span_offset];
        // The connector dedup above keys by target, and every fragment's block
        // is probed exactly once, so anything left in `connectors` after this
        // loop necessarily targets a block with no fragment — a transport
        // exit — and is dropped.
        match connectors.remove(&fragment.block) {
            Some(connector) if admitted.contains(&connector.source) => {
                inputs.push(FragmentInput {
                    fragment,
                    spans,
                    incoming: Some(connector),
                });
            }
            None => {
                inputs.push(FragmentInput {
                    fragment,
                    spans,
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
    if span_offset != legality.spans.len() {
        return Err(
            FixedPrecoloredSplitRequirementError::UnsupportedCrossBlockRange { function, register },
        );
    }
    Ok(inputs)
}
