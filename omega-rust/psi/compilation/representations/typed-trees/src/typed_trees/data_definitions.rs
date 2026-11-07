//! Data declarations and their complete visible-roster index share one owner.
//!
//! A mutable arena borrow clears the derived rows before exposing storage,
//! including replacement by an unrelated arena with the same revision. A whole
//! table replacement or clone carries storage and its matching index together.
//! The full visible span remains the query key: changing only a root generation
//! or count must not reuse another roster's declarations. Other declaration
//! indexes keep their own freshness rules in `declaration_index`.

use super::declaration_index::SymbolRows;
use crate::data::DataDefinition;
use arena::{Arena, Handle, HandleSpan, OwnedIndex};
use symbols::SymbolHandle;

type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;

#[cfg(test)]
mod tests;

/// Every declaration position per symbol and per name, in declaration order.
/// Duplicate symbols remain observable to readers that require a unique owner.
#[derive(Clone, Default)]
pub struct DataDefinitionIndex {
    by_symbol: SymbolRows<u32>,
    by_name: HashMap<Box<str>, Vec<u32>>,
}

impl DataDefinitionIndex {
    /// Every position declaring `symbol`, in declaration order — absent,
    /// unique, and duplicate answers all read the same slice.
    pub fn positions_for_symbol(&self, symbol: SymbolHandle) -> &[u32] {
        self.by_symbol.get(symbol)
    }

    pub(super) fn first_position(&self, symbol: SymbolHandle) -> Option<usize> {
        self.by_symbol
            .first(symbol)
            .map(|position| *position as usize)
    }

    pub(super) fn positions_for_name(&self, name: &str) -> &[u32] {
        self.by_name.get(name).map(Vec::as_slice).unwrap_or(&[])
    }

    fn build(definitions: &[DataDefinition]) -> Self {
        let mut by_symbol = Vec::with_capacity(definitions.len());
        let mut by_name: HashMap<Box<str>, Vec<u32>> = HashMap::default();
        for (position, definition) in definitions.iter().enumerate() {
            let position = position as u32;
            if definition.symbol.is_valid() {
                by_symbol.push((definition.symbol, position));
            }
            by_name
                .entry(definition.name.as_str().into())
                .or_default()
                .push(position);
        }
        Self {
            by_symbol: SymbolRows::new(by_symbol),
            by_name,
        }
    }
}

/// Data-declaration storage and its derived symbol/name relationships.
/// Producers retain arena access, but a mutable borrow invalidates both the
/// lazy and sealed index before any storage mutation can occur.
#[derive(Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct DataDefinitionTable {
    arena: Arena<DataDefinition>,
    index: OwnedIndex<DataDefinitionIndex, (HandleSpan<DataDefinition>, usize)>,
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
    /// A fresh append leaves every existing declaration row unchanged. Derived
    /// application views use this same arena without joining the visible roster.
    /// Reusing a freed slot or exposing mutable storage still clears the index.
    pub fn insert(&mut self, definition: DataDefinition) -> Handle<DataDefinition> {
        let before = self.arena.append_epoch();
        let handle = self.arena.insert(definition);
        let appended = before.zip(self.arena.append_epoch()).is_some_and(
            |((before_epoch, before_len), (after_epoch, after_len))| {
                before_epoch == after_epoch && before_len.checked_add(1) == Some(after_len)
            },
        );
        if !appended {
            self.index.clear();
        }
        handle
    }

    pub(super) fn with_index<Result>(
        &self,
        roster: HandleSpan<DataDefinition>,
        read: impl FnOnce(&DataDefinitionIndex) -> Result,
    ) -> Result {
        let definitions = self.arena.span_or_empty(roster);
        // A previously out-of-range roster can become valid after an append.
        // Preserve its complete handle identity and distinguish that resolution
        // from the empty answer retained before its rows existed.
        self.index.with(
            (roster, definitions.len()),
            || DataDefinitionIndex::build(definitions),
            read,
        )
    }

    pub(super) fn seal(&mut self) {
        self.index.seal_with(|index| index.by_symbol.densify());
    }

    pub(super) fn unseal(&mut self) {
        self.index.unseal();
    }
}
