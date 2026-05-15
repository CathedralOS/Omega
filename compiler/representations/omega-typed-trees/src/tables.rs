use crate::data::DataMember;
use crate::expression::{Expression, ExpressionTable};
use crate::machine::OwnedData;
use crate::signature::{StateParameter, StateSignature};
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

        for data_definition in typed_trees.data_definitions() {
            tables.insert_data_definition(
                typed_trees.data_members(data_definition),
                &typed_trees.type_constraints,
                &typed_trees.type_reference_arguments,
            );
        }

        for platform in typed_trees.platforms() {
            tables.insert_platform(
                typed_trees.platform_state_signatures(platform),
                typed_trees,
                &typed_trees.type_constraints,
                &typed_trees.type_reference_arguments,
            );
        }

        for machine in typed_trees.machines() {
            tables.insert_machine(
                typed_trees.machine_owned_data(machine),
                typed_trees.machine_states(machine),
                typed_trees,
                &typed_trees.type_constraints,
                &typed_trees.type_reference_arguments,
            );
        }

        tables
    }

    pub fn from_typed_trees_with_state_spans(typed_trees: &mut TypedTrees) -> Self {
        let mut tables = Self::default();
        let TypedTrees {
            root_data_definitions,
            data_definitions,
            data_members,
            root_invariants,
            invariant_definitions,
            root_machines,
            machines,
            machine_owned_data,
            machine_states,
            state_parameters,
            state_statements,
            type_reference_arguments,
            root_platforms,
            platforms,
            platform_state_signatures,
            type_constraints,
            ..
        } = typed_trees;

        for invariant in invariant_definitions.span_or_empty(*root_invariants) {
            tables.insert_type_constraints(invariant.constraints, type_constraints);
        }

        for data_definition in data_definitions.span_or_empty(*root_data_definitions) {
            tables.insert_data_definition(
                data_members.span_or_empty(data_definition.members),
                type_constraints,
                type_reference_arguments,
            );
        }

        for platform in platforms.span_or_empty(*root_platforms) {
            tables.insert_platform_with_parameter_arena(
                platform_state_signatures.span_or_empty(platform.states),
                state_parameters,
                type_constraints,
                type_reference_arguments,
            );
        }

        for machine in machines.span_mut_or_empty(*root_machines) {
            tables.insert_machine_with_state_spans(
                machine_owned_data.span_or_empty(machine.owned_data),
                machine_states.span_mut_or_empty(machine.states),
                state_parameters,
                state_statements,
                type_constraints,
                type_reference_arguments,
            );
        }

        tables
    }

    fn insert_data_definition(
        &mut self,
        members: &[DataMember],
        type_constraints: &Arena<TypeConstraint>,
        type_reference_arguments: &Arena<TypeReference>,
    ) {
        for member in members {
            if let DataMember::Field(field) = member {
                self.insert_type_reference(
                    &field.type_reference,
                    type_constraints,
                    type_reference_arguments,
                );
            }
        }
    }

    fn insert_platform(
        &mut self,
        states: &[StateSignature],
        typed_trees: &TypedTrees,
        type_constraints: &Arena<TypeConstraint>,
        type_reference_arguments: &Arena<TypeReference>,
    ) {
        for state in states {
            self.insert_state_signature(
                state,
                typed_trees.state_signature_parameters(state),
                type_constraints,
                type_reference_arguments,
            );
        }
    }

    fn insert_platform_with_parameter_arena(
        &mut self,
        states: &[StateSignature],
        state_parameters: &Arena<StateParameter>,
        type_constraints: &Arena<TypeConstraint>,
        type_reference_arguments: &Arena<TypeReference>,
    ) {
        for state in states {
            self.insert_state_signature(
                state,
                state_parameters.span_or_empty(state.parameters),
                type_constraints,
                type_reference_arguments,
            );
        }
    }

    fn insert_machine(
        &mut self,
        owned_data: &[OwnedData],
        states: &[State],
        typed_trees: &TypedTrees,
        type_constraints: &Arena<TypeConstraint>,
        type_reference_arguments: &Arena<TypeReference>,
    ) {
        for owned_data in owned_data {
            self.insert_owned_data(owned_data, type_constraints, type_reference_arguments);
        }

        for state in states {
            self.insert_state(
                state,
                typed_trees.state_parameters(state),
                typed_trees.state_statements(state),
                type_constraints,
                type_reference_arguments,
            );
        }
    }

    fn insert_machine_with_state_spans(
        &mut self,
        owned_data: &[OwnedData],
        states: &mut [State],
        state_parameters: &Arena<StateParameter>,
        state_statements: &Arena<Statement>,
        type_constraints: &Arena<TypeConstraint>,
        type_reference_arguments: &Arena<TypeReference>,
    ) {
        for owned_data in owned_data {
            self.insert_owned_data(owned_data, type_constraints, type_reference_arguments);
        }

        for state in states {
            self.insert_state_with_statement_span(
                state,
                state_parameters.span_or_empty(state.parameters),
                state_statements.span_or_empty(state.statements),
                type_constraints,
                type_reference_arguments,
            );
        }
    }

    fn insert_owned_data(
        &mut self,
        owned_data: &OwnedData,
        type_constraints: &Arena<TypeConstraint>,
        type_reference_arguments: &Arena<TypeReference>,
    ) {
        self.insert_type_reference(
            &owned_data.type_reference,
            type_constraints,
            type_reference_arguments,
        );

        if let Some(initial_value) = &owned_data.initial_value {
            self.insert_expression(initial_value);
        }
    }

    fn insert_state(
        &mut self,
        state: &State,
        parameters: &[StateParameter],
        statements: &[Statement],
        type_constraints: &Arena<TypeConstraint>,
        type_reference_arguments: &Arena<TypeReference>,
    ) {
        for parameter in parameters {
            self.insert_type_reference(
                &parameter.type_reference,
                type_constraints,
                type_reference_arguments,
            );
        }

        if let Some(return_type) = &state.return_type {
            self.insert_type_reference(return_type, type_constraints, type_reference_arguments);
        }

        for statement in statements {
            self.insert_statement(statement, type_constraints, type_reference_arguments);
        }
    }

    fn insert_state_with_statement_span(
        &mut self,
        state: &mut State,
        parameters: &[StateParameter],
        statements: &[Statement],
        type_constraints: &Arena<TypeConstraint>,
        type_reference_arguments: &Arena<TypeReference>,
    ) {
        for parameter in parameters {
            self.insert_type_reference(
                &parameter.type_reference,
                type_constraints,
                type_reference_arguments,
            );
        }

        if let Some(return_type) = &state.return_type {
            self.insert_type_reference(return_type, type_constraints, type_reference_arguments);
        }

        let mut statement_nodes = HandleSpan::empty();
        for statement in statements {
            let handle = self.statements.insert_tree(
                statement,
                &mut self.expressions,
                &mut self.type_references,
                type_constraints,
                type_reference_arguments,
            );
            statement_nodes.push_contiguous(handle);
        }

        state.statement_nodes = statement_nodes;
    }

    fn insert_state_signature(
        &mut self,
        signature: &StateSignature,
        parameters: &[StateParameter],
        type_constraints: &Arena<TypeConstraint>,
        type_reference_arguments: &Arena<TypeReference>,
    ) {
        for parameter in parameters {
            self.insert_type_reference(
                &parameter.type_reference,
                type_constraints,
                type_reference_arguments,
            );
        }

        if let Some(return_type) = &signature.return_type {
            self.insert_type_reference(return_type, type_constraints, type_reference_arguments);
        }
    }

    fn insert_statement(
        &mut self,
        statement: &Statement,
        type_constraints: &Arena<TypeConstraint>,
        type_reference_arguments: &Arena<TypeReference>,
    ) {
        self.statements.insert_tree(
            statement,
            &mut self.expressions,
            &mut self.type_references,
            type_constraints,
            type_reference_arguments,
        );
    }

    fn insert_type_reference(
        &mut self,
        type_reference: &TypeReference,
        type_constraints: &Arena<TypeConstraint>,
        type_reference_arguments: &Arena<TypeReference>,
    ) {
        self.type_references.insert_tree(
            type_reference,
            &mut self.expressions,
            type_constraints,
            type_reference_arguments,
        );
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
        let mut typed_trees = TypedTrees::default();
        let mut machine = Machine {
            symbol: SymbolHandle::invalid(),
            name: ProgramName::generated("main"),
            contains: Default::default(),
            owned_data: Default::default(),
            states: Default::default(),
        };
        let mut state = State {
            symbol: SymbolHandle::invalid(),
            name: ProgramName::generated("entry"),
            parameters: Default::default(),
            return_type: Some(TypeReference::Named {
                symbol: SymbolHandle::invalid(),
                name: ProgramName::generated("i32"),
            }),
            statements: Default::default(),
            statement_nodes: HandleSpan::empty(),
        };
        typed_trees.push_state_statement(
            &mut state,
            Statement::Transition(Transition {
                target: TransitionTarget::Terminal,
                continuation: None,
                guard: TransitionGuard::When(Expression::Integer(1)),
            }),
        );
        typed_trees.push_machine_state(&mut machine, state);
        typed_trees.push_machine(machine);

        typed_trees.rebuild_tables();

        assert_eq!(typed_trees.type_reference_table.type_reference_count(), 1);
        assert_eq!(typed_trees.expression_table.expression_count(), 1);
        assert_eq!(typed_trees.statement_table.statement_count(), 1);
        assert_eq!(
            typed_trees.machine_states(&typed_trees.machines()[0])[0]
                .statement_nodes
                .count(),
            1
        );
    }
}
