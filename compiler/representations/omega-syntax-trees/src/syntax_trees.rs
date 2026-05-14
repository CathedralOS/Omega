use crate::expression::ExpressionTable;
use crate::item::{Item, ItemHandle, ItemTable, Machine, Platform};
use crate::statement::StatementTable;
use crate::types::TypeReferenceTable;
use omega_core::arena::Arena;
use omega_core::source::SourceId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxTrees {
    pub source_id: SourceId,
    pub root_item_handles: Arena<ItemHandle>,
    pub items: ItemTable,
    pub expressions: ExpressionTable,
    pub statements: StatementTable,
    pub type_references: TypeReferenceTable,
}

impl SyntaxTrees {
    pub fn new(source_id: SourceId) -> Self {
        Self {
            source_id,
            root_item_handles: Arena::new(),
            items: ItemTable::new(),
            expressions: ExpressionTable::new(),
            statements: StatementTable::new(),
            type_references: TypeReferenceTable::new(),
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
        self.root_item_handles.append(handle);
        handle
    }

    pub fn root_item_handles(&self) -> &[ItemHandle] {
        self.root_item_handles.storage_slice()
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
        self.root_item_handles.len()
    }

    fn insert_item(&mut self, item: Item) -> ItemHandle {
        match &item {
            Item::Machine(machine) => self.insert_machine(machine),
            Item::Platform(platform) => self.insert_platform(platform),
            Item::Capability(_)
            | Item::Data(_)
            | Item::Invariant(_)
            | Item::Library(_)
            | Item::Target(_)
            | Item::TrustDefinition(_)
            | Item::Use(_) => {}
        }

        self.items.append_item(item)
    }

    fn insert_machine(&mut self, machine: &Machine) {
        self.items.insert_machine(machine);
    }

    fn insert_platform(&mut self, platform: &Platform) {
        self.items.insert_platform(platform);
    }
}

impl Default for SyntaxTrees {
    fn default() -> Self {
        Self::new(SourceId::default())
    }
}

#[cfg(test)]
mod tests {
    use super::SyntaxTrees;
    use crate::identifier::Identifier;
    use crate::item::{Item, Machine, State};
    use crate::statement::{
        StatementNode, TableTransition, TransitionGuardNode, TransitionTargetNode,
    };
    use crate::types::TypeReferenceNode;
    use omega_core::arena::HandleSpan;

    #[test]
    fn syntax_trees_collect_state_expression_and_type_payloads() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());
        let guard = syntax_trees
            .expressions
            .insert(crate::expression::ExpressionNode::Integer(1));
        let target = syntax_trees
            .statements
            .insert_transition_target(TransitionTargetNode::Terminal);
        let statement =
            syntax_trees
                .statements
                .insert(StatementNode::Transition(TableTransition {
                    target,
                    continuation: crate::statement::TransitionTargetHandle::invalid(),
                    guard: TransitionGuardNode::When(guard),
                }));
        let statement_handle = syntax_trees.items.append_statement_handle(statement);
        let statements = HandleSpan::from_parts(statement_handle, 1);
        let return_type = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("i32")));
        let state = syntax_trees.items.insert_state(&State {
            name: Identifier::generated("entry"),
            parameters: HandleSpan::empty(),
            return_type,
            statements,
        });
        let state_handle = syntax_trees.items.append_state_handle(state);

        syntax_trees.push_root_item(Item::Machine(Machine {
            name: Identifier::generated("Main"),
            states: HandleSpan::from_parts(state_handle, 1),
        }));

        assert_eq!(syntax_trees.root_item_count(), 1);
        assert_eq!(syntax_trees.type_references.type_reference_count(), 1);
        assert_eq!(syntax_trees.expressions.expression_count(), 1);
        assert_eq!(syntax_trees.statements.statement_count(), 1);
        assert_eq!(syntax_trees.items.machine_count(), 1);
        assert_eq!(syntax_trees.items.state_count(), 1);
    }
}
