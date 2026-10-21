//! Storage-definition flow is a set of local ordinals, not a tree per block.
//! Flat bit rows keep the common one-definition case allocation-free per block.
//! A store fixes its block's exit regardless of incoming writers; seed every
//! such exit, then visit store-free blocks only when their incoming bits grow.
//! This computes the same least union fixed point, including disconnected stores.
use std::collections::VecDeque;

pub(super) struct DefinitionEntries {
    words: usize,
    entries: Vec<u64>,
}
impl DefinitionEntries {
    pub(super) fn compute(count: usize, last: &[Option<usize>], successors: &[Vec<usize>]) -> Self {
        let words = count.div_ceil(64);
        let mut result = Self {
            words,
            entries: vec![0; words * last.len()],
        };
        let mut pending: VecDeque<_> = last
            .iter()
            .enumerate()
            .filter_map(|(block, value)| value.map(|_| block))
            .collect();
        let mut queued: Vec<_> = last.iter().map(Option::is_some).collect();
        let mut state = result.scratch();
        while let Some(block) = pending.pop_front() {
            queued[block] = false;
            if let Some(definition) = last[block] {
                replace(&mut state, definition);
            } else {
                result.copy_into(block, &mut state);
            }
            for &destination in &successors[block] {
                let row = &mut result.entries[destination * words..(destination + 1) * words];
                let mut changed = false;
                for (word, incoming) in row.iter_mut().zip(&state) {
                    let merged = *word | *incoming;
                    changed |= merged != *word;
                    *word = merged;
                }
                if changed && last[destination].is_none() && !queued[destination] {
                    queued[destination] = true;
                    pending.push_back(destination);
                }
            }
        }
        result
    }
    pub(super) fn scratch(&self) -> Vec<u64> {
        vec![0; self.words]
    }
    pub(super) fn copy_into(&self, block: usize, state: &mut [u64]) {
        state.copy_from_slice(&self.entries[block * self.words..(block + 1) * self.words]);
    }
}
pub(super) fn replace(state: &mut [u64], definition: usize) {
    state.fill(0);
    state[definition / 64] = 1u64 << (definition % 64);
}
pub(super) fn members(state: &[u64]) -> impl Iterator<Item = usize> + '_ {
    state
        .iter()
        .copied()
        .enumerate()
        .flat_map(|(word, mut bits)| {
            std::iter::from_fn(move || {
                if bits == 0 {
                    return None;
                }
                let bit = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                Some(word * 64 + bit)
            })
        })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cyclic_joins_preserve_definition_bits_across_word_boundaries() {
        let entries = DefinitionEntries::compute(
            130,
            &[Some(0), Some(65), Some(129), None],
            &[vec![3], vec![3], vec![3], vec![0, 1, 2]],
        );
        let mut state = entries.scratch();
        for block in 0..4 {
            entries.copy_into(block, &mut state);
            assert_eq!(members(&state).collect::<Vec<_>>(), vec![0, 65, 129]);
        }
        let entries = DefinitionEntries::compute(
            65,
            &[Some(63), None, Some(64), None],
            &[vec![1], vec![2], vec![3], vec![]],
        );
        for (block, expected) in [vec![], vec![63], vec![63], vec![64]]
            .into_iter()
            .enumerate()
        {
            entries.copy_into(block, &mut state[..2]);
            assert_eq!(members(&state[..2]).collect::<Vec<_>>(), expected);
        }
    }
}
