//! Data declarations and their complete visible-roster index share one owner.
//!
//! A mutable arena borrow clears the derived rows before exposing storage,
//! including replacement by an unrelated arena with the same revision. A whole
//! table replacement or clone carries storage and its matching index together.
//! The full visible span remains the query key: changing only a root generation
//! or count must not reuse another roster's declarations. Other declaration
//! indexes keep their own freshness rules in `declaration_index`.

use crate::data::DataDefinition;
use arena::{Arena, HandleSpan, OwnedIndex};
use symbols::SymbolHandle;

type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;

#[cfg(test)]
mod tests;

/// Every declaration position per symbol and name, plus the first symbol row.
/// Duplicate symbols remain observable to readers that require a unique owner.
#[derive(Clone, Default)]
pub struct DataDefinitionIndex {
    by_symbol: HashMap<SymbolHandle, Vec<u32>>,
    by_name: HashMap<Box<str>, Vec<u32>>,
    first_position: HashMap<SymbolHandle, u32>,
}

impl DataDefinitionIndex {
    /// Every position declaring `symbol`, in declaration order — absent,
    /// unique, and duplicate answers all read the same slice.
    pub fn positions_for_symbol(&self, symbol: SymbolHandle) -> &[u32] {
        self.by_symbol
            .get(&symbol)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(super) fn first_position(&self, symbol: SymbolHandle) -> Option<usize> {
        self.first_position
            .get(&symbol)
            .map(|position| *position as usize)
    }

    pub(super) fn positions_for_name(&self, name: &str) -> &[u32] {
        self.by_name.get(name).map(Vec::as_slice).unwrap_or(&[])
    }

    fn build(definitions: &[DataDefinition]) -> Self {
        let mut index = Self::default();
        for (position, definition) in definitions.iter().enumerate() {
            let position = position as u32;
            if definition.symbol.is_valid() {
                index
                    .by_symbol
                    .entry(definition.symbol)
                    .or_default()
                    .push(position);
                index
                    .first_position
                    .entry(definition.symbol)
                    .or_insert(position);
            }
            index
                .by_name
                .entry(definition.name.as_str().into())
                .or_default()
                .push(position);
        }
        index
    }
}

/// Data-declaration storage and its derived symbol/name relationships.
/// Producers retain arena access, but a mutable borrow invalidates both the
/// lazy and sealed index before any storage mutation can occur.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct DataDefinitionTable {
    arena: Arena<DataDefinition>,
    index: OwnedIndex<DataDefinitionIndex, HandleSpan<DataDefinition>>,
}

impl From<Arena<DataDefinition>> for DataDefinitionTable {
    fn from(arena: Arena<DataDefinition>) -> Self {
        Self {
            arena,
            index: OwnedIndex::default(),
        }
    }
}

impl std::fmt::Debug for DataDefinitionTable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.arena, formatter)
    }
}

impl std::ops::Deref for DataDefinitionTable {
    type Target = Arena<DataDefinition>;

    fn deref(&self) -> &Self::Target {
        &self.arena
    }
}

impl std::ops::DerefMut for DataDefinitionTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.index.clear();
        &mut self.arena
    }
}

impl DataDefinitionTable {
    pub(super) fn with_index<Result>(
        &self,
        roster: HandleSpan<DataDefinition>,
        read: impl FnOnce(&DataDefinitionIndex) -> Result,
    ) -> Result {
        self.index.with(
            roster,
            || DataDefinitionIndex::build(self.arena.span_or_empty(roster)),
            read,
        )
    }

    pub(super) fn seal(&mut self) {
        self.index.seal();
    }

    pub(super) fn unseal(&mut self) {
        self.index.unseal();
    }
}
