//! Captured block coordinates are address lookup authority, not loan resources.
//! A scalar continuation can outlive the temporary descriptor it read without
//! needing that obsolete coordinate. Keep a coordinate through every reachable
//! use, including cleanup and backedges, then expire it at successor entry.
//! Descriptor descendants independently retain their exact parent obligations;
//! forgetting a lookup supplies neither disposal nor restored permission.

use super::{BlockId, Custody, PlaceId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_semantics::descriptor_lifetimes::{DescriptorFlowStep, DescriptorPosition};

/// Use the existing exhaustive operation-input adapter. Unknown case edges
/// remain reachable here. Borrowed coordinates are block parameter names;
/// only rebinding those parameters ends their old instance. A store defines
/// contents, not a new address, so descriptor definitions never kill a name.
/// Each successor's own uses keep its newly bound instance live.
pub(in crate::legalization::scalar_graph_input::reference_custody) fn live_coordinates(
    flow: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
) -> Option<BTreeMap<DescriptorPosition, BTreeSet<PlaceId>>> {
    if flow
        .values()
        .flat_map(|step| &step.successors)
        .any(|edge| !flow.contains_key(&edge.target))
    {
        return None;
    }
    let mut live = flow
        .keys()
        .map(|position| (*position, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for (position, step) in flow.iter().rev() {
            let mut incoming = BTreeSet::<PlaceId>::new();
            for edge in &step.successors {
                if let Some(successor) = live.get(&edge.target) {
                    incoming.extend(successor.iter().copied().filter(|place| {
                        !edge
                            .parameters
                            .iter()
                            .any(|parameter| parameter.root == *place)
                    }));
                }
            }
            incoming.extend(step.inputs.iter().map(|input| input.root));
            let previous = live
                .get_mut(position)
                .expect("flow position was initialized");
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
/// An unavailable flow supplies no proof of death and retains every source.
pub(in crate::legalization::scalar_graph_input::reference_custody) fn expire_coordinates(
    custody: &mut Custody,
    target: BlockId,
) {
    let Some(live) = custody
        .descriptor_backing
        .borrowed_live
        .as_ref()
        .and_then(|live| live.get(&(target, 0)))
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
    use terminal_semantics::descriptor_lifetimes::{DescriptorFlowEdge, DescriptorLocation};

    fn position(number: u64) -> DescriptorPosition {
        (BlockId::new(number).unwrap(), 0)
    }

    fn successor(target: u64) -> DescriptorFlowEdge {
        DescriptorFlowEdge {
            target: position(target),
            parameters: Vec::new(),
            bindings: Vec::new(),
        }
    }

    #[test]
    fn later_reads_remain_live_through_stores_and_backedges() {
        let place = PlaceId::new(7).unwrap();
        let location = DescriptorLocation::whole(place);
        let flow = BTreeMap::from([
            (
                position(1),
                DescriptorFlowStep {
                    successors: vec![successor(2), successor(4)],
                    ..Default::default()
                },
            ),
            (
                position(2),
                DescriptorFlowStep {
                    // Replacing referent contents cannot redefine its address.
                    definitions: vec![location.clone()],
                    successors: vec![successor(3)],
                    ..Default::default()
                },
            ),
            (
                position(3),
                DescriptorFlowStep {
                    inputs: vec![location],
                    successors: vec![successor(1)],
                    ..Default::default()
                },
            ),
            (position(4), DescriptorFlowStep::default()),
        ]);
        let live = live_coordinates(&flow).unwrap();
        for number in [1, 2, 3] {
            assert!(live[&position(number)].contains(&place));
        }
        assert!(!live[&position(4)].contains(&place));
    }

    #[test]
    fn incomplete_flow_proves_no_coordinate_dead() {
        let flow = BTreeMap::from([(
            position(1),
            DescriptorFlowStep {
                successors: vec![successor(2)],
                ..Default::default()
            },
        )]);
        assert!(live_coordinates(&flow).is_none());
    }

    #[test]
    fn rebinding_a_coordinate_does_not_keep_its_previous_instance_live() {
        let place = PlaceId::new(7).unwrap();
        let location = DescriptorLocation::whole(place);
        let mut edge = successor(2);
        edge.parameters.push(location.clone());
        let flow = BTreeMap::from([
            (
                position(1),
                DescriptorFlowStep {
                    successors: vec![edge],
                    ..Default::default()
                },
            ),
            (
                position(2),
                DescriptorFlowStep {
                    inputs: vec![location],
                    ..Default::default()
                },
            ),
        ]);
        let live = live_coordinates(&flow).unwrap();
        assert!(!live[&position(1)].contains(&place));
        assert!(live[&position(2)].contains(&place));
    }
}
