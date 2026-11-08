/*
We own the arena storage tables (`storage::tables`) and revision-tracked symbol
lookup indexes (`lookup_indexes::ResolvedIndexes`) for `SymbolResolvedTrees`.
*/

pub mod lookup_indexes {
    use std::sync::Mutex;

    use crate::{signature, statement};
    use arena::{Handle, OwnedIndex};
    use compiler_storage::{RecordError, RecordReader, RecordWriter};
    use symbols::{SymbolHandle, SymbolKeyMap};

    type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;

    #[derive(Clone, Default)]
    pub struct GenericOriginIndex {
        pub origins_by_symbol:
            SymbolKeyMap<SymbolHandle, Vec<Handle<crate::types::GenericApplicationOrigin>>>,
        pub definitions_by_symbol: SymbolKeyMap<SymbolHandle, Vec<usize>>,
        pub malformed: bool,
    }

    #[derive(Clone, Default)]
    struct PrefixPositions {
        watermark: usize,
        samples: [usize; 3],
        by_symbol: SymbolKeyMap<SymbolHandle, u32>,
    }

    #[derive(Clone, Default)]
    struct PrefixPositionMaps {
        by_start: HashMap<usize, PrefixPositions>,
    }

    impl PrefixPositionMaps {
        fn position<S>(
            &mut self,
            rows: &[S],
            symbol: SymbolHandle,
            sample: impl Fn(&S) -> usize,
            key: impl Fn(&S) -> Option<SymbolHandle>,
        ) -> Option<u32> {
            let start = rows.as_ptr() as usize;
            let length = rows.len();
            let samples = |watermark: usize| {
                [
                    sample(&rows[0]),
                    sample(&rows[watermark / 2]),
                    sample(&rows[watermark - 1]),
                ]
            };
            let fresh = self.by_start.get(&start).is_some_and(|entry| {
                entry.watermark <= length
                    && (entry.watermark == 0 || samples(entry.watermark) == entry.samples)
            });
            if !fresh {
                self.by_start.insert(start, PrefixPositions::default());
            }
            let entry = self.by_start.get_mut(&start).expect("entry just populated");
            for (position, row) in rows.iter().enumerate().skip(entry.watermark) {
                if let Some(row_symbol) = key(row) {
                    entry.by_symbol.entry(row_symbol).or_insert(position as u32);
                }
            }
            entry.watermark = length;
            entry.samples = if length == 0 { [0; 3] } else { samples(length) };
            entry.by_symbol.get(&symbol).copied()
        }
    }

    #[derive(Default)]
    pub struct ResolvedIndexes {
        generic_origins: OwnedIndex<GenericOriginIndex>,
        data_positions: OwnedIndex<SymbolKeyMap<SymbolHandle, u32>>,
        statements: Mutex<PrefixPositionMaps>,
        parameters: Mutex<PrefixPositionMaps>,
    }

    impl Clone for ResolvedIndexes {
        fn clone(&self) -> Self {
            let clone_maps = |maps: &Mutex<PrefixPositionMaps>| {
                Mutex::new(
                    maps.lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .clone(),
                )
            };
            Self {
                generic_origins: self.generic_origins.clone(),
                data_positions: self.data_positions.clone(),
                statements: clone_maps(&self.statements),
                parameters: clone_maps(&self.parameters),
            }
        }
    }

    impl std::fmt::Debug for ResolvedIndexes {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("ResolvedIndexes")
        }
    }

    impl PartialEq for ResolvedIndexes {
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }

    impl Eq for ResolvedIndexes {}

    fn statement_sample(statement: &statement::Statement) -> usize {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::mem::discriminant(statement).hash(&mut hasher);
        if let statement::Statement::LocalData(local) = statement {
            local.symbol.arena_index().hash(&mut hasher);
        }
        hasher.finish() as usize
    }

    impl ResolvedIndexes {
        pub fn with_generic_origin_index<R>(
            &self,
            revision: u64,
            build: impl FnOnce(&mut GenericOriginIndex),
            run: impl FnOnce(&GenericOriginIndex) -> R,
        ) -> R {
            self.generic_origins.with_mut(
                revision,
                || {
                    let mut index = GenericOriginIndex::default();
                    build(&mut index);
                    index
                },
                |index| run(index),
            )
        }

        pub fn data_definition_position(
            &self,
            definitions: &arena::OrderedRootArena<crate::data::DataDefinition>,
            symbol: SymbolHandle,
        ) -> Option<u32> {
            self.data_positions.with(
                definitions.revision(),
                || {
                    let mut positions = SymbolKeyMap::default();
                    for (position, definition) in definitions.iter().enumerate() {
                        positions
                            .entry(definition.symbol)
                            .or_insert(position as u32);
                    }
                    positions
                },
                |positions| positions.get(&symbol).copied(),
            )
        }

        pub fn local_statement_position(
            &self,
            statements: &[statement::Statement],
            symbol: SymbolHandle,
        ) -> Option<u32> {
            self.statements
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .position(
                    statements,
                    symbol,
                    statement_sample,
                    |statement| match statement {
                        statement::Statement::LocalData(local) => Some(local.symbol),
                        _ => None,
                    },
                )
        }

        pub fn parameter_position(
            &self,
            parameters: &[signature::StateParameter],
            symbol: SymbolHandle,
        ) -> Option<u32> {
            self.parameters
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .position(
                    parameters,
                    symbol,
                    |parameter| parameter.symbol.arena_index() as usize,
                    |parameter| Some(parameter.symbol),
                )
        }
    }

    impl crate::SymbolResolvedTrees {
        pub fn with_generic_origin_index<R>(
            &self,
            build: impl FnOnce(&mut GenericOriginIndex),
            run: impl FnOnce(&GenericOriginIndex) -> R,
        ) -> R {
            let revision = self
                .tables
                .types
                .generic_application_origins
                .revision()
                .wrapping_add(self.declarations().child_type_references.revision())
                .wrapping_add(self.roots.data_definitions.revision());
            self.resolution_indexes
                .with_generic_origin_index(revision, build, run)
        }
    }

    impl compiler_storage::Record for ResolvedIndexes {
        fn write(&self, _: &mut RecordWriter) -> Result<(), RecordError> {
            Ok(())
        }
        fn read<Context>(_: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
            Ok(Self::default())
        }
    }
}

pub mod storage {
    pub mod tables {
        use crate::data::{DataField, DataMember};
        use crate::expression::{ExpressionSource, ExpressionTable};
        use crate::machine::{Machine, OwnedData};
        use crate::name::DiagnosticName;
        use crate::operator::OperatorDefinition;
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

            pub fn from_body_completion(
                symbol_resolved_trees: &mut SymbolResolvedTrees,
                predecessor: &SymbolResolvedTrees,
                completed_machines: &[symbols::SymbolHandle],
            ) -> Self {
                let mut tables = Self {
                    bodies: ResolvedBodyTables {
                        expressions: ExpressionTable::default(),
                        statements: std::mem::take(
                            &mut symbol_resolved_trees.tables.bodies.statements,
                        ),
                    },
                    types: ResolvedTypeTables {
                        references: std::mem::take(
                            &mut symbol_resolved_trees.tables.types.references,
                        ),
                    },
                };
                tables.append_declaration_tables(
                    symbol_resolved_trees,
                    &predecessor.roots,
                    completed_machines,
                    predecessor.declarations().assembly_blocks.len(),
                );
                tables
            }

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
                    for operator in operator_definitions.span_or_empty(domain_definition.operators)
                    {
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
                    if let crate::proposition::PropositionBody::Witness { evidence } =
                        &proposition.body
                    {
                        tables.insert_type_reference(&transplant, evidence);
                    }
                }

                for trait_definition in roots.traits.iter().skip(frontier.traits.len()) {
                    tables.insert_platform(
                        &transplant,
                        trait_machine_signatures.span_or_empty(trait_definition.machines),
                    );
                }

                let mut completed = completed_machines
                    .iter()
                    .map(|symbol| (symbol.arena_index(), symbol.generation()))
                    .collect::<Vec<_>>();
                completed.sort_unstable();
                for (_, machine) in
                    roots
                        .machines
                        .iter()
                        .enumerate()
                        .filter(|(ordinal, machine)| {
                            *ordinal >= frontier.machines.len()
                                || completed
                                    .binary_search(&(
                                        machine.symbol.arena_index(),
                                        machine.symbol.generation(),
                                    ))
                                    .is_ok()
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
                operator: &OperatorDefinition,
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

            fn insert_data_definition(
                &mut self,
                transplant: &Transplant<'_>,
                members: &[DataMember],
            ) {
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
                for equation in
                    declared_type_equations.span_or_empty(machine.declared_type_equations)
                {
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
                    self.insert_state_with_statement_span(
                        transplant,
                        machine_states.get_mut(state),
                    );
                }
            }

            fn insert_owned_data(&mut self, transplant: &Transplant<'_>, owned_data: &OwnedData) {
                self.insert_type_reference(transplant, &owned_data.type_reference);
            }

            fn insert_state_with_statement_span(
                &mut self,
                transplant: &Transplant<'_>,
                state: &mut State,
            ) {
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
    }
}
