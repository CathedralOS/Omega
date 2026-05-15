use crate::SymbolResolvedTrees;
use crate::data::DataMember;
use crate::expression::{Expression, ExpressionTable};
use crate::machine::{Machine, OwnedData};
use crate::signature::{StateParameter, StateSignature};
use crate::state::State;
use crate::statement::StatementTable;
use crate::types::{TypeConstraint, TypeReference, TypeReferenceTable};
use omega_core::arena::{Arena, Handle, HandleSpan};

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
        let type_constraints = source_tables.types.constraints.clone();
        let declarations = &source_tables.declarations;

        for invariant in &roots.invariant_definitions {
            tables.insert_type_constraints(invariant.constraints, &type_constraints);
        }

        for data_definition in &roots.data_definitions {
            tables.insert_data_definition(
                declarations
                    .data_members
                    .span_or_empty(data_definition.members),
                &type_constraints,
            );
        }

        for platform in &roots.platforms {
            tables.insert_platform(
                declarations
                    .platform_state_signatures
                    .span_or_empty(platform.states),
                &declarations.state_parameters,
                &type_constraints,
            );
        }

        let state_parameters = &declarations.state_parameters;
        roots.machines.for_each_mut(|machine| {
            tables.insert_machine_with_state_spans(machine, state_parameters, &type_constraints);
        });

        tables
    }

    fn insert_data_definition(
        &mut self,
        members: &[DataMember],
        type_constraints: &Arena<TypeConstraint>,
    ) {
        for member in members {
            if let DataMember::Field(field) = member {
                self.insert_type_reference(&field.type_reference, type_constraints);
            }
        }
    }

    fn insert_platform(
        &mut self,
        states: &[StateSignature],
        state_parameters: &Arena<StateParameter>,
        type_constraints: &Arena<TypeConstraint>,
    ) {
        for state in states {
            self.insert_state_signature(
                state_parameters.span_or_empty(state.parameters),
                state,
                type_constraints,
            );
        }
    }

    fn insert_machine_with_state_spans(
        &mut self,
        machine: &mut Machine,
        state_parameters: &Arena<StateParameter>,
        type_constraints: &Arena<TypeConstraint>,
    ) {
        for owned_data in &machine.owned_data {
            self.insert_owned_data(owned_data, type_constraints);
        }

        for state in &mut machine.states {
            self.insert_state_with_statement_span(state, state_parameters, type_constraints);
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

    fn insert_state_with_statement_span(
        &mut self,
        state: &mut State,
        state_parameters: &Arena<StateParameter>,
        type_constraints: &Arena<TypeConstraint>,
    ) {
        for parameter in state_parameters.span_or_empty(state.parameters) {
            self.insert_type_reference(&parameter.type_reference, type_constraints);
        }

        if let Some(return_type) = &state.return_type {
            self.insert_type_reference(return_type, type_constraints);
        }

        let mut start = Handle::invalid();
        let mut count = 0u32;
        for statement in &state.statements {
            let handle = self.bodies.statements.insert_tree(
                statement,
                &mut self.bodies.expressions,
                &mut self.types.references,
                type_constraints,
            );
            if count == 0 {
                start = handle;
            }
            count = count.checked_add(1).expect("state statement span overflow");
        }

        state.statement_nodes = if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        };
    }

    fn insert_state_signature(
        &mut self,
        parameters: &[StateParameter],
        signature: &StateSignature,
        type_constraints: &Arena<TypeConstraint>,
    ) {
        for parameter in parameters {
            self.insert_type_reference(&parameter.type_reference, type_constraints);
        }

        if let Some(return_type) = &signature.return_type {
            self.insert_type_reference(return_type, type_constraints);
        }
    }

    fn insert_type_reference(
        &mut self,
        type_reference: &TypeReference,
        type_constraints: &Arena<TypeConstraint>,
    ) {
        self.types.references.insert_tree(
            type_reference,
            &mut self.bodies.expressions,
            type_constraints,
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
        self.bodies.expressions.insert_tree(expression);
    }
}

#[cfg(test)]
mod tests {
    use crate::SymbolResolvedTrees;
    use crate::expression::Expression;
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
        program.machines.push(Machine {
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::generated("main"),
            storage: MachineStorage {
                contains: Vec::new(),
                owned_data: Vec::new(),
                states: vec![State {
                    symbol: SymbolHandle::invalid(),
                    name: DiagnosticName::generated("entry"),
                    storage: StateStorage {
                        parameters: HandleSpan::empty(),
                        return_type: Some(TypeReference::Named {
                            symbol: SymbolHandle::invalid(),
                            name: DiagnosticName::generated("i32"),
                        }),
                        statements: vec![Statement::Transition(Transition {
                            target: TransitionTarget::Terminal,
                            continuation: None,
                            guard: TransitionGuard::When(Expression::Integer(1)),
                        })],
                        statement_nodes: HandleSpan::empty(),
                    },
                }],
            },
        });

        program.rebuild_tables();

        assert_eq!(program.tables.types.references.type_reference_count(), 1);
        assert_eq!(program.tables.bodies.expressions.expression_count(), 1);
        assert_eq!(program.tables.bodies.statements.statement_count(), 1);
        assert_eq!(program.machines[0].states[0].statement_nodes.count(), 1);
    }
}
