//! Keep exact dominator membership without copying a tree of block IDs for
//! every block. Every declared block participates, including unreachable ones;
//! the equations retain the original greatest fixed point.

use semantic_vocabulary::BlockId;

pub struct DominatorSets {
    blocks: Vec<BlockId>,
    rows: Vec<Vec<u64>>,
}

impl DominatorSets {
    pub fn get(&self, block: &BlockId) -> Option<DominatorSet<'_>> {
        let position = self.blocks.binary_search(block).ok()?;
        Some(DominatorSet {
            blocks: &self.blocks,
            words: &self.rows[position],
        })
    }
}

pub struct DominatorSet<'graph> {
    blocks: &'graph [BlockId],
    words: &'graph [u64],
}

impl DominatorSet<'_> {
    pub fn contains(&self, block: &BlockId) -> bool {
        self.blocks
            .binary_search(block)
            .is_ok_and(|position| self.words[position / 64] & (1 << (position % 64)) != 0)
    }
}

impl std::fmt::Debug for DominatorSet<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_set()
            .entries(
                self.blocks
                    .iter()
                    .enumerate()
                    .filter_map(|(position, block)| {
                        (self.words[position / 64] & (1 << (position % 64)) != 0).then_some(block)
                    }),
            )
            .finish()
    }
}

// The public entry resolves every predecessor against this same sorted roster.
// Only its in-range ordinals enter the solver.
pub(super) fn solve(
    blocks: Vec<BlockId>,
    entry_position: Option<usize>,
    incoming: Vec<Vec<usize>>,
) -> DominatorSets {
    let mut outgoing = vec![Vec::new(); blocks.len()];
    for (target, predecessors) in incoming.iter().enumerate() {
        for predecessor in predecessors {
            outgoing[*predecessor].push(target);
        }
    }
    let order = reverse_postorder(&outgoing, entry_position);
    let word_count = blocks.len().div_ceil(64);
    let mut universe = vec![u64::MAX; word_count];
    if !blocks.len().is_multiple_of(64)
        && let Some(last) = universe.last_mut()
    {
        *last = (1 << (blocks.len() % 64)) - 1;
    }
    let mut rows = vec![universe; blocks.len()];
    let mut dirty = vec![true; blocks.len()];
    if let Some(position) = entry_position {
        rows[position].fill(0);
        rows[position][position / 64] |= 1 << (position % 64);
        dirty[position] = false;
    }
    let mut pending = blocks.len() - usize::from(entry_position.is_some());
    let mut next = vec![0; word_count];
    while pending != 0 {
        for &position in &order {
            if !dirty[position] {
                continue;
            }
            dirty[position] = false;
            pending -= 1;
            if let Some(first) = incoming[position].first() {
                next.copy_from_slice(&rows[*first]);
            } else {
                next.fill(0);
            }
            for predecessor in incoming[position].iter().skip(1) {
                for (word, predecessor_word) in next.iter_mut().zip(&rows[*predecessor]) {
                    *word &= predecessor_word;
                }
            }
            next[position / 64] |= 1 << (position % 64);
            if rows[position] != next {
                rows[position].copy_from_slice(&next);
                for &successor in &outgoing[position] {
                    if !dirty[successor] {
                        dirty[successor] = true;
                        pending += 1;
                    }
                }
            }
        }
    }
    DominatorSets { blocks, rows }
}

fn reverse_postorder(outgoing: &[Vec<usize>], entry: Option<usize>) -> Vec<usize> {
    let mut seen = vec![false; outgoing.len()];
    let mut finished = Vec::with_capacity(outgoing.len());
    let mut pending = Vec::new();
    for root in entry.into_iter().chain(0..outgoing.len()) {
        if seen[root] {
            continue;
        }
        seen[root] = true;
        pending.push((root, 0));
        while let Some((position, next)) = pending.last_mut() {
            if let Some(&successor) = outgoing[*position].get(*next) {
                *next += 1;
                if !seen[successor] {
                    seen[successor] = true;
                    pending.push((successor, 0));
                }
            } else {
                finished.push(*position);
                pending.pop();
            }
        }
    }
    finished.reverse();
    finished
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dominators;
    use std::collections::{BTreeMap, BTreeSet};

    fn block(position: usize) -> BlockId {
        BlockId::new(101 + position as u64 * 3).unwrap()
    }

    fn direct_sets(
        entry: BlockId,
        blocks: &[BlockId],
        predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    ) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
        let all: BTreeSet<_> = blocks.iter().copied().collect();
        let mut result: BTreeMap<_, _> = all
            .iter()
            .map(|&block| {
                (
                    block,
                    if block == entry {
                        BTreeSet::from([entry])
                    } else {
                        all.clone()
                    },
                )
            })
            .collect();
        loop {
            let mut changed = false;
            for &block in all.iter().filter(|&&block| block != entry) {
                let incoming = &predecessors[&block];
                let mut next = incoming
                    .first()
                    .map_or_else(BTreeSet::new, |first| result[first].clone());
                for predecessor in incoming.iter().skip(1) {
                    next = next.intersection(&result[predecessor]).copied().collect();
                }
                next.insert(block);
                if result[&block] != next {
                    result.insert(block, next);
                    changed = true;
                }
            }
            if !changed {
                return result;
            }
        }
    }

    #[test]
    fn packed_membership_matches_full_sets_for_every_small_graph() {
        for count in 1usize..=4 {
            let blocks: Vec<_> = (0..count).map(block).collect();
            for edges in 0..1usize << (count * count) {
                let mut predecessors: BTreeMap<_, BTreeSet<_>> = blocks
                    .iter()
                    .map(|&block| (block, BTreeSet::new()))
                    .collect();
                for source in 0..count {
                    for target in 0..count {
                        if edges & (1 << (source * count + target)) != 0 {
                            predecessors
                                .get_mut(&blocks[target])
                                .unwrap()
                                .insert(blocks[source]);
                        }
                    }
                }
                // An absent entry is intentional too: the old equations do not
                // impose reachability or invent a root for disconnected graphs.
                for entry in blocks.iter().copied().chain([block(count)]) {
                    let expected = direct_sets(entry, &blocks, &predecessors);
                    let packed = dominators(
                        entry,
                        blocks.iter().copied().rev().chain(blocks.iter().copied()),
                        &predecessors,
                    );
                    for target in &blocks {
                        let actual = packed.get(target).unwrap();
                        for source in &blocks {
                            assert_eq!(
                                actual.contains(source),
                                expected[target].contains(source),
                                "graph {count}/{edges}, entry {entry:?}, {source:?}->{target:?}"
                            );
                        }
                        assert!(!actual.contains(&block(count)));
                    }
                    assert!(packed.get(&block(count)).is_none());
                }
            }
        }
        let empty = dominators(block(0), std::iter::empty(), &BTreeMap::new());
        assert!(empty.get(&block(0)).is_none());
        let ignored = BTreeMap::from([
            (block(0), BTreeSet::from([block(9)])),
            (block(8), BTreeSet::from([block(9)])),
        ]);
        assert!(
            dominators(block(0), [block(0), block(0)].into_iter(), &ignored)
                .get(&block(0))
                .unwrap()
                .contains(&block(0))
        );
    }

    #[test]
    fn reversed_chains_keep_exact_membership_across_word_boundaries() {
        for count in [63usize, 64, 65, 8192] {
            let blocks: Vec<_> = (0..count).map(block).collect();
            let predecessors: BTreeMap<_, _> = blocks
                .iter()
                .enumerate()
                .map(|(position, &block)| {
                    (
                        block,
                        blocks.get(position + 1).copied().into_iter().collect(),
                    )
                })
                .collect();
            let packed = dominators(blocks[count - 1], blocks.iter().copied(), &predecessors);
            assert_eq!(packed.rows.len(), count);
            assert_eq!(
                packed.rows.iter().map(Vec::len).sum::<usize>(),
                count * count.div_ceil(64)
            );
            for position in 0..count {
                let set = packed.get(&blocks[position]).unwrap();
                assert!(set.contains(&blocks[position]));
                assert!(set.contains(&blocks[count - 1]));
                if position > 0 {
                    assert!(!set.contains(&blocks[position - 1]));
                }
                if count <= 65 {
                    for (member_position, member) in blocks.iter().enumerate() {
                        assert_eq!(set.contains(member), member_position >= position);
                    }
                }
            }
            if !count.is_multiple_of(64) {
                assert!(
                    packed
                        .rows
                        .iter()
                        .all(|row| row.last().unwrap() >> (count % 64) == 0)
                );
            }
        }
    }
}
