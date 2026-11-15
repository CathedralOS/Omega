//! Source components give disconnected control flow conservative virtual entries.
use super::{BlockId, index};

pub(super) fn virtual_entries(
    entry: BlockId,
    predecessors: &[Vec<BlockId>],
    successors: &[Vec<BlockId>],
    ordered: &[BlockId],
) -> Vec<bool> {
    let count = predecessors.len();
    let mut reachable = vec![false; count];
    let mut pending = vec![entry];
    while let Some(block) = pending.pop() {
        let slot = index(block.get());
        if std::mem::replace(&mut reachable[slot], true) {
            continue;
        }
        pending.extend(successors[slot].iter().copied());
    }
    // Ordinary lowered bodies have one entry-reachable component. They need
    // no SCC preparation and can reuse the reachability allocation for roots.
    if reachable.iter().all(|&reached| reached) {
        reachable.fill(false);
        reachable[index(entry.get())] = true;
        return reachable;
    }
    // Kosaraju's reverse walk in decreasing forward finish order identifies
    // SCCs without recursion. Only the entry-unreachable subgraph participates.
    let mut components = vec![usize::MAX; count];
    let mut component_count = 0;
    for &root in ordered {
        let slot = index(root.get());
        if reachable[slot] || components[slot] != usize::MAX {
            continue;
        }
        pending.push(root);
        while let Some(block) = pending.pop() {
            let slot = index(block.get());
            if reachable[slot] || components[slot] != usize::MAX {
                continue;
            }
            components[slot] = component_count;
            pending.extend(predecessors[slot].iter().copied());
        }
        component_count += 1;
    }
    let mut source_components = vec![true; component_count];
    for (block, incoming) in predecessors.iter().enumerate() {
        if reachable[block] {
            continue;
        }
        for source in incoming {
            let source = index(source.get());
            if !reachable[source] && components[source] != components[block] {
                source_components[components[block]] = false;
            }
        }
    }
    let mut roots = vec![false; count];
    roots[index(entry.get())] = true;
    for block in 0..count {
        // Root every member of a disconnected source cycle: no arbitrary
        // cycle member or unrelated reachable block may supply its premises.
        // Blocks downstream of that component still keep their dominators.
        if !reachable[block] && source_components[components[block]] {
            roots[block] = true;
        }
    }
    roots
}
