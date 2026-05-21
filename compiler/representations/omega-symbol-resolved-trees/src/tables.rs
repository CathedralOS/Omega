use crate::data::DataMember;
use crate::expression::ExpressionTable;
use crate::machine::{Machine, OwnedData};
use crate::signature::{StateParameter, StateSignature};
use crate::state::State;
use crate::statement::{Statement, StatementTable};
use crate::types::{TypeConstraint, TypeReference, TypeReferenceTable};
use crate::{SymbolResolvedDeclarationStorage, SymbolResolvedTrees};
use omega_core::arena::{Arena, HandleSpan};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolResolvedTreeTables {
    pub bodies: ResolvedBodyTables,
    pub types: ResolvedTypeTables,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedBodyTables {
    pub expressions: ExpressionTable,
    pub statements: StatementTable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedTypeTables {
    pub references: TypeReferenceTable,
}

impl SymbolResolvedTreeTables {
    pub fn from_symbol_resolved_trees_with_state_spans(
        symbol_resolved_trees: &mut SymbolResolvedTrees,
    ) -> Self {
        let mut tables = Self::default();
        let SymbolResolvedTrees {
            roots,
            tables: source_tables,
            ..
        } = symbol_resolved_trees;
        let type_constraints = &source_tables.types.constraints;
        let source_expressions = &source_tables.bodies.expressions;
        tables.bodies.expressions = source_expressions.clone();
        let SymbolResolvedDeclarationStorage {
            data_members,
            machine_owned_data,
            machine_state_handles,
            machine_states,
            platform_state_signatures,
            trait_machine_signatures,
            state_parameters,
            statement_path_members,
            state_statements,
            child_type_references,
            ..
        } = &mut source_tables.declarations;

        for data_definition in &roots.data_definitions {
            tables.insert_data_definition(
                data_members.span_or_empty(data_definition.members),
                child_type_references,
                type_constraints,
                source_expressions,
            );
        }

        for platform in &roots.platforms {
            tables.insert_platform(
                platform_state_signatures.span_or_empty(platform.states),
                state_parameters,
                child_type_references,
                type_constraints,
                source_expressions,
            );
        }

        for trait_definition in &roots.traits {
            tables.insert_platform(
                trait_machine_signatures.span_or_empty(trait_definition.machines),
                state_parameters,
                child_type_references,
                type_constraints,
                source_expressions,
            );
        }

        for machine in &roots.machines {
            tables.insert_machine_with_state_spans(
                machine,
                machine_owned_data,
                machine_state_handles,
                machine_states,
                state_parameters,
                statement_path_members,
                state_statements,
                child_type_references,
                type_constraints,
                source_expressions,
            );
        }

        tables
    }

    fn insert_data_definition(
        &mut self,
        members: &[DataMember],
        child_type_references: &Arena<TypeReference>,
        type_constraints: &Arena<TypeConstraint>,
        source_expressions: &ExpressionTable,
    ) {
        for member in members {
            if let DataMember::Field(field) = member {
                self.insert_type_reference(
                    &field.type_reference,
                    child_type_references,
                    type_constraints,
                    source_expressions,
                );
            }
        }
    }

    fn insert_platform(
        &mut self,
        states: &[StateSignature],
        state_parameters: &Arena<StateParameter>,
        child_type_references: &Arena<TypeReference>,
        type_constraints: &Arena<TypeConstraint>,
        source_expressions: &ExpressionTable,
    ) {
        for state in states {
            self.insert_state_signature(
                state_parameters.span_or_empty(state.parameters),
                state,
                child_type_references,
                type_constraints,
                source_expressions,
            );
        }
    }

    fn insert_machine_with_state_spans(
        &mut self,
        machine: &Machine,
        machine_owned_data: &Arena<OwnedData>,
        machine_state_handles: &Arena<omega_core::arena::Handle<State>>,
        machine_states: &mut Arena<State>,
        state_parameters: &Arena<StateParameter>,
        statement_path_members: &Arena<crate::name::DiagnosticName>,
        state_statements: &Arena<Statement>,
        child_type_references: &Arena<TypeReference>,
        type_constraints: &Arena<TypeConstraint>,
        source_expressions: &ExpressionTable,
    ) {
        for owned_data in machine_owned_data.span_or_empty(machine.owned_data) {
            self.insert_owned_data(
                owned_data,
                child_type_references,
                type_constraints,
                source_expressions,
            );
        }

        for state in machine_state_handles
            .span_or_empty(machine.states)
            .iter()
            .copied()
        {
            let state = machine_states.get_mut(state);
            self.insert_state_with_statement_span(
                state,
                state_parameters,
                statement_path_members,
                state_statements,
                child_type_references,
                type_constraints,
                source_expressions,
            );
        }
    }

    fn insert_owned_data(
        &mut self,
        owned_data: &OwnedData,
        child_type_references: &Arena<TypeReference>,
        type_constraints: &Arena<TypeConstraint>,
        source_expressions: &ExpressionTable,
    ) {
        self.insert_type_reference(
            &owned_data.type_reference,
            child_type_references,
            type_constraints,
            source_expressions,
        );
    }

    fn insert_state_with_statement_span(
        &mut self,
        state: &mut State,
        state_parameters: &Arena<StateParameter>,
        statement_path_members: &Arena<crate::name::DiagnosticName>,
        state_statements: &Arena<Statement>,
        child_type_references: &Arena<TypeReference>,
        type_constraints: &Arena<TypeConstraint>,
        source_expressions: &ExpressionTable,
    ) {
        for parameter in state_parameters.span_or_empty(state.parameters) {
            self.insert_type_reference(
                &parameter.type_reference,
                child_type_references,
                type_constraints,
                source_expressions,
            );
        }

        if let Some(return_type) = &state.return_type {
            self.insert_type_reference(
                return_type,
                child_type_references,
                type_constraints,
                source_expressions,
            );
        }

        let mut statement_nodes = HandleSpan::empty();
        for statement in state_statements.span_or_empty(state.statements) {
            let statement = self.bodies.statements.insert_tree(
                statement,
                source_expressions,
                &mut self.bodies.expressions,
                &mut self.types.references,
                child_type_references,
                type_constraints,
                source_expressions,
                statement_path_members,
                false,
            );
            statement_nodes.push_contiguous(statement);
        }

        state.statement_nodes = statement_nodes;
    }

    fn insert_state_signature(
        &mut self,
        parameters: &[StateParameter],
        signature: &StateSignature,
        child_type_references: &Arena<TypeReference>,
        type_constraints: &Arena<TypeConstraint>,
        source_expressions: &ExpressionTable,
    ) {
        for parameter in parameters {
            self.insert_type_reference(
                &parameter.type_reference,
                child_type_references,
                type_constraints,
                source_expressions,
            );
        }

        if let Some(return_type) = &signature.return_type {
            self.insert_type_reference(
                return_type,
                child_type_references,
                type_constraints,
                source_expressions,
            );
        }
    }

    fn insert_type_reference(
        &mut self,
        type_reference: &TypeReference,
        child_type_references: &Arena<TypeReference>,
        type_constraints: &Arena<TypeConstraint>,
        source_expressions: &ExpressionTable,
    ) {
        self.types.references.insert_tree(
            type_reference,
            &mut self.bodies.expressions,
            child_type_references,
            type_constraints,
            source_expressions,
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::SymbolResolvedTrees;
    use crate::expression::ExpressionNode;
    use crate::machine::{Machine, MachineStorage};
    use crate::name::DiagnosticName;
    use crate::state::{State, StateStorage};
    use crate::statement::{Statement, Transition, TransitionGuard, TransitionTarget};
    use crate::types::TypeReference;
    use omega_core::arena::HandleSpan;
    use omega_core::symbols::SymbolHandle;

    #[test]
    fn rebuild_tables_collects_typed_program_payloads() {
        let mut program = SymbolResolvedTrees::default();
        let guard = program
            .tables
            .bodies
            .expressions
            .insert(ExpressionNode::Integer(1));
        let statements =
            program
                .tables
                .declarations
                .state_statements
                .insert_many([Statement::Transition(Transition {
                    target: TransitionTarget::Terminal,
                    continuation: None,
                    guard: TransitionGuard::When(guard),
                })]);
        let state = program.tables.declarations.machine_states.append(State {
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::generated("entry"),
            storage: StateStorage {
                parameters: HandleSpan::empty(),
                return_type: Some(TypeReference::Named {
                    symbol: SymbolHandle::invalid(),
                    name: DiagnosticName::generated("i32"),
                }),
                statements,
                statement_nodes: HandleSpan::empty(),
            },
        });
        let states = program
            .tables
            .declarations
            .machine_state_handles
            .insert_many([state]);
        program.machines.push(Machine {
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::generated("main"),
            storage: MachineStorage {
                contains: HandleSpan::empty(),
                owned_data: HandleSpan::empty(),
                states,
            },
        });

        program.rebuild_tables();

        assert_eq!(program.tables.types.references.type_reference_count(), 1);
        assert_eq!(program.tables.bodies.expressions.expression_count(), 1);
        assert_eq!(program.tables.bodies.statements.statement_count(), 1);
        let state = program
            .machine_state_handles(program.machines[0].states)
            .first()
            .map(|state| program.machine_state(*state))
            .expect("entry state");
        assert_eq!(state.statement_nodes.count(), 1);
    }
}
