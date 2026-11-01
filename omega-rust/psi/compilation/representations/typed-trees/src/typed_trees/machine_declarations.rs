//! Machine storage owns both visible-root and retained lexical declarations.
//!
//! A mutable arena escape clears the relationships before mutation. Replacing
//! or cloning the table carries only the indexes belonging to that storage;
//! equal revision numbers in unrelated arenas cannot reuse declaration answers.

use crate::machine::Machine;
use arena::{Arena, Handle, HandleSpan, OwnedIndex};
use symbols::SymbolHandle;

#[derive(Clone, Default)]
pub(super) struct MachinePositionIndex {
    pub(super) rows: Vec<(SymbolHandle, u32)>,
}

#[derive(Clone, Default)]
struct RetainedDeclarationIndex {
    rows: Vec<(SymbolHandle, Option<Handle<Machine>>)>,
}

fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

impl RetainedDeclarationIndex {
    fn build(arena: &Arena<Machine>) -> Self {
        let mut declarations = arena
            .iter()
            .map(|(handle, machine)| (machine.symbol, handle))
            .collect::<Vec<_>>();
        declarations.sort_by_key(|(symbol, _)| symbol_key(*symbol));
        let mut rows: Vec<(SymbolHandle, Option<Handle<Machine>>)> = Vec::new();
        for (symbol, handle) in declarations {
            if let Some((previous, selected)) = rows.last_mut()
                && *previous == symbol
            {
                if selected.is_some_and(|first| arena.get(first) != arena.get(handle)) {
                    *selected = None;
                }
            } else {
                rows.push((symbol, Some(handle)));
            }
        }
        Self { rows }
    }

    fn declaration(&self, symbol: SymbolHandle) -> Option<Handle<Machine>> {
        let position = self
            .rows
            .binary_search_by_key(&symbol_key(symbol), |(symbol, _)| symbol_key(*symbol))
            .ok()?;
        self.rows[position].1
    }
}

#[derive(Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct MachineDeclarationTable {
    arena: Arena<Machine>,
    roots: OwnedIndex<MachinePositionIndex, HandleSpan<Machine>>,
    retained: OwnedIndex<RetainedDeclarationIndex, ()>,
}

impl From<Arena<Machine>> for MachineDeclarationTable {
    fn from(arena: Arena<Machine>) -> Self {
        Self {
            arena,
            roots: OwnedIndex::default(),
            retained: OwnedIndex::default(),
        }
    }
}

impl std::fmt::Debug for MachineDeclarationTable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.arena, formatter)
    }
}

impl std::ops::Deref for MachineDeclarationTable {
    type Target = Arena<Machine>;

    fn deref(&self) -> &Self::Target {
        &self.arena
    }
}

impl std::ops::DerefMut for MachineDeclarationTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.roots.clear();
        self.retained.clear();
        &mut self.arena
    }
}

impl MachineDeclarationTable {
    pub(super) fn with_root_index<Result>(
        &self,
        roster: HandleSpan<Machine>,
        read: impl FnOnce(&MachinePositionIndex) -> Result,
    ) -> Result {
        self.roots.with(
            roster,
            || {
                let mut rows = self
                    .arena
                    .span_or_empty(roster)
                    .iter()
                    .enumerate()
                    .map(|(position, machine)| (machine.symbol, position as u32))
                    .collect::<Vec<_>>();
                rows.sort_by_key(|(symbol, _)| symbol_key(*symbol));
                MachinePositionIndex { rows }
            },
            read,
        )
    }

    pub(super) fn retained_declaration(&self, symbol: SymbolHandle) -> Option<&Machine> {
        let handle = self.retained.with(
            (),
            || RetainedDeclarationIndex::build(&self.arena),
            |index| index.declaration(symbol),
        )?;
        Some(self.arena.get(handle))
    }

    pub(super) fn seal(&mut self) {
        self.retained
            .with((), || RetainedDeclarationIndex::build(&self.arena), |_| ());
        self.roots.seal();
        self.retained.seal();
    }

    pub(super) fn unseal(&mut self) {
        self.roots.unseal();
        self.retained.unseal();
    }
}
