//! Definition scheduling only: no cut executable edges or ranking conclusions.
use std::collections::{BTreeSet, VecDeque};

pub(crate) fn schedule(
    incoming: &[Vec<usize>],
    outgoing: &[Vec<usize>],
    entry: usize,
) -> Option<Vec<(usize, Option<usize>)>> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![entry];
    while let Some(position) = pending.pop() {
        if reachable.insert(position) {
            pending.extend(outgoing.get(position)?.iter().copied());
        }
    }
    if reachable.len() != incoming.len() {
        return None;
    }
    let mut dominators = vec![reachable; incoming.len()];
    dominators[entry] = BTreeSet::from([entry]);
    loop {
        let mut changed = false;
        for (position, predecessors) in incoming.iter().enumerate() {
            if position == entry {
                continue;
            }
            let (first, others) = predecessors.split_first()?;
            let mut current = dominators[*first].clone();
            for predecessor in others {
                current.retain(|candidate| dominators[*predecessor].contains(candidate));
            }
            current.insert(position);
            if current != dominators[position] {
                dominators[position] = current;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // First-arrival order gives every non-entry block an actual incoming exit
    // before its replay. Strict dominators still own ordinary definitions;
    // custody instead follows that incoming edge and checks every later arrival,
    // including backedges. Sorting by dominator depth could visit a diamond's
    // join before the deeper arm that transports its descriptors.
    let mut order = Vec::with_capacity(incoming.len());
    let mut visited = BTreeSet::new();
    let mut pending = VecDeque::from([entry]);
    while let Some(position) = pending.pop_front() {
        if visited.insert(position) {
            order.push(position);
            pending.extend(outgoing[position].iter().copied());
        }
    }
    order
        .into_iter()
        .map(|position| {
            if position == entry {
                Some((position, None))
            } else {
                dominators[position]
                    .iter()
                    .copied()
                    .filter(|dominator| *dominator != position)
                    .max_by_key(|dominator| dominators[*dominator].len())
                    .map(|dominator| (position, Some(dominator)))
            }
        })
        .collect()
}
