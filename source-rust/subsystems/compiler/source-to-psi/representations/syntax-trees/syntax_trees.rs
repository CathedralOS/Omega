//! Unresolved syntax trees retain authored structure, source text and arena identities.
//! `SyntaxTrees` owns roots and tables; follow their row types into the grammar domains.

#[path = "statements/root_bindings.rs"]
pub mod root_bindings;
pub use equality::SyntaxEquivalent;
pub use float_spelling::FloatSpelling;
pub use item_copies::SuppliedSyntaxArgument;
pub use language_core::operator_spelling;
pub use relocation::{PageRelocation, SyntaxRelocate};
pub use source_files::{ParsedRootItems, ParsedSource, ParsedSources, ParsedUnit};
pub use text_owner::{RetainedSource, SyntaxTextContext, SyntaxTextOwner};

use crate::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use crate::item::{
    Item, ItemHandle, ItemTable, MathematicalDefinition, MathematicalDefinitionHandle,
};
use crate::statement::StatementTable;
use crate::types::TypeReferenceTable;
use source_text::SourceId;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, record_storage::Record)]
pub struct SyntaxTrees {
    pub source_id: SourceId,
    pub roots: SyntaxTreeRoots,
    pub tables: SyntaxTreeTables,
}

#[derive(Debug, Clone, record_storage::Record)]
pub struct SyntaxTreeTables {
    pub text: crate::SyntaxTextOwner,
    pub items: ItemTable,
    pub expressions: ExpressionTable,
    pub statements: StatementTable,
    pub type_references: TypeReferenceTable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct SyntaxTreeRoots {
    pub items: Vec<ItemHandle>,
    pub constant_extent_probes: Vec<(ExpressionHandle, bool)>,
    pub mathematical_definitions: Vec<MathematicalDefinitionHandle>,
}

pub struct SyntaxAppendCheckpoint {
    items: item::ItemTableAppendCheckpoint,
    expressions: expression::ExpressionTableAppendCheckpoint,
    statements: statement::StatementTableAppendCheckpoint,
    types: types::TypeReferenceTableAppendCheckpoint,
    root_items: usize,
    mathematical_definitions: usize,
    constant_extent_probes: usize,
}

impl SyntaxTrees {
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

    pub fn share_pages(&mut self) {
        self.items.share_pages();
        self.expressions.share_pages();
        self.statements.share_pages();
        self.type_references.share_pages();
    }

    pub fn new(source_id: SourceId) -> Self {
        Self::new_with_generation(source_id, 1)
    }

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
        let handle = self.items.append_item(item);
        self.roots.items.push(handle);
        handle
    }

    pub fn const_expression_contains_name(&self, expression: ExpressionHandle) -> bool {
        match self.expressions.expression(expression) {
            ExpressionNode::Name { .. } => true,
            ExpressionNode::Binary(binary) => {
                self.const_expression_contains_name(binary.left)
                    || self.const_expression_contains_name(binary.right)
            }
            ExpressionNode::Unary(unary) => self.const_expression_contains_name(unary.operand),
            _ => false,
        }
    }

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

    pub fn extend_from(&mut self, other: &SyntaxTrees) {
        let frontier = self.type_references.supplied_operand_owner_frontier();
        let mut declarations = Vec::with_capacity(other.root_item_count());
        for handle in other.root_item_handles() {
            let copied = self.push_copied_root_item(other, *handle);
            declarations.push((*handle, copied));
        }
        for handle in other.root_mathematical_definition_handles() {
            let definition = self
                .copy_mathematical_definition(other, other.root_mathematical_definition(*handle));
            self.push_root_mathematical_definition(definition);
        }
        for &(expression, public) in &other.roots.constant_extent_probes {
            let copied = self.copy_expression_from(other, expression);
            self.roots.constant_extent_probes.push((copied, public));
        }
        self.type_references
            .remap_supplied_operand_owners(frontier, &declarations);
    }

    pub fn adopt_page(&mut self, other: SyntaxTrees) {
        self.roots.items.extend(other.roots.items);
        self.roots
            .constant_extent_probes
            .extend(other.roots.constant_extent_probes);
        self.roots
            .mathematical_definitions
            .extend(other.roots.mathematical_definitions);
        self.tables.adopt(other.tables);
    }

    pub fn begin_page(&mut self, generation: u32) {
        self.tables.begin_page(generation);
    }

    /// Moves a standalone single-page forest, parsed under private source
    /// coordinates, to the admitted `source_id` and to `generation`, a page
    /// the receiving forest has not used. A forest with more than one page,
    /// or whose rows were rewritten after parsing, is refused.
    fn relocate_unit(
        &mut self,
        source_id: SourceId,
        generation: u32,
    ) -> Result<PageRelocation, &'static str> {
        if self.page_count() != 1 {
            return Err("only a standalone unit page can be relocated");
        }
        let relocation = PageRelocation {
            from_generation: self.items.first_generation(),
            generation,
            from_source: self.source_id,
            source: source_id,
        };
        self.tables.items.relocate_page(&relocation)?;
        self.tables.expressions.relocate_page(&relocation)?;
        self.tables.statements.relocate_page(&relocation)?;
        self.tables.type_references.relocate_page(&relocation)?;
        self.roots.items.relocate(&relocation);
        self.roots.constant_extent_probes.relocate(&relocation);
        self.roots.mathematical_definitions.relocate(&relocation);
        self.source_id = source_id;
        Ok(relocation)
    }

    pub fn page_count(&self) -> usize {
        self.tables.page_count()
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

#[path = "declarations/items.rs"]
pub mod item;

#[path = "statements/assembly.rs"]
pub mod assembly;

#[path = "statements/control_flow.rs"]
pub mod statement;

#[path = "text/identifiers.rs"]
pub mod identifier;

#[path = "text/owner.rs"]
pub mod text_owner;

#[path = "types/references.rs"]
pub mod types;

#[path = "expressions/values.rs"]
pub mod expression;

#[path = "expressions/float_spelling.rs"]
pub mod float_spelling;

#[path = "copying/bodies.rs"]
pub(crate) mod body_copies;

#[path = "copying/items.rs"]
pub(crate) mod item_copies;

#[path = "storage/equality.rs"]
pub(crate) mod equality;

#[path = "storage/relocation.rs"]
pub(crate) mod relocation;

#[path = "storage/paged_tables.rs"]
pub(crate) mod paged_roster;

#[path = "files/parsed.rs"]
pub mod source_files;

#[cfg(test)]
#[path = "tests/unit/forest.rs"]
pub(crate) mod tests;

#[path = "copying/statement_paths.rs"]
mod statement_paths;
