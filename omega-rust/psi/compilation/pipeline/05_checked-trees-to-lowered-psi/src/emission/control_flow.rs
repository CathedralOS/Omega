//! Dominance over an emitted block graph, shared by observation unification
//! and declaration-ordered cleanup. Coordinates are positions in the caller's
//! graph; this query grants no source, storage, or custody authority.
use std::collections::BTreeSet;

/// Every path into a block passes through each member of its dominator set.
/// The caller supplies exact graph positions. An absent entry or unreachable
/// block retains the universal set, as in the observation-unification owner.
pub(super) fn block_dominators(
    entry: Option<usize>,
    successors: &[Vec<usize>],
) -> Vec<BTreeSet<usize>> {
    let count = successors.len();
    let mut predecessors = vec![Vec::new(); count];
    for (block, outs) in successors.iter().enumerate() {
        for &successor in outs {
            predecessors[successor].push(block);
        }
    }
    let universal: BTreeSet<usize> = (0..count).collect();
    let mut dominators = vec![universal.clone(); count];
    let Some(entry) = entry else {
        return dominators;
    };
    dominators[entry] = BTreeSet::from([entry]);
    loop {
        let mut changed = false;
        for block in 0..count {
            if block == entry {
                continue;
            }
            let mut next = universal.clone();
            for &predecessor in &predecessors[block] {
                next = next
                    .intersection(&dominators[predecessor])
                    .copied()
                    .collect();
            }
            next.insert(block);
            if next != dominators[block] {
                dominators[block] = next;
                changed = true;
            }
        }
        if !changed {
            return dominators;
        }
    }
}
