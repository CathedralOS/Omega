use crate::{Aarch64MovnPatch, Aarch64MovnRecipe};

pub(crate) fn zero_seed_word_count(bits: u64) -> u8 {
    1 + (1..4)
        .filter(|halfword| ((bits >> (halfword * 16)) & 0xffff) != 0)
        .count() as u8
}

pub(crate) fn movn_recipe(bits: u64) -> Aarch64MovnRecipe {
    let chunks = [
        bits as u16,
        (bits >> 16) as u16,
        (bits >> 32) as u16,
        (bits >> 48) as u16,
    ];
    let seed_halfword = chunks
        .iter()
        .position(|chunk| *chunk != u16::MAX)
        .unwrap_or(0) as u8;
    let patches = chunks
        .iter()
        .enumerate()
        .filter(|(halfword, chunk)| *halfword != usize::from(seed_halfword) && **chunk != u16::MAX)
        .map(|(halfword, immediate)| Aarch64MovnPatch {
            halfword: halfword as u8,
            immediate: *immediate,
        })
        .collect();
    Aarch64MovnRecipe {
        seed_halfword,
        seed_immediate: !chunks[usize::from(seed_halfword)],
        patches,
    }
}
