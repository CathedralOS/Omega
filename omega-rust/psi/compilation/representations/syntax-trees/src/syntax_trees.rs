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
pub mod source_files;
#[cfg(test)]
mod tests;
pub mod type_system;
pub mod values;

use crate::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use crate::item::{
    Item, ItemHandle, ItemTable, Machine, MathematicalDefinition, MathematicalDefinitionHandle,
    TraitDefinition,
};
use crate::statement::StatementTable;
use crate::types::TypeReferenceTable;
use source::SourceId;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone)]
pub struct SyntaxTrees {
    pub source_id: SourceId,
    pub roots: SyntaxTreeRoots,
    pub tables: SyntaxTreeTables,
}

#[derive(Debug, Clone)]
pub struct SyntaxTreeTables {
    pub text: crate::SyntaxTextOwner,
    pub items: ItemTable,
    pub expressions: ExpressionTable,
    pub statements: StatementTable,
    pub type_references: TypeReferenceTable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyntaxTreeRoots {
    pub items: Vec<ItemHandle>,
    /// Closed compiler-private extent entries retain the consuming type's
    /// exposure on their exact authored expression root.
    pub constant_extent_probes: Vec<(ExpressionHandle, bool)>,
    /// Parsed top-level `let`/`boundary let` mathematical declarations in
    /// source order. They sit beside `items`, not inside it: the
    /// symbol-resolution lowering that admits declarations is a separately
    /// owned leg, and resolution refuses these explicitly until it lands.
    pub mathematical_definitions: Vec<MathematicalDefinitionHandle>,
}

/// Frontiers for an append-only parser attempt. Suffix handles are private
/// until acceptance; rejection drops their payloads without copying a forest.
pub struct SyntaxAppendCheckpoint {
    items: declarations::item::ItemTableAppendCheckpoint,
    expressions: values::expression::ExpressionTableAppendCheckpoint,
    statements: control_flow::statement::StatementTableAppendCheckpoint,
    types: type_system::types::TypeReferenceTableAppendCheckpoint,
    root_items: usize,
    mathematical_definitions: usize,
    constant_extent_probes: usize,
}

impl SyntaxTrees {
    /// Begin an append-only attempt. The attempt may edit rows it creates,
    /// but must leave earlier rows and page identities unchanged. No rejected
    /// handle may escape; retained handles remain stable across rollback.
    pub fn append_checkpoint(&self) -> SyntaxAppendCheckpoint {
        SyntaxAppendCheckpoint {
            items: self.items.append_checkpoint(),
            expressions: self.expressions.append_checkpoint(),
            statements: self.statements.append_checkpoint(),
            types: self.type_references.append_checkpoint(),
            root_items: self.roots.items.len(),
            mathematical_definitions: self.roots.mathematical_definitions.len(),
            constant_extent_probes: self.roots.constant_extent_probes.len(),
        }
    }

    pub fn rewind_appended(&mut self, checkpoint: SyntaxAppendCheckpoint) {
        self.items.rewind_appended(checkpoint.items);
        self.expressions.rewind_appended(checkpoint.expressions);
        self.statements.rewind_appended(checkpoint.statements);
        self.type_references.rewind_appended(checkpoint.types);
        self.roots.items.truncate(checkpoint.root_items);
        self.roots
            .mathematical_definitions
            .truncate(checkpoint.mathematical_definitions);
        self.roots
            .constant_extent_probes
            .truncate(checkpoint.constant_extent_probes);
    }

    /// Retain immutable checkpoint payloads once; descendants detach edited pages.
    pub fn share_pages(&mut self) {
        self.items.share_pages();
        self.expressions.share_pages();
        self.statements.share_pages();
        self.type_references.share_pages();
    }

    pub fn new(source_id: SourceId) -> Self {
        Self::new_with_generation(source_id, 1)
    }

    /// Create one independently owned syntax page. Every handle minted by its
    /// tables carries `generation`, allowing several pages to coexist without
    /// remapping their local arena indices.
    pub fn new_with_generation(source_id: SourceId, generation: u32) -> Self {
        Self {
            source_id,
            roots: SyntaxTreeRoots::new_with_generation(generation),
            tables: SyntaxTreeTables::new_with_generation(generation),
        }
    }

    pub fn display_expression_name(&self, handle: ExpressionHandle) -> String {
        self.expressions.expression(handle).display_name(self)
    }

    pub fn push_root_item(&mut self, item: Item) -> ItemHandle {
        let handle = self.insert_item(item);
        self.roots.items.push(handle);
        handle
    }

    /// Whether `expression` reaches a name leaf anywhere in its tree: the
    /// const-eval producers ask this before attempting closed-integer folding.
    pub fn const_expression_contains_name(&self, expression: ExpressionHandle) -> bool {
        match self.expressions.expression(expression) {
            ExpressionNode::Name(_) => true,
            ExpressionNode::Binary(binary) => {
                self.const_expression_contains_name(binary.left)
                    || self.const_expression_contains_name(binary.right)
            }
            ExpressionNode::Unary(unary) => self.const_expression_contains_name(unary.operand),
            _ => false,
        }
    }

    /// Record one parsed top-level `let`/`boundary let` declaration. It is a
    /// root in its own collection, not an [`Item`], so the `Item` consumers
    /// unchanged by this leg never see it; resolution refuses it explicitly.
    pub fn push_root_mathematical_definition(
        &mut self,
        definition: MathematicalDefinition,
    ) -> MathematicalDefinitionHandle {
        let handle = self.items.append_mathematical_definition(definition);
        self.roots.mathematical_definitions.push(handle);
        handle
    }

    pub fn root_mathematical_definition_handles(&self) -> &[MathematicalDefinitionHandle] {
        &self.roots.mathematical_definitions
    }

    pub fn root_mathematical_definition(
        &self,
        handle: MathematicalDefinitionHandle,
    ) -> &MathematicalDefinition {
        self.items.mathematical_definition(handle)
    }

    pub fn root_item_handles(&self) -> &[ItemHandle] {
        &self.roots.items
    }

    #[inline]
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
        let frontier = self.type_references.supplied_operand_owner_frontier();
        let mut declarations = Vec::with_capacity(other.root_item_count());
        for handle in other.root_item_handles() {
            let copied = self.push_copied_root_item(other, *handle);
            declarations.push((*handle, copied));
        }
        // Mathematical declarations land after all copied items; source order
        // is retained within each root collection, and nothing this stage
        // produces reads the interleave between the two.
        for handle in other.root_mathematical_definition_handles() {
            let definition = self
                .copy_mathematical_definition(other, other.root_mathematical_definition(*handle));
            self.push_root_mathematical_definition(definition);
        }
        self.type_references
            .remap_supplied_operand_owners(frontier, &declarations);
    }

    /// Adopt one independently produced syntax page without copying nodes or
    /// remapping handles. Root handles remain in authored page order.
    pub fn adopt_page(&mut self, other: SyntaxTrees) {
        self.roots.items.extend(other.roots.items);
        self.roots
            .mathematical_definitions
            .extend(other.roots.mathematical_definitions);
        self.tables.adopt(other.tables);
    }

    /// Start a fresh page for syntax synthesized after page adoption.
    pub fn begin_page(&mut self, generation: u32) {
        self.tables.begin_page(generation);
    }

    pub fn page_count(&self) -> usize {
        self.tables.page_count()
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
        let item = self.copy_item_from(other, other.root_item(handle));
        self.push_root_item(item)
    }
}

impl SyntaxTreeTables {
    pub fn new() -> Self {
        Self::new_with_generation(1)
    }

    pub fn new_with_generation(generation: u32) -> Self {
        Self {
            text: crate::SyntaxTextOwner::default(),
            items: ItemTable::new_with_generation(generation),
            expressions: ExpressionTable::new_with_generation(generation),
            statements: StatementTable::new_with_generation(generation),
            type_references: TypeReferenceTable::new_with_generation(generation),
        }
    }

    fn adopt(&mut self, other: Self) {
        self.text.adopt(other.text);
        self.items.adopt(other.items);
        self.expressions.adopt(other.expressions);
        self.statements.adopt(other.statements);
        self.type_references.adopt(other.type_references);
    }

    fn begin_page(&mut self, generation: u32) {
        self.items.begin_page(generation);
        self.expressions.begin_page(generation);
        self.statements.begin_page(generation);
        self.type_references.begin_page(generation);
    }

    fn page_count(&self) -> usize {
        self.items.page_count()
    }
}

impl SyntaxTreeRoots {
    fn new_with_generation(_generation: u32) -> Self {
        Self {
            items: Vec::new(),
            constant_extent_probes: Vec::new(),
            mathematical_definitions: Vec::new(),
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

impl crate::SyntaxTextContext for SyntaxTrees {
    fn source_text(&self, source: arena::Handle<crate::RetainedSource>) -> &str {
        self.text.source_text(source)
    }
}
