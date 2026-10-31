//! Mutable storage for the two declaration columns that supply operator tokens.
//!
//! A mutable arena escape invalidates its owner identity before the caller can
//! edit or replace rows. Whole-column replacement carries its matching identity;
//! independently constructed storage starts with a fresh one.
//! Declaration indexes retain that identity strongly; recycled allocations and
//! equal local revisions cannot make another table look like the old one.
use arena::Arena;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

#[derive(Clone, Default)]
pub(super) struct DeclarationStorageIdentity(Arc<()>);

impl PartialEq for DeclarationStorageIdentity {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for DeclarationStorageIdentity {}

/// Operator and domain declaration storage. The physical owner identity is
/// neither semantic content nor persisted graph data. Clones retain the identity
/// of their equal contents; the first mutable escape separates them.
#[derive(Clone)]
pub struct SpelledDeclarationTable<T> {
    arena: Arena<T>,
    identity: DeclarationStorageIdentity,
}

impl<T: Default> Default for SpelledDeclarationTable<T> {
    fn default() -> Self {
        Arena::default().into()
    }
}
impl<T> From<Arena<T>> for SpelledDeclarationTable<T> {
    fn from(arena: Arena<T>) -> Self {
        Self {
            arena,
            identity: DeclarationStorageIdentity::default(),
        }
    }
}
impl<T> Deref for SpelledDeclarationTable<T> {
    type Target = Arena<T>;
    fn deref(&self) -> &Self::Target {
        &self.arena
    }
}
impl<T> DerefMut for SpelledDeclarationTable<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.identity = DeclarationStorageIdentity::default();
        &mut self.arena
    }
}
impl<T> SpelledDeclarationTable<T> {
    pub(super) fn storage_identity(&self) -> DeclarationStorageIdentity {
        self.identity.clone()
    }
}
impl<T: PartialEq> PartialEq for SpelledDeclarationTable<T> {
    fn eq(&self, other: &Self) -> bool {
        self.arena == other.arena
    }
}
impl<T: Eq> Eq for SpelledDeclarationTable<T> {}
impl<T: std::fmt::Debug> std::fmt::Debug for SpelledDeclarationTable<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.arena, formatter)
    }
}

// Preserve the exact Arena encoding. The new owner identity starts cold on
// recovery and conveys no declaration or proof authority.
impl<T: compiler_storage::Record> compiler_storage::Record for SpelledDeclarationTable<T> {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        compiler_storage::Record::write(&self.arena, output)
    }
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        <Arena<T> as compiler_storage::Record>::read(input).map(Self::from)
    }
}
