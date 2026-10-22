//! Payload sharing begins at an explicit publication boundary, not per row.
//! Ordinary construction stays inline. Shared pages detach on the first write,
//! including writes obtained through mutable slices; readers keep stable handles.
use crate::Arena;
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

pub(super) enum ArenaPage<T> {
    Owned(Arena<T>),
    Shared {
        arena: Arc<Arena<T>>,
        copy: fn(&Arena<T>) -> Arena<T>,
    },
}
impl<T> From<Arena<T>> for ArenaPage<T> {
    fn from(arena: Arena<T>) -> Self {
        Self::Owned(arena)
    }
}
impl<T> Deref for ArenaPage<T> {
    type Target = Arena<T>;
    fn deref(&self) -> &Arena<T> {
        match self {
            Self::Owned(arena) => arena,
            Self::Shared { arena, .. } => arena,
        }
    }
}
impl<T: Default> DerefMut for ArenaPage<T> {
    fn deref_mut(&mut self) -> &mut Arena<T> {
        if let Self::Shared { arena, copy } = self {
            // Complete a potentially panicking payload clone before replacing
            // the retained page. The set's counts stay valid during unwinding.
            if Arc::strong_count(arena) > 1 {
                let detached = copy(arena);
                *self = Self::Owned(detached);
            } else {
                let Self::Shared { arena, .. } =
                    std::mem::replace(self, Self::Owned(Arena::default()))
                else {
                    unreachable!()
                };
                // Shared ownership is private to this type; it never publishes
                // Weak pointers, so a unique page cannot acquire another owner.
                *self = Self::Owned(match Arc::try_unwrap(arena) {
                    Ok(arena) => arena,
                    Err(_) => unreachable!("unique arena page gained an owner"),
                });
            }
        }
        match self {
            Self::Owned(arena) => arena,
            Self::Shared { .. } => unreachable!(),
        }
    }
}
impl<T: Default + Clone> ArenaPage<T> {
    pub(super) fn share(&mut self) {
        if matches!(self, Self::Owned(arena) if !arena.is_empty()) {
            let Self::Owned(arena) = std::mem::replace(self, Self::Owned(Arena::default())) else {
                unreachable!()
            };
            *self = Self::Shared {
                arena: Arc::new(arena),
                copy: Arena::clone,
            };
        }
    }
}
impl<T: Clone> Clone for ArenaPage<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Owned(arena) => Self::Owned(arena.clone()),
            Self::Shared { arena, copy } => Self::Shared {
                arena: arena.clone(),
                copy: *copy,
            },
        }
    }
    fn clone_from(&mut self, other: &Self) {
        match (self, other) {
            (Self::Owned(destination), Self::Owned(source)) => destination.clone_from(source),
            (destination, source) => *destination = source.clone(),
        }
    }
}
impl<T: std::fmt::Debug> std::fmt::Debug for ArenaPage<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.deref().fmt(formatter)
    }
}
impl<T: PartialEq> PartialEq for ArenaPage<T> {
    fn eq(&self, other: &Self) -> bool {
        self.deref() == other.deref()
    }
}
impl<T: Eq> Eq for ArenaPage<T> {}
