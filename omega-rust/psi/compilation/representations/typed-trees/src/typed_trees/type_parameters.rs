//! Parameter storage owns its complete symbol index, including negative answers.
//! Mutable arena access invalidates it before exposing storage. Cloning or
//! replacing the table carries only the index belonging to those declarations.
use crate::data::TypeParameter;
use arena::{Arena, Handle, OwnedIndex};
use symbols::SymbolHandle;

#[derive(Clone, Default)]
pub(super) struct TypeParameterIndex {
    pub(super) rows: Vec<(SymbolHandle, Handle<TypeParameter>)>,
}

#[derive(Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct TypeParameterTable {
    arena: Arena<TypeParameter>,
    index: OwnedIndex<TypeParameterIndex, ()>,
}

impl From<Arena<TypeParameter>> for TypeParameterTable {
    fn from(arena: Arena<TypeParameter>) -> Self {
        Self {
            arena,
            index: OwnedIndex::default(),
        }
    }
}
impl std::fmt::Debug for TypeParameterTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.arena.fmt(f)
    }
}
impl std::ops::Deref for TypeParameterTable {
    type Target = Arena<TypeParameter>;
    fn deref(&self) -> &Self::Target {
        &self.arena
    }
}
impl std::ops::DerefMut for TypeParameterTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.index.clear();
        &mut self.arena
    }
}
impl TypeParameterTable {
    pub(super) fn with_index<R>(&self, read: impl FnOnce(&TypeParameterIndex) -> R) -> R {
        self.index.with(
            (),
            || {
                let mut rows = self
                    .arena
                    .iter()
                    .filter(|(_, parameter)| parameter.symbol.is_valid())
                    .map(|(handle, parameter)| (parameter.symbol, handle))
                    .collect::<Vec<_>>();
                // Stable sorting retains arena declaration order for duplicate
                // identities. Both halves of every generational symbol survive.
                rows.sort_by_key(|(symbol, _)| (symbol.arena_index(), symbol.generation()));
                TypeParameterIndex { rows }
            },
            read,
        )
    }
    pub(super) fn seal(&mut self) {
        self.with_index(|_| ());
        self.index.seal();
    }
    pub(super) fn unseal(&mut self) {
        self.index.unseal();
    }
}

#[cfg(test)]
mod tests;
