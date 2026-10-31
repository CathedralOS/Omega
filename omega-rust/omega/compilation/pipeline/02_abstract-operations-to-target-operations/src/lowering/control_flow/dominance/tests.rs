use super::{dominator_positions, schedule};
use std::collections::{BTreeSet, VecDeque};

fn original_schedule(
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

fn compare(outgoing: &[Vec<usize>], entry: usize) {
    let mut incoming = vec![Vec::new(); outgoing.len()];
    for (from, targets) in outgoing.iter().enumerate() {
        for &to in targets {
            incoming[to].push(from);
        }
    }
    assert_eq!(
        schedule(&incoming, outgoing, entry),
        original_schedule(&incoming, outgoing, entry),
        "entry={entry}, outgoing={outgoing:?}",
    );
}

#[test]
fn packed_dominators_match_original_sets_for_all_small_graphs_and_boundaries() {
    // Every directed graph through four positions, with all entry choices:
    // self edges, irreducible cycles and unreachable rosters are included.
    for count in 1..=4 {
        for mask in 0_u32..(1_u32 << (count * count)) {
            let mut outgoing = vec![Vec::new(); count];
            for (from, targets) in outgoing.iter_mut().enumerate() {
                for to in 0..count {
                    if mask & (1 << (from * count + to)) != 0 {
                        targets.push(to);
                    }
                }
            }
            for entry in 0..count {
                compare(&outgoing, entry);
            }
        }
    }
    compare(&[], 0);
    compare(&[vec![1, 2], vec![3], vec![3], vec![1, 2, 4], vec![]], 0);
    compare(&[vec![1, 1, 2], vec![3, 3], vec![3], vec![]], 0);
    for count in [63, 64, 65, 127, 128, 130] {
        // A nonzero entry with both a long cycle and shared successor joins
        // exercises complete sets and memberships across word boundaries.
        let mut outgoing = vec![Vec::new(); count];
        for (from, targets) in outgoing.iter_mut().enumerate() {
            targets.push((from + 1) % count);
            if from % 3 == 0 {
                targets.push((from + 2) % count);
                targets.push((from + 2) % count);
            }
        }
        compare(&outgoing, count / 2);
        outgoing[0].clear();
        compare(&outgoing, count / 2);
    }
    // The existing missing-predecessor and invalid-successor refusals stay None.
    assert_eq!(schedule(&[vec![], vec![]], &[vec![1], vec![]], 0), None);
    assert_eq!(schedule(&[vec![]], &[vec![1]], 0), None);
    assert_eq!(
        schedule(&[vec![]], &[vec![1]], 0),
        original_schedule(&[vec![]], &[vec![1]], 0)
    );
    // Mismatched row domains keep the original result, not a new policy.
    for (incoming, outgoing, entry) in [
        (vec![vec![], vec![0]], vec![vec![1], vec![], vec![]], 0),
        (vec![vec![]], vec![vec![], vec![]], 0),
    ] {
        assert_eq!(
            schedule(&incoming, &outgoing, entry),
            original_schedule(&incoming, &outgoing, entry),
        );
    }
    // Explicitly pin ascending enumeration and unused final-word bits.
    assert_eq!(
        dominator_positions(&[1_u64 << 63, 3, 2]).collect::<Vec<_>>(),
        vec![63, 64, 65, 129]
    );
}
