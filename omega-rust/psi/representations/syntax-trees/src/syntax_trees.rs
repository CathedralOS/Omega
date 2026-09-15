//! The current syntax trees program and its concept-owned storage.
//!
//! Declarations, executable control flow, values, type references and retained
//! inspection data are subordinate to this root, not separate pipeline outputs.
//!
//! This file owns the syntax trees carrier, its roots and tables.
//! `item_copies.rs` and `body_copies.rs` copy items and bodies between
//! programs.

mod body_copies;
pub mod control_flow;
pub mod declarations;
pub mod inspection;
mod item_copies;
pub mod names;
#[cfg(test)]
mod tests;
pub mod type_system;
pub mod values;

use crate::expression::ExpressionTable;
use crate::item::{Item, ItemHandle, ItemTable, Machine, TraitDefinition};
use crate::statement::StatementTable;
use crate::types::TypeReferenceTable;
use arena::Arena;
use source::SourceId;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxTrees {
    pub source_id: SourceId,
    pub roots: SyntaxTreeRoots,
    pub tables: SyntaxTreeTables,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxTreeTables {
    pub items: ItemTable,
    pub expressions: ExpressionTable,
    pub statements: StatementTable,
    pub type_references: TypeReferenceTable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyntaxTreeRoots {
    pub items: Arena<ItemHandle>,
}

impl SyntaxTrees {
    pub fn new(source_id: SourceId) -> Self {
        Self {
            source_id,
            roots: SyntaxTreeRoots::default(),
            tables: SyntaxTreeTables::new(),
        }
    }

    pub fn from_root_items(source_id: SourceId, items: impl IntoIterator<Item = Item>) -> Self {
        let mut syntax_trees = Self::new(source_id);

        for item in items {
            syntax_trees.push_root_item(item);
        }

        syntax_trees
    }

    pub fn push_root_item(&mut self, item: Item) -> ItemHandle {
        let handle = self.insert_item(item);
        self.roots.items.append(handle);
        handle
    }

    pub fn root_item_handles(&self) -> &[ItemHandle] {
        self.roots.items.storage_slice()
    }

    pub fn root_item(&self, handle: ItemHandle) -> &Item {
        self.items.item(handle)
    }

    pub fn root_items(&self) -> impl Iterator<Item = &Item> {
        self.root_item_handles()
            .iter()
            .map(|handle| self.root_item(*handle))
    }

    pub fn root_item_count(&self) -> usize {
        self.roots.items.len()
    }

    /// Whether a folded declaration owns this original selection occurrence.
    /// Copying its value into an index must not promote implementation custody
    /// to the consuming signature's public exposure.
    pub fn constant_initializer_owns_selection(&self, reference: source::SourceSpan) -> bool {
        self.root_items().any(|item| {
            let Item::Const(definition) = item else {
                return false;
            };
            definition
                .normalization
                .as_ref()
                .is_some_and(|normalization| {
                    normalization
                        .selections
                        .iter()
                        .any(|origin| origin.reference == reference)
                        || normalization.builtin_operators.contains(&reference)
                        || normalization
                            .call_selections
                            .iter()
                            .any(|selection| selection.0 == reference)
                })
        })
    }

    pub fn extend_from(&mut self, other: &SyntaxTrees) {
        for handle in other.root_item_handles() {
            self.push_copied_root_item(other, *handle);
        }
    }

    fn insert_item(&mut self, item: Item) -> ItemHandle {
        match &item {
            Item::Machine(machine) => self.insert_machine(machine),
            Item::Trait(trait_definition) => self.insert_trait_definition(trait_definition),
            Item::Capability(_)
            | Item::Conformance(_)
            | Item::Const(_)
            | Item::Data(_)
            | Item::Domain(_)
            | Item::Measure(_)
            | Item::Module(_)
            | Item::Operator(_)
            | Item::Package(_)
            | Item::Proposition(_)
            | Item::Use(_) => {}
        }

        self.items.append_item(item)
    }

    fn insert_machine(&mut self, machine: &Machine) {
        self.items.insert_machine(machine);
    }

    fn insert_trait_definition(&mut self, trait_definition: &TraitDefinition) {
        self.items.insert_trait_definition(trait_definition);
    }

    fn push_copied_root_item(&mut self, other: &SyntaxTrees, handle: ItemHandle) -> ItemHandle {
        let item = self.copy_item(other, other.root_item(handle));
        self.push_root_item(item)
    }
}

impl SyntaxTreeTables {
    pub fn new() -> Self {
        Self {
            items: ItemTable::new(),
            expressions: ExpressionTable::new(),
            statements: StatementTable::new(),
            type_references: TypeReferenceTable::new(),
        }
    }
}

impl Default for SyntaxTreeTables {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for SyntaxTrees {
    type Target = SyntaxTreeTables;

    fn deref(&self) -> &Self::Target {
        &self.tables
    }
}

impl DerefMut for SyntaxTrees {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tables
    }
}

impl Default for SyntaxTrees {
    fn default() -> Self {
        Self::new(SourceId::default())
    }
}
