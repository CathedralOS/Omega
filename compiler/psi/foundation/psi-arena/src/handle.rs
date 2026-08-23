use std::fmt;
use std::marker::PhantomData;

pub struct Handle<T> {
    arena_index: u32,
    generation: u32,
    marker: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    pub const fn invalid() -> Self {
        Self {
            arena_index: 0,
            generation: 0,
            marker: PhantomData,
        }
    }

    pub const fn from_arena_index(arena_index: u32) -> Self {
        Self {
            arena_index,
            generation: 1,
            marker: PhantomData,
        }
    }

    pub const fn from_parts(arena_index: u32, generation: u32) -> Self {
        Self {
            arena_index,
            generation,
            marker: PhantomData,
        }
    }

    pub const fn arena_index(self) -> u32 {
        self.arena_index
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }

    pub const fn is_valid(self) -> bool {
        self.arena_index != 0
    }
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Handle<T> {}

impl<T> Default for Handle<T> {
    fn default() -> Self {
        Self::invalid()
    }
}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.arena_index == other.arena_index && self.generation == other.generation
    }
}

impl<T> Eq for Handle<T> {}

impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Handle")
            .field("arena_index", &self.arena_index)
            .field("generation", &self.generation)
            .finish()
    }
}
