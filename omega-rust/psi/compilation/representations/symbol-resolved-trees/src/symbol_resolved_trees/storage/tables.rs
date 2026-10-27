use crate::data::{DataField, DataMember};
use crate::expression::{ExpressionSource, ExpressionTable};
use crate::machine::{Machine, OwnedData};
use crate::name::DiagnosticName;
use crate::signature::{StateParameter, StateSignature};
use crate::state::State;
use crate::statement::{Statement, StatementNode, StatementTable};
use crate::types::{TypeConstraint, TypeReference, TypeReferenceTable};
use crate::{SymbolResolvedDeclarationStorage, SymbolResolvedTrees};
use arena::{Arena, HandleSpan};

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

/// The source coordinates every declaration walker transplants rows through:
/// the child type-reference and constraint rows, the expression table the
/// copies read, and the three declaration rosters a nested statement or
/// signature reads. Each walker used to thread these as separate parameters;
/// one value now carries them.
#[derive(Clone, Copy)]
struct Transplant<'tree> {
    child_type_references: &'tree Arena<TypeReference>,
    type_constraints: &'tree Arena<TypeConstraint>,
    source_expressions: ExpressionSource<'tree>,
    data_payload_fields: &'tree Arena<DataField>,
    state_parameters: &'tree Arena<StateParameter>,
    statement_path_members: &'tree Arena<DiagnosticName>,
    state_statements: &'tree Arena<Statement>,
}

impl SymbolResolvedTreeTables {
    pub fn from_symbol_resolved_trees_with_state_spans(
        symbol_resolved_trees: &mut SymbolResolvedTrees,
    ) -> Self {
        let mut tables = Self::default();
        tables.append_declaration_tables(
            symbol_resolved_trees,
            &crate::SymbolResolvedRoots::default(),
            &[],
            0,
        );
        tables
    }

    /// Retain normalized rows belonging to earlier completed bodies. The
    /// successor owns a clone of the predecessor, extended with resolved body
    /// graphs in declaration storage. Its normalized statements and types are
    /// still the exact predecessor tables: final routing runs after this append.
    /// Transfer those owned tables instead of copying the predecessor twice.
    pub fn from_body_completion(
        symbol_resolved_trees: &mut SymbolResolvedTrees,
        predecessor: &SymbolResolvedTrees,
        completed_machines: &[symbols::SymbolHandle],
    ) -> Self {
        let mut tables = Self {
            bodies: ResolvedBodyTables {
                expressions: ExpressionTable::default(),
                statements: std::mem::take(&mut symbol_resolved_trees.tables.bodies.statements),
            },
            types: ResolvedTypeTables {
                references: std::mem::take(&mut symbol_resolved_trees.tables.types.references),
            },
        };
        tables.append_declaration_tables(
            symbol_resolved_trees,
            &predecessor.roots,
            completed_machines,
            predecessor.tables.declarations.assembly_blocks.len(),
        );
        tables
    }

    /// The same table insertions serve complete lowering and continuation.
    /// Root frontiers select new declarations; exact machine identities select
    /// existing declarations whose implementations have just been completed.
    fn append_declaration_tables(
        &mut self,
        symbol_resolved_trees: &mut SymbolResolvedTrees,
        frontier: &crate::SymbolResolvedRoots,
        completed_machines: &[symbols::SymbolHandle],
        assembly_frontier: usize,
    ) {
        let tables = self;
        let SymbolResolvedTrees {
            roots,
            tables: source_tables,
            ..
        } = symbol_resolved_trees;
        // The expressions already have their final identities. Transfer the arena
        // and append normalized type-expression copies beside their originals;
        // a whole-table snapshot would only be discarded after normalization.
        tables.bodies.expressions = std::mem::take(&mut source_tables.bodies.expressions);
        let source_expressions = ExpressionSource::Own;
        let SymbolResolvedDeclarationStorage {
            data_members,
            data_payload_fields,
            declared_type_equations,
            machine_owned_data,
            machine_state_handles,
            machine_states,
            trait_machine_signatures,
            operator_definitions,
            proposition_binders,
            state_parameters,
            statement_path_members,
            state_statements,
            assembly_blocks,
            child_type_references,
            ..
        } = &mut source_tables.declarations;
        let transplant = Transplant {
            child_type_references,
            type_constraints: &source_tables.types.constraints,
            source_expressions,
            data_payload_fields,
            state_parameters,
            statement_path_members,
            state_statements,
        };

        // Assembly scopes use the same statement normalization as machine
        // states. Only appended blocks are new during deferred completion;
        // retained predecessor views keep their exact node coordinates.
        let blocks: Vec<_> = assembly_blocks
            .iter()
            .skip(assembly_frontier)
            .map(|(handle, block)| (handle, block.instructions))
            .collect();
        for (handle, instructions) in blocks {
            let nodes = tables.normalize_statements(
                &transplant,
                transplant.state_statements.span_or_empty(instructions),
            );
            assembly_blocks.get_mut(handle).statement_nodes = nodes;
        }

        for data_definition in roots
            .data_definitions
            .iter()
            .skip(frontier.data_definitions.len())
        {
            tables.insert_data_definition(
                &transplant,
                data_members.span_or_empty(data_definition.members),
            );
        }

        for domain_definition in roots
            .domain_definitions
            .iter()
            .skip(frontier.domain_definitions.len())
        {
            tables.insert_type_reference(&transplant, &domain_definition.target_type);
            for operator in operator_definitions.span_or_empty(domain_definition.operators) {
                tables.insert_operator(&transplant, operator);
            }
        }

        for proposition in roots.propositions.iter().skip(frontier.propositions.len()) {
            for binder in proposition_binders.span_or_empty(proposition.binders) {
                if let crate::proposition::PropositionBinderKind::Const { type_reference } =
                    &binder.kind
                {
                    tables.insert_type_reference(&transplant, type_reference);
                }
            }
            for parameter in state_parameters.span_or_empty(proposition.parameters) {
                tables.insert_type_reference(&transplant, &parameter.type_reference);
            }
            if let crate::proposition::PropositionBody::Witness { evidence } = &proposition.body {
                tables.insert_type_reference(&transplant, evidence);
            }
        }

        for trait_definition in roots.traits.iter().skip(frontier.traits.len()) {
            tables.insert_platform(
                &transplant,
                trait_machine_signatures.span_or_empty(trait_definition.machines),
            );
        }

        for (_, machine) in roots
            .machines
            .iter()
            .enumerate()
            .filter(|(ordinal, machine)| {
                *ordinal >= frontier.machines.len() || completed_machines.contains(&machine.symbol)
            })
        {
            tables.insert_machine_with_state_spans(
                &transplant,
                machine,
                declared_type_equations,
                machine_owned_data,
                machine_state_handles,
                machine_states,
            );
        }

        for operator in roots.operators.iter().skip(frontier.operators.len()) {
            tables.insert_operator(&transplant, operator);
        }

        for measure in roots.measures.iter().skip(frontier.measures.len()) {
            if let Some(parameter) = &measure.parameter {
                tables.insert_type_reference(&transplant, &parameter.type_reference);
            }
            if let Some(return_type) = &measure.return_type {
                tables.insert_type_reference(&transplant, return_type);
            }
        }
    }

    /// Normalize one tree statement span into table statements, in order. Both
    /// machine states and assembly blocks read their statements this way.
    fn normalize_statements(
        &mut self,
        transplant: &Transplant<'_>,
        statements: &[Statement],
    ) -> HandleSpan<StatementNode> {
        let statement_transplant = crate::statement::StatementTransplant {
            source_expressions: transplant.source_expressions,
            source_expression_table: transplant.source_expressions,
            source_child_type_references: transplant.child_type_references,
            source_constraints: transplant.type_constraints,
            source_statement_path_members: transplant.statement_path_members,
            copy_expression_handles: false,
        };
        let mut nodes = HandleSpan::empty();
        for statement in statements {
            let node = self.bodies.statements.insert_tree_from_source(
                statement,
                &statement_transplant,
                &mut self.bodies.expressions,
                &mut self.types.references,
            );
            nodes.push_contiguous(node);
        }
        nodes
    }

    fn insert_parameter_and_return_types<'tree>(
        &mut self,
        transplant: &Transplant<'_>,
        parameters: impl Iterator<Item = &'tree StateParameter>,
        return_type: &Option<TypeReference>,
    ) {
        for parameter in parameters {
            self.insert_type_reference(transplant, &parameter.type_reference);
        }
        if let Some(return_type) = return_type {
            self.insert_type_reference(transplant, return_type);
        }
    }

    fn insert_operator(
        &mut self,
        transplant: &Transplant<'_>,
        operator: &crate::operator::OperatorDefinition,
    ) {
        self.insert_parameter_and_return_types(
            transplant,
            transplant
                .state_parameters
                .span_or_empty(operator.parameters)
                .iter(),
            &operator.return_type,
        );
    }

    fn insert_data_definition(&mut self, transplant: &Transplant<'_>, members: &[DataMember]) {
        for member in members {
            match member {
                DataMember::Field(field) => {
                    self.insert_type_reference(transplant, &field.type_reference);
                }
                DataMember::Variant(variant) => {
                    for field in transplant
                        .data_payload_fields
                        .span_or_empty(variant.payload)
                    {
                        self.insert_type_reference(transplant, &field.type_reference);
                    }
                }
            }
        }
    }

    fn insert_platform(&mut self, transplant: &Transplant<'_>, states: &[StateSignature]) {
        for state in states {
            self.insert_state_signature(
                transplant,
                transplant.state_parameters.span_or_empty(state.parameters),
                state,
            );
        }
    }

    fn insert_machine_with_state_spans(
        &mut self,
        transplant: &Transplant<'_>,
        machine: &Machine,
        declared_type_equations: &Arena<crate::machine::DeclaredTypeEquation>,
        machine_owned_data: &Arena<OwnedData>,
        machine_state_handles: &Arena<arena::Handle<State>>,
        machine_states: &mut Arena<State>,
    ) {
        for equation in declared_type_equations.span_or_empty(machine.declared_type_equations) {
            for reference in [&equation.parameter, &equation.definition] {
                self.insert_type_reference(transplant, reference);
            }
        }
        for owned_data in machine_owned_data.span_or_empty(machine.owned_data) {
            self.insert_owned_data(transplant, owned_data);
        }

        for state in machine_state_handles
            .span_or_empty(machine.states)
            .iter()
            .copied()
        {
            self.insert_state_with_statement_span(transplant, machine_states.get_mut(state));
        }
    }

    fn insert_owned_data(&mut self, transplant: &Transplant<'_>, owned_data: &OwnedData) {
        self.insert_type_reference(transplant, &owned_data.type_reference);
    }

    fn insert_state_with_statement_span(&mut self, transplant: &Transplant<'_>, state: &mut State) {
        for parameter in transplant.state_parameters.span_or_empty(state.parameters) {
            self.insert_type_reference(transplant, &parameter.type_reference);
        }

        if let Some(return_type) = &state.return_type {
            self.insert_type_reference(transplant, return_type);
        }

        state.statement_nodes = self.normalize_statements(
            transplant,
            transplant.state_statements.span_or_empty(state.statements),
        );
    }

    fn insert_state_signature(
        &mut self,
        transplant: &Transplant<'_>,
        parameters: &[StateParameter],
        signature: &StateSignature,
    ) {
        self.insert_parameter_and_return_types(
            transplant,
            parameters.iter(),
            &signature.return_type,
        );
    }

    fn insert_type_reference(
        &mut self,
        transplant: &Transplant<'_>,
        type_reference: &TypeReference,
    ) {
        self.types.references.insert_tree_from_source(
            type_reference,
            &mut self.bodies.expressions,
            transplant.child_type_references,
            transplant.type_constraints,
            transplant.source_expressions,
        );
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn rebuild_tables_collects_typed_program_payloads() {
        let (mut program, states) = crate::machine::test_program_with_guarded_transition();
        program.machines.push(crate::machine::test_machine(states));

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
