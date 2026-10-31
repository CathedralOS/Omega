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
    // Keep the exact reachable universe, including either supplied row domain.
    // Each predecessor intersection then works on packed dominator words.
    let mut complete = vec![0_u64; incoming.len().max(outgoing.len()).div_ceil(64)];
    for position in reachable {
        complete[position / 64] |= 1_u64 << (position % 64);
    }
    let mut dominators = vec![complete; incoming.len()];
    dominators[entry].fill(0);
    dominators[entry][entry / 64] |= 1_u64 << (entry % 64);
    loop {
        let mut changed = false;
        for (position, predecessors) in incoming.iter().enumerate() {
            if position == entry {
                continue;
            }
            let (first, others) = predecessors.split_first()?;
            let mut current = dominators[*first].clone();
            for predecessor in others {
                for (word, predecessor_word) in current.iter_mut().zip(&dominators[*predecessor]) {
                    *word &= *predecessor_word;
                }
            }
            current[position / 64] |= 1_u64 << (position % 64);
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
    let depths: Vec<usize> = dominators
        .iter()
        .map(|words| words.iter().map(|word| word.count_ones() as usize).sum())
        .collect();
    order
        .into_iter()
        .map(|position| {
            if position == entry {
                Some((position, None))
            } else {
                dominator_positions(&dominators[position])
                    .filter(|dominator| *dominator != position)
                    .max_by_key(|dominator| depths[*dominator])
                    .map(|dominator| (position, Some(dominator)))
            }
        })
        .collect()
}

// Word order and trailing-bit selection preserve ascending set iteration,
// including the original last-position choice when dominator depths tie.
fn dominator_positions(words: &[u64]) -> impl Iterator<Item = usize> + '_ {
    words.iter().enumerate().flat_map(|(word_index, &word)| {
        let mut remaining = word;
        std::iter::from_fn(move || {
            if remaining == 0 {
                return None;
            }
            let bit = remaining.trailing_zeros() as usize;
            remaining &= remaining - 1;
            Some(word_index * 64 + bit)
        })
    })
}

#[cfg(test)]
#[path = "dominance/tests.rs"]
mod tests;
