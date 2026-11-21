//! Dominance over an explicitly prepared local graph, including virtual entries.

/// A node's interval in the immediate-dominator forest. The graph adapter owns
/// the association between these local slots and its validated block identities.
pub struct DominanceInterval {
    traversal_start: usize,
    traversal_end: usize,
    depth: usize,
}

impl DominanceInterval {
    pub fn dominates(&self, used: &Self) -> bool {
        self.traversal_start <= used.traversal_start && used.traversal_start < self.traversal_end
    }

    pub fn depth(&self) -> usize {
        self.depth
    }
}

/// Compute immediate dominators with Cooper, Harvey and Kennedy's iterative
/// reverse-postorder algorithm, then retain only traversal intervals. Its space
/// is linear in the supplied topology; construction is iterative, not a claim
/// of worst-case linear running time.
/// https://hipersoft.cs.rice.edu/grads/publications/dom14.pdf
///
/// Every root has an independent virtual entry, even if real edges enter it.
/// `ordered` must contain each local slot once in reverse postorder. Missing
/// roots, foreign slots, and nodes unreachable from the roots return `None`.
pub fn dominance_forest(
    predecessors: &[Vec<usize>],
    ordered: &[usize],
    roots: &[bool],
) -> Option<Vec<DominanceInterval>> {
    let count = predecessors.len();
    if ordered.len() != count || roots.len() != count {
        return None;
    }
    let mut positions = vec![usize::MAX; count];
    for (position, &block) in ordered.iter().enumerate() {
        let slot = positions.get_mut(block)?;
        if *slot != usize::MAX {
            return None;
        }
        *slot = position + 1;
    }
    if predecessors.iter().flatten().any(|&block| block >= count) {
        return None;
    }
    // Position zero is the virtual entry. Actual nodes are ranked from one,
    // so all roots meet there without inventing a dominating executable block.
    let mut immediate = vec![usize::MAX; count + 1];
    immediate[0] = 0;
    for (block, &is_root) in roots.iter().enumerate() {
        if is_root {
            immediate[positions[block]] = 0;
        }
    }
    loop {
        let mut changed = false;
        for (offset, &block) in ordered.iter().enumerate() {
            if roots[block] {
                continue;
            }
            let position = offset + 1;
            let mut common = usize::MAX;
            for &predecessor in &predecessors[block] {
                let predecessor = positions[predecessor];
                if immediate[predecessor] == usize::MAX {
                    continue;
                }
                common = if common == usize::MAX {
                    predecessor
                } else {
                    intersect(common, predecessor, &immediate)
                };
            }
            if common != usize::MAX && common >= position {
                return None;
            }
            if immediate[position] != common {
                immediate[position] = common;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    if immediate.contains(&usize::MAX) {
        return None;
    }
    // The tree is construction scratch, represented by links rather than one
    // small allocation per block. Only the interval rows survive publication.
    let mut first_child = vec![usize::MAX; count + 1];
    let mut next_sibling = vec![usize::MAX; count + 1];
    for position in 1..=count {
        let parent = immediate[position];
        next_sibling[position] = first_child[parent];
        first_child[parent] = position;
    }
    let mut nodes: Vec<_> = (0..count)
        .map(|_| DominanceInterval {
            traversal_start: 0,
            traversal_end: 0,
            depth: 0,
        })
        .collect();
    let mut clock = 0;
    let mut pending = vec![(0, first_child[0])];
    while let Some((parent, child)) = pending.last_mut() {
        if *child == usize::MAX {
            if *parent != 0 {
                nodes[ordered[*parent - 1]].traversal_end = clock;
            }
            pending.pop();
            continue;
        }
        let position = *child;
        *child = next_sibling[position];
        let depth = if *parent == 0 {
            1
        } else {
            nodes[ordered[*parent - 1]].depth + 1
        };
        let node = &mut nodes[ordered[position - 1]];
        node.traversal_start = clock;
        node.depth = depth;
        clock += 1;
        pending.push((position, first_child[position]));
    }
    Some(nodes)
}

fn intersect(mut first: usize, mut second: usize, immediate: &[usize]) -> usize {
    while first != second {
        if first > second {
            first = immediate[first];
        } else {
            second = immediate[second];
        }
    }
    first
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diamond_and_multiple_entries_keep_exact_dominators() {
        let predecessors = vec![vec![], vec![0], vec![0], vec![1, 2]];
        let nodes =
            dominance_forest(&predecessors, &[0, 2, 1, 3], &[true, false, false, false]).unwrap();
        assert!(nodes[0].dominates(&nodes[3]));
        assert!(!nodes[1].dominates(&nodes[3]));
        assert_eq!(nodes[3].depth(), 2);
        let nodes =
            dominance_forest(&predecessors, &[0, 2, 1, 3], &[true, false, true, false]).unwrap();
        assert!(!nodes[0].dominates(&nodes[3]));
        assert_eq!(nodes[3].depth(), 1);
    }

    #[test]
    fn incomplete_or_foreign_topology_has_no_tree() {
        assert!(dominance_forest(&[], &[], &[]).unwrap().is_empty());
        assert!(dominance_forest(&[vec![]], &[], &[true]).is_none());
        assert!(dominance_forest(&[vec![]], &[1], &[true]).is_none());
        assert!(dominance_forest(&[vec![]], &[0], &[]).is_none());
        assert!(dominance_forest(&[vec![1]], &[0], &[true]).is_none());
        assert!(dominance_forest(&[vec![]], &[0], &[false]).is_none());
        assert!(dominance_forest(&[vec![0]], &[0], &[false]).is_none());
        assert!(dominance_forest(&[vec![], vec![0]], &[0, 0], &[true, false]).is_none());
        assert!(dominance_forest(&[vec![], vec![0]], &[1, 0], &[true, false]).is_none());
    }
}
