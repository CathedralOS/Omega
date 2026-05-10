use crate::expression::{Expression, ExpressionTable};
use crate::item::{CapabilityMember, DataMember, Item, ItemTable, Machine, Platform};
use crate::statement::StatementTable;
use crate::types::{TypeConstraint, TypeReference, TypeReferenceTable};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AstTables {
    pub items: ItemTable,
    pub expressions: ExpressionTable,
    pub statements: StatementTable,
    pub type_references: TypeReferenceTable,
}

impl AstTables {
    pub fn from_items(items: &[Item]) -> Self {
        let mut tables = Self::default();

        for item in items {
            tables.insert_item(item);
        }

        tables
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

#[cfg(test)]
mod tests {
    use super::AstTables;
    use crate::expression::Expression;
    use crate::identifier::Identifier;
    use crate::item::{Item, Machine, State};
    use crate::statement::{Statement, Transition, TransitionGuard, TransitionTarget};
    use crate::types::TypeReference;

    #[test]
    fn ast_tables_collect_state_expression_and_type_payloads() {
        let items = vec![Item::Machine(Machine {
            name: Identifier::generated("Main"),
            contains: Vec::new(),
            owned_data: Vec::new(),
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
        })];

        let tables = AstTables::from_items(&items);

        assert_eq!(tables.type_references.type_reference_count(), 1);
        assert_eq!(tables.expressions.expression_count(), 1);
        assert_eq!(tables.statements.statement_count(), 1);
        assert_eq!(tables.items.machine_count(), 1);
        assert_eq!(tables.items.state_count(), 1);
    }
}
