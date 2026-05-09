use crate::expression::{Expression, ExpressionTable};
use crate::item::{
    CapabilityMember, DataMember, Item, Machine, OwnedData, Platform, State, StateSignature,
};
use crate::statement::{Statement, TransitionGuard, TransitionTarget};
use crate::types::{TypeConstraint, TypeReference, TypeReferenceTable};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AstTables {
    pub expressions: ExpressionTable,
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
                            self.insert_state_signature(&state.signature);
                        }
                    }
                }
            }
            Item::Data(data_definition) => {
                for member in &data_definition.members {
                    if let DataMember::Field(field) = member {
                        self.insert_type_reference(&field.type_reference);
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
                    self.insert_state_signature(&function.signature);
                }
            }
            Item::Machine(machine) => self.insert_machine(machine),
            Item::Platform(platform) => self.insert_platform(platform),
            Item::Target(_) | Item::TrustDefinition(_) | Item::Use(_) => {}
        }
    }

    fn insert_machine(&mut self, machine: &Machine) {
        for owned_data in &machine.owned_data {
            self.insert_owned_data(owned_data);
        }

        for state in &machine.states {
            self.insert_state(state);
        }
    }

    fn insert_owned_data(&mut self, owned_data: &OwnedData) {
        self.insert_type_reference(&owned_data.type_reference);

        if let Some(initial_value) = &owned_data.initial_value {
            self.insert_expression(initial_value);
        }
    }

    fn insert_platform(&mut self, platform: &Platform) {
        for state in &platform.states {
            self.insert_state_signature(state);
        }
    }

    fn insert_state(&mut self, state: &State) {
        for parameter in &state.parameters {
            self.insert_type_reference(&parameter.type_reference);
        }

        if let Some(return_type) = &state.return_type {
            self.insert_type_reference(return_type);
        }

        for statement in &state.statements {
            self.insert_statement(statement);
        }
    }

    fn insert_state_signature(&mut self, signature: &StateSignature) {
        for parameter in &signature.parameters {
            self.insert_type_reference(&parameter.type_reference);
        }

        if let Some(return_type) = &signature.return_type {
            self.insert_type_reference(return_type);
        }
    }

    fn insert_statement(&mut self, statement: &Statement) {
        match statement {
            Statement::Assignment(assignment) => {
                self.insert_expression(&assignment.target);
                self.insert_expression(&assignment.value);
            }
            Statement::Call(call) => {
                for argument in &call.arguments {
                    self.insert_expression(argument);
                }
            }
            Statement::Expression(expression) => {
                self.insert_expression(expression);
            }
            Statement::LocalData(local_data) => {
                self.insert_type_reference(&local_data.type_reference);
            }
            Statement::Transition(transition) => {
                self.insert_transition_target(&transition.target);

                if let Some(continuation) = &transition.continuation {
                    self.insert_transition_target(continuation);
                }

                if let TransitionGuard::When(expression) = &transition.guard {
                    self.insert_expression(expression);
                }
            }
        }
    }

    fn insert_transition_target(&mut self, target: &TransitionTarget) {
        if let TransitionTarget::Named { arguments, .. } = target {
            for argument in arguments {
                self.insert_expression(argument);
            }
        }
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
    }
}
