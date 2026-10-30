//! Captured block coordinates are address lookup authority, not loan resources.
//! A scalar continuation can outlive the temporary descriptor it read without
//! needing that obsolete coordinate. Keep a coordinate through every reachable
//! use, including cleanup and backedges, then expire it at successor entry.
//! Descriptor descendants independently retain their exact parent obligations;
//! forgetting a lookup supplies neither disposal nor restored permission.

use super::{BlockId, Custody, PlaceId, PsiOptimizationFunction};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
struct BlockUses {
    parameters: BTreeSet<PlaceId>,
    inputs: BTreeSet<PlaceId>,
    successors: BTreeSet<BlockId>,
}

/// Read the complete operation-use inventory, not descriptor-specific queries:
/// those may select one operand while a content transform has several parents.
/// Stores define contents, not new addresses; only an edge's receiving block
/// parameters end the previous instance of a borrowed coordinate.
pub(in crate::legalization::scalar_graph_input::reference_custody) fn live_coordinates(
    function: &PsiOptimizationFunction,
) -> Option<BTreeMap<BlockId, BTreeSet<PlaceId>>> {
    let blocks = function
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                BlockUses {
                    parameters: block
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.place)
                        .collect(),
                    inputs: block
                        .nodes
                        .iter()
                        .flat_map(|node| {
                            optimization_unit_semantics::operation_place_inputs(&node.operation)
                        })
                        .collect(),
                    successors: block.successors().map(|edge| edge.target).collect(),
                },
            )
        })
        .collect();
    live_blocks(&blocks)
}

fn live_blocks(
    blocks: &BTreeMap<BlockId, BlockUses>,
) -> Option<BTreeMap<BlockId, BTreeSet<PlaceId>>> {
    if blocks
        .values()
        .flat_map(|block| &block.successors)
        .any(|target| !blocks.contains_key(target))
    {
        return None;
    }
    let mut live = blocks
        .keys()
        .map(|block| (*block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for (block, uses) in blocks.iter().rev() {
            let mut incoming = uses.inputs.clone();
            for target in &uses.successors {
                incoming.extend(live[target].difference(&blocks[target].parameters).copied());
            }
            let previous = live.get_mut(block).expect("block was initialized");
            if *previous != incoming {
                *previous = incoming;
                changed = true;
            }
        }
        if !changed {
            return Some(live);
        }
    }
}

/// Capture/bind and selected cleanup run first. Only the next block's dead
/// lookup coordinates expire; its resident and retired loan graphs are intact.
/// An unavailable graph supplies no proof of death and retains every source.
pub(in crate::legalization::scalar_graph_input::reference_custody) fn expire_coordinates(
    custody: &mut Custody,
    target: BlockId,
) {
    let Some(live) = custody
        .descriptor_backing
        .borrowed_live
        .as_ref()
        .and_then(|live| live.get(&target))
    else {
        return;
    };
    for (place, source) in std::sync::Arc::make_mut(&mut custody.borrowed_sources) {
        if !live.contains(place) {
            *source = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(number: u64) -> BlockId {
        BlockId::new(number).unwrap()
    }

    #[test]
    fn later_reads_remain_live_through_branches_and_backedges() {
        let place = PlaceId::new(7).unwrap();
        let blocks = BTreeMap::from([
            (
                block(1),
                BlockUses {
                    successors: BTreeSet::from([block(2), block(4)]),
                    ..Default::default()
                },
            ),
            (
                block(2),
                BlockUses {
                    successors: BTreeSet::from([block(3)]),
                    ..Default::default()
                },
            ),
            (
                block(3),
                BlockUses {
                    inputs: BTreeSet::from([place]),
                    successors: BTreeSet::from([block(1)]),
                    ..Default::default()
                },
            ),
            (block(4), BlockUses::default()),
        ]);
        let live = live_blocks(&blocks).unwrap();
        for number in [1, 2, 3] {
            assert!(live[&block(number)].contains(&place));
        }
        assert!(!live[&block(4)].contains(&place));
    }

    #[test]
    fn incomplete_graph_proves_no_coordinate_dead() {
        let blocks = BTreeMap::from([(
            block(1),
            BlockUses {
                successors: BTreeSet::from([block(2)]),
                ..Default::default()
            },
        )]);
        assert!(live_blocks(&blocks).is_none());
    }

    #[test]
    fn rebinding_a_coordinate_does_not_keep_its_previous_instance_live() {
        let place = PlaceId::new(7).unwrap();
        let blocks = BTreeMap::from([
            (
                block(1),
                BlockUses {
                    successors: BTreeSet::from([block(2)]),
                    ..Default::default()
                },
            ),
            (
                block(2),
                BlockUses {
                    parameters: BTreeSet::from([place]),
                    inputs: BTreeSet::from([place]),
                    ..Default::default()
                },
            ),
        ]);
        let live = live_blocks(&blocks).unwrap();
        assert!(!live[&block(1)].contains(&place));
        assert!(live[&block(2)].contains(&place));
    }
}
