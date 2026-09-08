//! Definition scheduling only: no cut executable edges or ranking conclusions.
use std::collections::BTreeSet;

pub(super) fn schedule(
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
    // Strict dominators form a chain. Their exit definitions are available on
    // every arrival, including a backedge. Predecessor-only observations are
    // never imported; destination parameters are introduced by the block itself.
    let mut order = (0..incoming.len()).collect::<Vec<_>>();
    order.sort_by_key(|position| dominators[*position].len());
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
