//! Dominance over an emitted block graph, shared by observation unification
//! and declaration-ordered cleanup. Coordinates are positions in the caller's
//! graph; this query grants no source, storage, or custody authority.
use std::collections::BTreeSet;

/// Every path into a block passes through each member of its dominator set.
/// The caller supplies exact graph positions. An absent entry or unreachable
/// block retains the universal set, as in the observation-unification owner.
pub(crate) fn block_dominators(
    entry: Option<usize>,
    successors: &[Vec<usize>],
) -> Vec<BTreeSet<usize>> {
    let count = successors.len();
    if count == 0 {
        return Vec::new();
    }
    let words = count.div_ceil(64);
    let mut universal_bits = vec![!0u64; words];
    let tail_bits = count % 64;
    if tail_bits != 0 {
        universal_bits[words - 1] = (1u64 << tail_bits) - 1;
    }
    let Some(entry) = entry else {
        let universal: BTreeSet<usize> = (0..count).collect();
        return vec![universal; count];
    };
    let mut predecessors = vec![Vec::new(); count];
    for (block, outs) in successors.iter().enumerate() {
        for &successor in outs {
            predecessors[successor].push(block);
        }
    }
    let mut dom_bits = vec![universal_bits.clone(); count];
    let mut entry_bits = vec![0u64; words];
    entry_bits[entry / 64] |= 1u64 << (entry % 64);
    dom_bits[entry] = entry_bits;
    let mut next_bits = vec![0u64; words];
    loop {
        let mut changed = false;
        for block in 0..count {
            if block == entry {
                continue;
            }
            next_bits.copy_from_slice(&universal_bits);
            for &predecessor in &predecessors[block] {
                for (word, pred_word) in next_bits.iter_mut().zip(&dom_bits[predecessor]) {
                    *word &= *pred_word;
                }
            }
            next_bits[block / 64] |= 1u64 << (block % 64);
            if next_bits != dom_bits[block] {
                dom_bits[block].copy_from_slice(&next_bits);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    dom_bits
        .into_iter()
        .map(|bits| {
            let mut set = BTreeSet::new();
            for (word_index, mut word) in bits.into_iter().enumerate() {
                while word != 0 {
                    let tz = word.trailing_zeros() as usize;
                    set.insert(word_index * 64 + tz);
                    word &= word - 1;
                }
            }
            set
        })
        .collect()
}
