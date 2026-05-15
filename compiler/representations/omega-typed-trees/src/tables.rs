use crate::data::{DataDefinition, DataMember};
use crate::expression::{Expression, ExpressionTable};
use crate::machine::{Machine, OwnedData};
use crate::platform::Platform;
use crate::signature::StateSignature;
use crate::state::State;
use crate::statement::{Statement, StatementTable};
use crate::typed_trees::TypedTrees;
use crate::types::{TypeConstraint, TypeReference, TypeReferenceTable};
use omega_core::arena::{Arena, HandleSpan};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedProgramTables {
    pub expressions: ExpressionTable,
    pub statements: StatementTable,
    pub type_references: TypeReferenceTable,
}

impl TypedProgramTables {
    pub fn from_typed_trees(typed_trees: &TypedTrees) -> Self {
        let mut tables = Self::default();

        for invariant in typed_trees.invariant_definitions() {
            tables.insert_type_constraints(invariant.constraints, &typed_trees.type_constraints);
        }

        for data_definition in &typed_trees.data_definitions {
            tables.insert_data_definition(data_definition, &typed_trees.type_constraints);
        }

        for platform in &typed_trees.platforms {
            tables.insert_platform(platform, &typed_trees.type_constraints);
        }

        for machine in &typed_trees.machines {
            tables.insert_machine(machine, &typed_trees.type_constraints);
        }

        tables
    }

    pub fn from_typed_trees_with_state_spans(typed_trees: &mut TypedTrees) -> Self {
        let mut tables = Self::default();
        let type_constraints = &typed_trees.type_constraints;

        for invariant in typed_trees.invariant_definitions() {
            tables.insert_type_constraints(invariant.constraints, type_constraints);
        }

        for data_definition in &typed_trees.data_definitions {
            tables.insert_data_definition(data_definition, type_constraints);
        }

        for platform in &typed_trees.platforms {
            tables.insert_platform(platform, type_constraints);
        }

        for machine in &mut typed_trees.machines {
            tables.insert_machine_with_state_spans(machine, type_constraints);
        }

        tables
    }

    fn insert_data_definition(
        &mut self,
        data_definition: &DataDefinition,
        type_constraints: &Arena<TypeConstraint>,
    ) {
        for member in &data_definition.members {
            if let DataMember::Field(field) = member {
                self.insert_type_reference(&field.type_reference, type_constraints);
            }
        }
    }

    fn insert_platform(&mut self, platform: &Platform, type_constraints: &Arena<TypeConstraint>) {
        for state in &platform.states {
            self.insert_state_signature(state, type_constraints);
        }
    }

    fn insert_machine(&mut self, machine: &Machine, type_constraints: &Arena<TypeConstraint>) {
        for owned_data in &machine.owned_data {
            self.insert_owned_data(owned_data, type_constraints);
        }

        for state in &machine.states {
            self.insert_state(state, type_constraints);
        }
    }

    fn insert_machine_with_state_spans(
        &mut self,
        machine: &mut Machine,
        type_constraints: &Arena<TypeConstraint>,
    ) {
        for owned_data in &machine.owned_data {
            self.insert_owned_data(owned_data, type_constraints);
        }

        for state in &mut machine.states {
            self.insert_state_with_statement_span(state, type_constraints);
        }
    }

    fn insert_owned_data(
        &mut self,
        owned_data: &OwnedData,
        type_constraints: &Arena<TypeConstraint>,
    ) {
        self.insert_type_reference(&owned_data.type_reference, type_constraints);

        if let Some(initial_value) = &owned_data.initial_value {
            self.insert_expression(initial_value);
        }
    }

    fn insert_state(&mut self, state: &State, type_constraints: &Arena<TypeConstraint>) {
        for parameter in &state.parameters {
            self.insert_type_reference(&parameter.type_reference, type_constraints);
        }

        if let Some(return_type) = &state.return_type {
            self.insert_type_reference(return_type, type_constraints);
        }

        for statement in &state.statements {
            self.insert_statement(statement, type_constraints);
        }
    }

    fn insert_state_with_statement_span(
        &mut self,
        state: &mut State,
        type_constraints: &Arena<TypeConstraint>,
    ) {
        for parameter in &state.parameters {
            self.insert_type_reference(&parameter.type_reference, type_constraints);
        }

        if let Some(return_type) = &state.return_type {
            self.insert_type_reference(return_type, type_constraints);
        }

        let mut statements = HandleSpan::empty();
        for statement in &state.statements {
            let handle = self.statements.insert_tree(
                statement,
                &mut self.expressions,
                &mut self.type_references,
                type_constraints,
            );
            statements.push_contiguous(handle);
        }

        state.statement_nodes = statements;
    }

    fn insert_state_signature(
        &mut self,
        signature: &StateSignature,
        type_constraints: &Arena<TypeConstraint>,
    ) {
        for parameter in &signature.parameters {
            self.insert_type_reference(&parameter.type_reference, type_constraints);
        }

        if let Some(return_type) = &signature.return_type {
            self.insert_type_reference(return_type, type_constraints);
        }
    }

    fn insert_statement(
        &mut self,
        statement: &Statement,
        type_constraints: &Arena<TypeConstraint>,
    ) {
        self.statements.insert_tree(
            statement,
            &mut self.expressions,
            &mut self.type_references,
            type_constraints,
        );
    }

    fn insert_type_reference(
        &mut self,
        type_reference: &TypeReference,
        type_constraints: &Arena<TypeConstraint>,
    ) {
        self.type_references
            .insert_tree(type_reference, &mut self.expressions, type_constraints);
    }

    fn insert_type_constraints(
        &mut self,
        constraints: omega_core::arena::HandleSpan<TypeConstraint>,
        type_constraints: &Arena<TypeConstraint>,
    ) {
        for constraint in type_constraints.span_or_empty(constraints) {
            if let TypeConstraint::Range { minimum, maximum } = constraint {
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
    use crate::expression::Expression;
    use crate::machine::Machine;
    use crate::name::ProgramName;
    use crate::state::State;
    use crate::statement::{Statement, Transition, TransitionGuard, TransitionTarget};
    use crate::typed_trees::TypedTrees;
    use crate::types::TypeReference;
    use omega_core::arena::HandleSpan;
    use omega_core::symbols::SymbolHandle;

    #[test]
    fn rebuild_tables_collects_typed_program_payloads() {
        let mut typed_trees = TypedTrees {
            machines: vec![Machine {
                symbol: SymbolHandle::invalid(),
                name: ProgramName::generated("main"),
                contains: Vec::new(),
                owned_data: Vec::new(),
                states: vec![State {
                    symbol: SymbolHandle::invalid(),
                    name: ProgramName::generated("entry"),
                    parameters: Vec::new(),
                    return_type: Some(TypeReference::Named {
                        symbol: SymbolHandle::invalid(),
                        name: ProgramName::generated("i32"),
                    }),
                    statements: vec![Statement::Transition(Transition {
                        target: TransitionTarget::Terminal,
                        continuation: None,
                        guard: TransitionGuard::When(Expression::Integer(1)),
                    })],
                    statement_nodes: HandleSpan::empty(),
                }],
            }],
            ..TypedTrees::default()
        };

        typed_trees.rebuild_tables();

        assert_eq!(typed_trees.type_reference_table.type_reference_count(), 1);
        assert_eq!(typed_trees.expression_table.expression_count(), 1);
        assert_eq!(typed_trees.statement_table.statement_count(), 1);
        assert_eq!(typed_trees.machines[0].states[0].statement_nodes.count(), 1);
    }
}
