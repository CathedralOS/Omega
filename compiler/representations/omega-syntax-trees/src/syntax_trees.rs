use crate::expression::{Expression, ExpressionTable};
use crate::item::{CapabilityMember, DataMember, Item, ItemHandle, ItemTable, Machine, Platform};
use crate::statement::StatementTable;
use crate::types::{TypeConstraint, TypeReference, TypeReferenceTable};
use omega_core::arena::{Arena, HandleSpan};
use omega_core::source::SourceId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxTrees {
    pub source_id: SourceId,
    pub root_items: HandleSpan<ItemHandle>,
    pub root_item_handles: Arena<ItemHandle>,
    pub root_item_storage: Arena<Item>,
    pub items: ItemTable,
    pub expressions: ExpressionTable,
    pub statements: StatementTable,
    pub type_references: TypeReferenceTable,
}

impl SyntaxTrees {
    pub fn new(source_id: SourceId) -> Self {
        Self {
            source_id,
            root_items: HandleSpan::empty(),
            root_item_handles: Arena::new(),
            root_item_storage: Arena::new(),
            items: ItemTable::new(),
            expressions: ExpressionTable::new(),
            statements: StatementTable::new(),
            type_references: TypeReferenceTable::new(),
        }
    }

    pub fn from_root_items(
        source_id: SourceId,
        items: impl IntoIterator<Item = Item>,
    ) -> Self {
        let mut syntax_trees = Self::new(source_id);

        for item in items {
            syntax_trees.push_root_item(item);
        }

        syntax_trees
    }

    pub fn push_root_item(&mut self, item: Item) -> ItemHandle {
        self.insert_item(&item);
        let handle = self.root_item_storage.append(item);
        let root_handle = self.root_item_handles.append(handle);

        self.root_items = if self.root_items.is_empty() {
            HandleSpan::from_parts(root_handle, 1)
        } else {
            HandleSpan::from_parts(
                self.root_items.start(),
                self.root_items
                    .count()
                    .checked_add(1)
                    .expect("root item span count overflow"),
            )
        };

        handle
    }

    pub fn root_item_handles(&self) -> &[ItemHandle] {
        self.root_item_handles.span_or_empty(self.root_items)
    }

    pub fn root_item(&self, handle: ItemHandle) -> &Item {
        self.root_item_storage.get(handle)
    }

    pub fn root_items(&self) -> impl Iterator<Item = &Item> {
        self.root_item_handles()
            .iter()
            .map(|handle| self.root_item(*handle))
    }

    pub fn root_item_count(&self) -> usize {
        self.root_item_handles().len()
    }

    fn insert_item(&mut self, item: &Item) {
        match item {
            Item::Capability(capability) => {
                for member in &capability.members {
                    match member {
                        CapabilityMember::Field(field) => {
                            self.insert_type_reference(&field.type_reference);
                        }
                        CapabilityMember::State(state) => {
                            self.items.insert_state_signature_tree(
                                &state.signature,
                                &mut self.type_references,
                                &mut self.expressions,
                            );
                        }
                    }
                }
            }
            Item::Data(data_definition) => {
                for member in &data_definition.members {
                    if let DataMember::Field(field) = member {
                        self.insert_type_reference(&field.type_reference);

                        if let Some(initial_value) = &field.initial_value {
                            self.insert_expression(initial_value);
                        }
                    }
                }
            }
            Item::Invariant(invariant) => {
                for constraint in &invariant.constraints {
                    self.insert_type_constraint_expressions(constraint);
                }
            }
            Item::Library(library) => {
                for function in &library.functions {
                    self.items.insert_state_signature_tree(
                        &function.signature,
                        &mut self.type_references,
                        &mut self.expressions,
                    );
                }
            }
            Item::Machine(machine) => self.insert_machine(machine),
            Item::Platform(platform) => self.insert_platform(platform),
            Item::Target(_) | Item::TrustDefinition(_) | Item::Use(_) => {}
        }
    }

    fn insert_machine(&mut self, machine: &Machine) {
        self.items.insert_machine_tree(
            machine,
            &mut self.statements,
            &mut self.type_references,
            &mut self.expressions,
        );
    }

    fn insert_platform(&mut self, platform: &Platform) {
        self.items.insert_platform_tree(
            platform,
            &mut self.type_references,
            &mut self.expressions,
        );
    }

    fn insert_type_reference(&mut self, type_reference: &TypeReference) {
        self.type_references
            .insert_tree(type_reference, &mut self.expressions);
    }

    fn insert_type_constraint_expressions(&mut self, constraint: &TypeConstraint) {
        match constraint {
            TypeConstraint::Named(_) => {}
            TypeConstraint::Range { minimum, maximum } => {
                self.insert_expression(minimum);
                self.insert_expression(maximum);
            }
        }
    }

    fn insert_expression(&mut self, expression: &Expression) {
        self.expressions.insert_tree(expression);
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
    use crate::expression::Expression;
    use crate::identifier::Identifier;
    use crate::item::{Item, Machine, State};
    use crate::statement::{Statement, Transition, TransitionGuard, TransitionTarget};
    use crate::types::TypeReference;

    #[test]
    fn syntax_trees_collect_state_expression_and_type_payloads() {
        let syntax_trees = SyntaxTrees::from_root_items(
            Default::default(),
            vec![Item::Machine(Machine {
                name: Identifier::generated("Main"),
                states: vec![State {
                    name: Identifier::generated("entry"),
                    parameters: Vec::new(),
                    return_type: Some(TypeReference::named("i32")),
                    statements: vec![Statement::Transition(Transition {
                        target: TransitionTarget::Terminal,
                        continuation: None,
                        guard: TransitionGuard::When(Expression::Integer(1)),
                    })],
                }],
            })],
        );

        assert_eq!(syntax_trees.root_item_count(), 1);
        assert_eq!(syntax_trees.type_references.type_reference_count(), 1);
        assert_eq!(syntax_trees.expressions.expression_count(), 1);
        assert_eq!(syntax_trees.statements.statement_count(), 1);
        assert_eq!(syntax_trees.items.machine_count(), 1);
        assert_eq!(syntax_trees.items.state_count(), 1);
    }
}
