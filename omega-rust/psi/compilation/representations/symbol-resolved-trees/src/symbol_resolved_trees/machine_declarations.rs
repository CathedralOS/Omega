//! Machine roots own the relationship from declaration symbols to their rows.
//!
//! Resolution probes repeatedly read the same declaration context. Rewalking its
//! full roster at each call needlessly scales with unrelated declarations. Keep
//! symbol-ordered positions beside that roster, preserving first-match order and
//! full handle generations. Mutable access invalidates the relationship before
//! exposing storage; cloning or replacing this owner carries the matching rows
//! together, including replacements with coincident arena revisions.

use crate::machine::Machine;
use arena::{OrderedRootArena, OwnedIndex};
use std::ops::{Deref, DerefMut};
use symbols::SymbolHandle;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct MachineDeclarations {
    arena: OrderedRootArena<Machine>,
    positions: OwnedIndex<Vec<(SymbolHandle, usize)>, ()>,
}

impl From<OrderedRootArena<Machine>> for MachineDeclarations {
    fn from(arena: OrderedRootArena<Machine>) -> Self {
        Self {
            arena,
            positions: OwnedIndex::default(),
        }
    }
}

impl std::fmt::Debug for MachineDeclarations {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.arena, formatter)
    }
}

impl Deref for MachineDeclarations {
    type Target = OrderedRootArena<Machine>;

    fn deref(&self) -> &Self::Target {
        &self.arena
    }
}

impl DerefMut for MachineDeclarations {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.positions.clear();
        &mut self.arena
    }
}

impl<'arena> IntoIterator for &'arena MachineDeclarations {
    type Item = &'arena Machine;
    type IntoIter = <&'arena OrderedRootArena<Machine> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.arena.iter()
    }
}

impl MachineDeclarations {
    pub fn by_symbol(&self, symbol: SymbolHandle) -> Option<&Machine> {
        let position = self.positions.with(
            (),
            || {
                let mut positions = self
                    .arena
                    .iter()
                    .enumerate()
                    .map(|(position, machine)| (machine.symbol, position))
                    .collect::<Vec<_>>();
                positions.sort_by_key(|(symbol, _)| (symbol.arena_index(), symbol.generation()));
                positions
            },
            |positions| {
                let key = (symbol.arena_index(), symbol.generation());
                let first = positions.partition_point(|(candidate, _)| {
                    (candidate.arena_index(), candidate.generation()) < key
                });
                positions
                    .get(first)
                    .filter(|(candidate, _)| *candidate == symbol)
                    .map(|(_, position)| *position)
            },
        )?;
        Some(&self.arena[position])
    }
}

#[cfg(test)]
mod tests;
