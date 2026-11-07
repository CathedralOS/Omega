//! Parameter storage owns its complete symbol index, including negative answers.
//! Mutable arena access invalidates it before exposing storage. Cloning or
//! replacing the table carries only the index belonging to those declarations.
use super::declaration_index::SymbolRows;
use crate::data::TypeParameter;
use arena::{Arena, Handle, OwnedIndex};

/// Every parameter handle per symbol, in arena declaration order.
pub(super) type TypeParameterIndex = SymbolRows<Handle<TypeParameter>>;

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
                // Rows keep arena declaration order for duplicate identities.
                SymbolRows::new(
                    self.arena
                        .iter()
                        .filter(|(_, parameter)| parameter.symbol.is_valid())
                        .map(|(handle, parameter)| (parameter.symbol, handle))
                        .collect(),
                )
            },
            read,
        )
    }
    pub(super) fn seal(&mut self) {
        self.with_index(|_| ());
        self.index.seal_with(SymbolRows::densify);
    }
    pub(super) fn unseal(&mut self) {
        self.index.unseal();
    }
}

#[cfg(test)]
mod tests;
