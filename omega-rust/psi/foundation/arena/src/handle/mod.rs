//! Arena addressing: the typed `Handle` and the contiguous `HandleSpan`.
//!
//! Identity comparisons and accessors sit inside loops across pipeline crates.
//! Keep these tiny operations available for cross-crate inlining: otherwise
//! opt-level-1 consumers pay a call for each handle comparison or component read.
//! Equality and hashing still observe both the arena position and generation.

pub(crate) mod handle_span;

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

pub struct Handle<T> {
    arena_index: u32,
    generation: u32,
    marker: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    #[inline]
    pub const fn invalid() -> Self {
        Self {
            arena_index: 0,
            generation: 0,
            marker: PhantomData,
        }
    }

    #[inline]
    pub const fn from_arena_index(arena_index: u32) -> Self {
        Self {
            arena_index,
            generation: 1,
            marker: PhantomData,
        }
    }

    #[inline]
    pub const fn from_parts(arena_index: u32, generation: u32) -> Self {
        Self {
            arena_index,
            generation,
            marker: PhantomData,
        }
    }

    #[inline]
    pub const fn arena_index(self) -> u32 {
        self.arena_index
    }

    #[inline]
    pub const fn generation(self) -> u32 {
        self.generation
    }

    #[inline]
    pub const fn is_valid(self) -> bool {
        self.arena_index != 0
    }
}

impl<T> Clone for Handle<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Handle<T> {}

impl<T> Default for Handle<T> {
    #[inline]
    fn default() -> Self {
        Self::invalid()
    }
}

impl<T> PartialEq for Handle<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.arena_index == other.arena_index && self.generation == other.generation
    }
}

impl<T> Eq for Handle<T> {}

impl<T> Hash for Handle<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.arena_index.hash(state);
        self.generation.hash(state);
    }
}

impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Handle")
            .field("arena_index", &self.arena_index)
            .field("generation", &self.generation)
            .finish()
    }
}

/// Retain each complete identity once, in first-appearance order.
/// This compares opaque identities only: it neither dereferences handles nor
/// establishes that they are valid in an arena. Stable sorting retains the
/// first occurrence before restoring the caller's order.
pub fn unique_handles_in_order<T>(handles: impl IntoIterator<Item = Handle<T>>) -> Vec<Handle<T>> {
    let mut ordered: Vec<_> = handles
        .into_iter()
        .enumerate()
        .map(|(order, handle)| (handle, order))
        .collect();
    ordered.sort_by_key(|(handle, _)| (handle.arena_index(), handle.generation()));
    ordered.dedup_by_key(|(handle, _)| (handle.arena_index(), handle.generation()));
    ordered.sort_by_key(|(_, order)| *order);
    ordered.into_iter().map(|(handle, _)| handle).collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{Handle, unique_handles_in_order};

    struct NonHashItem;

    #[test]
    fn hash_uses_the_complete_untyped_handle_identity() {
        let first = Handle::<NonHashItem>::from_parts(7, 2);
        let same = Handle::<NonHashItem>::from_parts(7, 2);
        let next_generation = Handle::<NonHashItem>::from_parts(7, 3);

        assert_eq!(HashSet::from([first, same, next_generation]).len(), 2);
    }

    #[test]
    fn unique_handles_preserve_generations_and_first_appearance() {
        let handles = [
            Handle::<NonHashItem>::from_parts(8, 1),
            Handle::from_parts(7, 2),
            Handle::from_parts(7, 1),
            Handle::invalid(),
        ];
        // Compare with the original growing identity scan for every short
        // sequence, including empty input, interleaved repeats and absence.
        for length in 0..=5 {
            for sequence_number in 0..handles.len().pow(length) {
                let mut digits = sequence_number;
                let mut sequence = Vec::new();
                for _ in 0..length {
                    sequence.push(handles[digits % handles.len()]);
                    digits /= handles.len();
                }
                let mut expected = Vec::new();
                for handle in &sequence {
                    if !expected.contains(handle) {
                        expected.push(*handle);
                    }
                }
                assert_eq!(unique_handles_in_order(sequence), expected);
            }
        }
    }
}
