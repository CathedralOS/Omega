/*
We own the typed declaration storage tables (`declaration_tables`,
`trait_and_machine_tables`, `data_definitions`, `machine_declarations`,
`spelled_declarations`, `type_parameters`, `table_reads`), relationship indexes
(`declaration_index`), parameter signature queries (`machine_parameter_signatures`),
and type-reference queries (`type_reference_queries`) on `TypedTrees`.
*/

pub(crate) mod data_definitions {
    use crate::data::DataDefinition;
    use crate::declaration_index::SymbolRows;
    use arena::{Arena, Handle, HandleSpan, OwnedIndex};
    use symbols::SymbolHandle;

    type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;

    #[cfg(test)]
    pub(crate) mod tests {
        use super::*;
        use crate::name::Identifier;
        use crate::{FrozenTypedTrees, TypedTrees};
        use arena::Handle;

        fn definition(symbol: SymbolHandle, name: &str) -> DataDefinition {
            DataDefinition {
                symbol,
                name: Identifier::generated(name),
                ..DataDefinition::default()
            }
        }

        fn assert_fresh_rows(program: &TypedTrees, symbols: &[SymbolHandle], names: &[&str]) {
            let definitions = program.data_definitions();
            for &symbol in symbols {
                let positions: Vec<_> = definitions
                    .iter()
                    .enumerate()
                    .filter(|(_, definition)| symbol.is_valid() && definition.symbol == symbol)
                    .map(|(position, _)| position as u32)
                    .collect();
                let first = positions.first().map(|position| *position as usize);
                let unique = match positions.as_slice() {
                    [position] => Some(&definitions[*position as usize]),
                    _ => None,
                };
                assert_eq!(
                    program.data_definition_positions_for_symbol(symbol),
                    positions
                );
                program.with_data_definition_index(|index| {
                    assert_eq!(index.positions_for_symbol(symbol), positions.as_slice());
                });
                assert_eq!(program.data_definition_position_by_symbol(symbol), first);
                assert_eq!(
                    program.data_definition_by_symbol(symbol),
                    first.map(|position| &definitions[position])
                );
                assert_eq!(program.unique_data_definition_by_symbol(symbol), unique);
            }
            for &name in names {
                let positions: Vec<_> = definitions
                    .iter()
                    .enumerate()
                    .filter(|(_, definition)| definition.name.as_str() == name)
                    .map(|(position, _)| position as u32)
                    .collect();
                assert_eq!(program.data_definition_positions_for_name(name), positions);
            }
        }

        #[test]
        fn data_definition_rows_preserve_unique_and_first_match_queries() {
            let original = SymbolHandle::from_parts(1, 1);
            let next_generation = SymbolHandle::from_parts(1, 2);
            let other = SymbolHandle::from_parts(2, 1);
            let missing = SymbolHandle::from_parts(1, 3);
            let invalid = SymbolHandle::invalid();
            let symbols = [original, next_generation, other, missing, invalid];
            let names = ["Shared", "Replacement", "Duplicate", "Distinct", "Missing"];
            let mut program = TypedTrees::default();
            assert_fresh_rows(&program, &symbols, &names);
            for (symbol, name) in [
                (original, "Shared"),
                (next_generation, "Replacement"),
                (original, "Duplicate"),
                (invalid, "Shared"),
                (other, "Distinct"),
            ] {
                program.push_data_definition(definition(symbol, name));
                assert_fresh_rows(&program, &symbols, &names);
            }
            assert_eq!(
                program.data_definition_positions_for_symbol(original),
                [0, 2]
            );
            assert_eq!(program.data_definition_positions_for_name("Shared"), [0, 3]);
            assert_eq!(
                program
                    .data_definition_by_symbol(original)
                    .unwrap()
                    .name
                    .as_str(),
                "Shared"
            );
            assert!(program.unique_data_definition_by_symbol(original).is_none());
            assert!(
                program
                    .unique_data_definition_by_symbol(next_generation)
                    .is_some()
            );
        }

        #[test]
        fn data_definition_rows_follow_storage_and_roster_changes() {
            let original = SymbolHandle::from_parts(1, 1);
            let replacement = SymbolHandle::from_parts(2, 1);
            let other = SymbolHandle::from_parts(3, 1);
            let symbols = [original, replacement, other, SymbolHandle::invalid()];
            let names = ["First", "Second", "Replacement"];
            let mut future = TypedTrees::default();
            let first = future
                .tables
                .data_definitions
                .insert(definition(original, "First"));
            let next = Handle::from_parts(first.arena_index() + 1, first.generation());
            future.roots.data_definitions = HandleSpan::from_parts(next, 1);
            assert_fresh_rows(&future, &symbols, &names);
            assert_eq!(
                future
                    .tables
                    .data_definitions
                    .insert(definition(replacement, "Replacement")),
                next
            );
            assert_fresh_rows(&future, &symbols, &names);

            let mut program = TypedTrees::default();
            program.push_data_definition(definition(original, "First"));
            program.push_data_definition(definition(other, "Second"));
            let roster = program.roots.data_definitions;
            assert_fresh_rows(&program, &symbols, &names);

            let revision = program.tables.data_definitions.revision();
            program
                .tables
                .data_definitions
                .get_mut(roster.start())
                .symbol = replacement;
            assert_eq!(program.tables.data_definitions.revision(), revision + 1);
            program.roots.data_definitions = HandleSpan::from_parts(roster.start(), 1);
            assert_fresh_rows(&program, &symbols, &names);
            program.roots.data_definitions = roster;
            assert_fresh_rows(&program, &symbols, &names);

            program.roots.data_definitions = HandleSpan::from_parts(
                Handle::from_parts(
                    roster.start().arena_index(),
                    roster.start().generation() + 1,
                ),
                roster.count(),
            );
            assert!(program.data_definitions().is_empty());
            assert_fresh_rows(&program, &symbols, &names);
            program.roots.data_definitions = roster;
            assert_fresh_rows(&program, &symbols, &names);

            let mut snapshot = program.clone();
            snapshot
                .tables
                .data_definitions
                .get_mut(roster.start())
                .symbol = original;
            assert_fresh_rows(&snapshot, &symbols, &names);
            assert_fresh_rows(&program, &symbols, &names);
            assert!(program.unique_data_definition_by_symbol(original).is_none());
            assert!(
                snapshot
                    .unique_data_definition_by_symbol(original)
                    .is_some()
            );

            let frozen = FrozenTypedTrees::from(program);
            assert_fresh_rows(&frozen, &symbols, &names);
            let mut program = frozen.into_inner();
            program
                .tables
                .data_definitions
                .get_mut(roster.start())
                .symbol = original;
            assert_fresh_rows(&program, &symbols, &names);
            assert_fresh_rows(&FrozenTypedTrees::from(program), &symbols, &names);

            let mut program = TypedTrees::default();
            program.push_data_definition(definition(original, "First"));
            assert_fresh_rows(&program, &symbols, &names);
            let mut incoming = Arena::default();
            let mut incoming_roster = HandleSpan::empty();
            let incoming_handle = incoming
                .append_to_span(&mut incoming_roster, definition(replacement, "Replacement"));
            assert_eq!(
                incoming.revision(),
                program.tables.data_definitions.revision()
            );
            assert_eq!(incoming_handle, program.roots.data_definitions.start());
            *program.tables.data_definitions = incoming;
            assert_fresh_rows(&program, &symbols, &names);

            let mut source = TypedTrees::default();
            source.push_data_definition(definition(other, "Second"));
            assert_fresh_rows(&source, &symbols, &names);
            program.tables.data_definitions = source.tables.data_definitions.clone();
            program.roots.data_definitions = source.roots.data_definitions;
            assert_fresh_rows(&program, &symbols, &names);
            program.tables = snapshot.tables.clone();
            program.roots = snapshot.roots.clone();
            assert_fresh_rows(&program, &symbols, &names);
            assert_fresh_rows(&source, &symbols, &names);
        }
    }

    #[derive(Clone, Default)]
    pub struct DataDefinitionIndex {
        by_symbol: SymbolRows<u32>,
        by_name: HashMap<Box<str>, Vec<u32>>,
    }

    impl DataDefinitionIndex {
        pub fn positions_for_symbol(&self, symbol: SymbolHandle) -> &[u32] {
            self.by_symbol.get(symbol)
        }

        pub(crate) fn first_position(&self, symbol: SymbolHandle) -> Option<usize> {
            self.by_symbol
                .first(symbol)
                .map(|position| *position as usize)
        }

        pub(crate) fn positions_for_name(&self, name: &str) -> &[u32] {
            self.by_name.get(name).map(Vec::as_slice).unwrap_or(&[])
        }

        fn build(definitions: &[DataDefinition]) -> Self {
            let mut by_symbol = Vec::with_capacity(definitions.len());
            let mut by_name: HashMap<Box<str>, Vec<u32>> = HashMap::default();
            for (position, definition) in definitions.iter().enumerate() {
                let position = position as u32;
                if definition.symbol.is_valid() {
                    by_symbol.push((definition.symbol, position));
                }
                by_name
                    .entry(definition.name.as_str().into())
                    .or_default()
                    .push(position);
            }
            Self {
                by_symbol: SymbolRows::new(by_symbol),
                by_name,
            }
        }
    }

    #[derive(Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct DataDefinitionTable {
        arena: Arena<DataDefinition>,
        index: OwnedIndex<DataDefinitionIndex, (HandleSpan<DataDefinition>, usize)>,
    }

    impl From<Arena<DataDefinition>> for DataDefinitionTable {
        fn from(arena: Arena<DataDefinition>) -> Self {
            Self {
                arena,
                index: OwnedIndex::default(),
            }
        }
    }

    impl std::fmt::Debug for DataDefinitionTable {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.arena, formatter)
        }
    }

    impl std::ops::Deref for DataDefinitionTable {
        type Target = Arena<DataDefinition>;

        fn deref(&self) -> &Self::Target {
            &self.arena
        }
    }

    impl std::ops::DerefMut for DataDefinitionTable {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.index.clear();
            &mut self.arena
        }
    }

    impl DataDefinitionTable {
        pub fn insert(&mut self, definition: DataDefinition) -> Handle<DataDefinition> {
            let before = self.arena.append_epoch();
            let handle = self.arena.insert(definition);
            let appended = before.zip(self.arena.append_epoch()).is_some_and(
                |((before_epoch, before_len), (after_epoch, after_len))| {
                    before_epoch == after_epoch && before_len.checked_add(1) == Some(after_len)
                },
            );
            if !appended {
                self.index.clear();
            }
            handle
        }

        pub(crate) fn with_index<Result>(
            &self,
            roster: HandleSpan<DataDefinition>,
            read: impl FnOnce(&DataDefinitionIndex) -> Result,
        ) -> Result {
            let definitions = self.arena.span_or_empty(roster);
            self.index.with(
                (roster, definitions.len()),
                || DataDefinitionIndex::build(definitions),
                read,
            )
        }

        pub(crate) fn seal(&mut self) {
            self.index.seal_with(|index| index.by_symbol.densify());
        }

        pub(crate) fn unseal(&mut self) {
            self.index.unseal();
        }
    }
}

pub mod declaration_index {
    use crate::machine_declarations::MachinePositionIndex;
    use crate::type_parameters::TypeParameterIndex;
    use crate::{data, machine};
    use arena::{Handle, HandleSpan, OwnedIndex};
    use symbols::SymbolHandle;

    pub(crate) mod operator_spellings {
        use super::HashMap;
        use crate::TypedTrees;
        use crate::domain::DomainDefinition;
        use crate::operator::OperatorDefinition;
        use crate::spelled_declarations::DeclarationStorageIdentity;
        use arena::{Handle, HandleSpan};
        use language_core::OperatorSpelling;
        use std::sync::Arc;

        pub(crate) type DomainDeclarationScope =
            (DeclarationStorageIdentity, HandleSpan<DomainDefinition>);

        #[derive(Clone, PartialEq, Eq)]
        pub(crate) struct OperatorSpellingScope {
            operators: DeclarationStorageIdentity,
            domains: DeclarationStorageIdentity,
            root_operators: HandleSpan<OperatorDefinition>,
            machine_bindings: HandleSpan<OperatorDefinition>,
            root_domains: HandleSpan<DomainDefinition>,
        }

        #[derive(Clone, Copy)]
        pub(crate) enum OperatorSpellingRow {
            Root(Handle<OperatorDefinition>),
            Domain {
                operator: Handle<OperatorDefinition>,
                domain: Handle<DomainDefinition>,
            },
        }

        #[derive(Clone, Default)]
        pub(crate) struct OperatorSpellingIndex {
            rows: HashMap<OperatorSpelling, Arc<[OperatorSpellingRow]>>,
        }

        impl TypedTrees {
            pub(crate) fn domain_declaration_scope(&self) -> DomainDeclarationScope {
                (
                    self.tables.domain_definitions.storage_identity(),
                    self.roots.domain_definitions,
                )
            }

            pub(crate) fn with_operator_spelling_index<Result>(
                &self,
                read: impl FnOnce(&OperatorSpellingIndex) -> Result,
            ) -> Result {
                let scope = OperatorSpellingScope {
                    operators: self.tables.operators.storage_identity(),
                    domains: self.tables.domain_definitions.storage_identity(),
                    root_operators: self.roots.operators,
                    machine_bindings: self.roots.machine_token_bindings,
                    root_domains: self.roots.domain_definitions,
                };
                self.declaration_indexes.operator_spellings.with(
                    scope,
                    || {
                        let mut rows: HashMap<OperatorSpelling, Vec<OperatorSpellingRow>> =
                            HashMap::default();
                        for span in [self.roots.operators, self.roots.machine_token_bindings] {
                            for (position, operator) in
                                self.tables.operators.span_or_empty(span).iter().enumerate()
                            {
                                if let Some(spelling) = operator.spelling {
                                    rows.entry(spelling).or_default().push(
                                        OperatorSpellingRow::Root(span.handle_at(position as u32)),
                                    );
                                }
                            }
                        }
                        for (position, domain) in self.domain_definitions().iter().enumerate() {
                            let domain_handle =
                                self.roots.domain_definitions.handle_at(position as u32);
                            for (position, operator) in
                                self.domain_operators(domain).iter().enumerate()
                            {
                                if let Some(spelling) = operator.spelling {
                                    rows.entry(spelling).or_default().push(
                                        OperatorSpellingRow::Domain {
                                            operator: domain.operators.handle_at(position as u32),
                                            domain: domain_handle,
                                        },
                                    );
                                }
                            }
                        }
                        OperatorSpellingIndex {
                            rows: rows
                                .into_iter()
                                .map(|(spelling, rows)| (spelling, Arc::from(rows)))
                                .collect(),
                        }
                    },
                    read,
                )
            }

            pub(crate) fn operator_spelling_rows(
                &self,
                spelling: OperatorSpelling,
            ) -> Option<Arc<[OperatorSpellingRow]>> {
                self.with_operator_spelling_index(|index| index.rows.get(&spelling).cloned())
            }
        }
    }
    pub(crate) use operator_spellings::OperatorSpellingRow;
    use operator_spellings::{
        DomainDeclarationScope, OperatorSpellingIndex, OperatorSpellingScope,
    };
    pub(crate) mod relationships {

        use super::roster_marker;
        use crate::{data::DataMember, signature::StateParameter, statement::StatementNode};
        use symbols::SymbolHandle;

        #[derive(Clone)]
        pub(crate) struct SymbolRows<Position> {
            symbols: Vec<SymbolHandle>,
            positions: Vec<Position>,
            starts: Vec<u32>,
        }

        impl<Position> Default for SymbolRows<Position> {
            fn default() -> Self {
                Self {
                    symbols: Vec::new(),
                    positions: Vec::new(),
                    starts: Vec::new(),
                }
            }
        }

        impl<Position> SymbolRows<Position> {
            pub(crate) fn new(mut rows: Vec<(SymbolHandle, Position)>) -> Self {
                rows.sort_by_key(|(symbol, _)| (symbol.arena_index(), symbol.generation()));
                let (symbols, positions) = rows.into_iter().unzip();
                Self {
                    symbols,
                    positions,
                    starts: Vec::new(),
                }
            }

            pub(crate) fn densify(&mut self) {
                let Some(last) = self.symbols.last() else {
                    return;
                };
                let mut starts = vec![0u32; last.arena_index() as usize + 2];
                for symbol in &self.symbols {
                    starts[symbol.arena_index() as usize + 1] += 1;
                }
                for index in 1..starts.len() {
                    starts[index] += starts[index - 1];
                }
                self.starts = starts;
            }

            pub(crate) fn get(&self, symbol: SymbolHandle) -> &[Position] {
                let index = symbol.arena_index();
                let (start, end) = if self.starts.is_empty() {
                    (
                        self.symbols
                            .partition_point(|row| row.arena_index() < index),
                        self.symbols
                            .partition_point(|row| row.arena_index() <= index),
                    )
                } else {
                    let index = index as usize;
                    let (Some(&start), Some(&end)) =
                        (self.starts.get(index), self.starts.get(index + 1))
                    else {
                        return &[];
                    };
                    (start as usize, end as usize)
                };
                let same_index = &self.symbols[start..end];
                let generation = symbol.generation();
                let low = start + same_index.partition_point(|row| row.generation() < generation);
                let high = start + same_index.partition_point(|row| row.generation() <= generation);
                &self.positions[low..high]
            }

            pub(crate) fn first(&self, symbol: SymbolHandle) -> Option<&Position> {
                self.get(symbol).first()
            }
        }

        pub(crate) type StateDeclarationRevision = (
            u64,
            u64,
            u64,
            u64,
            u64,
            u64,
            arena::HandleSpan<crate::machine::Machine>,
            arena::HandleSpan<crate::proposition::PropositionDefinition>,
        );

        #[derive(Clone, Default)]
        pub(crate) struct StateDeclarationIndex {
            locals: SymbolRows<(u32, u32, u32)>,
            states: SymbolRows<(u32, u32)>,
            parameter_types: SymbolRows<crate::types::TypeReferenceHandle>,
            value_types: SymbolRows<(usize, crate::types::TypeReferenceHandle)>,
        }

        #[derive(Clone, Default)]
        pub(crate) struct DataMemberIndex {
            fields: SymbolRows<(u32, u32, u32)>,
            variants: SymbolRows<(u32, u32)>,
            variant_names: Vec<(Box<str>, (u32, u32))>,
        }

        impl StateDeclarationIndex {
            pub(crate) fn densify(&mut self) {
                self.locals.densify();
                self.states.densify();
                self.parameter_types.densify();
                self.value_types.densify();
            }
        }

        impl DataMemberIndex {
            pub(crate) fn densify(&mut self) {
                self.fields.densify();
                self.variants.densify();
            }
        }

        impl crate::TypedTrees {
            pub(crate) fn with_state_declarations<R>(
                &self,
                run: impl FnOnce(&StateDeclarationIndex) -> R,
            ) -> R {
                let revision = (
                    self.tables.machines.revision(),
                    self.tables.machine_states.revision(),
                    self.tables.state_parameters.revision(),
                    self.tables.machine_owned_data.revision(),
                    self.statement_table.revision(),
                    self.tables.propositions.revision(),
                    self.roots.machines,
                    self.roots.propositions,
                );
                self.declaration_indexes.state_declarations.with(
                    revision,
                    || {
                        let (mut locals, mut states, mut parameter_types, mut value_types) =
                            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
                        for (_, parameter) in self.tables.state_parameters.iter() {
                            parameter_types.push((parameter.symbol, parameter.type_reference));
                        }
                        for (machine_position, machine) in self.machines().iter().enumerate() {
                            for owned in self.machine_owned_data(machine) {
                                let order = value_types.len();
                                value_types.push((owned.symbol, (order, owned.type_reference)));
                            }
                            for (state_position, state) in
                                self.machine_states(machine).iter().enumerate()
                            {
                                for parameter in self.state_parameters(state) {
                                    let order = value_types.len();
                                    value_types.push((
                                        parameter.symbol,
                                        (order, parameter.type_reference),
                                    ));
                                }
                                let position = (machine_position as u32, state_position as u32);
                                let first = self
                                    .state_parameters(state)
                                    .first()
                                    .map_or(SymbolHandle::invalid(), |parameter| parameter.symbol);
                                states.push((first, position));
                                for (statement_position, statement) in self
                                    .statement_table
                                    .statements(state.statement_nodes)
                                    .iter()
                                    .enumerate()
                                {
                                    if let StatementNode::LocalData(local) = statement {
                                        let order = value_types.len();
                                        value_types
                                            .push((local.symbol, (order, local.type_reference)));
                                        locals.push((
                                            local.symbol,
                                            (position.0, position.1, statement_position as u32),
                                        ));
                                    }
                                }
                            }
                        }
                        for proposition in self.propositions() {
                            for parameter in self.proposition_parameters(proposition) {
                                let order = value_types.len();
                                value_types
                                    .push((parameter.symbol, (order, parameter.type_reference)));
                            }
                        }
                        StateDeclarationIndex {
                            locals: SymbolRows::new(locals),
                            states: SymbolRows::new(states),
                            parameter_types: SymbolRows::new(parameter_types),
                            value_types: SymbolRows::new(value_types),
                        }
                    },
                    run,
                )
            }

            pub fn first_named_value_declared_type(
                &self,
                symbol: SymbolHandle,
                head_symbol: SymbolHandle,
            ) -> Option<crate::types::TypeReferenceHandle> {
                self.with_state_declarations(|index| {
                    if symbol.is_valid()
                        && symbol == head_symbol
                        && let Some(type_reference) = index.parameter_types.first(symbol)
                    {
                        return Some(*type_reference);
                    }
                    [symbol, head_symbol]
                        .into_iter()
                        .filter(|symbol| symbol.is_valid())
                        .filter_map(|symbol| index.value_types.first(symbol).copied())
                        .min_by_key(|(order, _)| *order)
                        .map(|(_, type_reference)| type_reference)
                })
            }

            pub fn local_data_positions_for_symbol(
                &self,
                symbol: SymbolHandle,
            ) -> Vec<(u32, u32, u32)> {
                self.with_state_declarations(|index| index.locals.get(symbol).to_vec())
            }

            /// One exact local declaration, without copying its indexed position roster.
            /// A duplicate binder cannot choose a component contract for another local.
            pub fn unique_local_data_for_symbol(
                &self,
                symbol: SymbolHandle,
            ) -> Option<&crate::statement::TableLocalData> {
                let (machine, state, statement) = self.with_state_declarations(|index| {
                    let [position] = index.locals.get(symbol) else {
                        return None;
                    };
                    Some(*position)
                })?;
                let machine = self.machines().get(machine as usize)?;
                let state = self.machine_states(machine).get(state as usize)?;
                self.statement_table
                    .statements(state.statement_nodes)
                    .get(statement as usize)?
                    .as_local_data()
            }

            /// The first state with this exact binder sequence. Empty spans and forged
            /// duplicate symbols retain the same first-match rule as a source roster.
            pub fn state_position_for_parameter_symbols(
                &self,
                parameters: &[StateParameter],
            ) -> Option<(u32, u32)> {
                let first = parameters
                    .first()
                    .map_or(SymbolHandle::invalid(), |parameter| parameter.symbol);
                self.with_state_declarations(|index| {
                    index.states.get(first).iter().copied().find(
                        |&(machine_position, state_position)| {
                            let machine = &self.machines()[machine_position as usize];
                            let state = &self.machine_states(machine)[state_position as usize];
                            let authored = self.state_parameters(state);
                            authored.len() == parameters.len()
                                && authored
                                    .iter()
                                    .zip(parameters)
                                    .all(|(left, right)| left.symbol == right.symbol)
                        },
                    )
                })
            }

            pub(crate) fn with_data_members<R>(
                &self,
                run: impl FnOnce(&DataMemberIndex) -> R,
            ) -> R {
                let revision = self
                    .tables
                    .data_definitions
                    .revision()
                    .wrapping_add(self.tables.data_members.revision())
                    .wrapping_add(self.tables.data_payload_fields.revision())
                    .wrapping_add(roster_marker(self.roots.data_definitions));
                self.declaration_indexes.data_members.with(
                    revision,
                    || {
                        let (mut fields, mut variants) = (Vec::new(), Vec::new());
                        let mut variant_names: Vec<(Box<str>, (u32, u32))> = Vec::new();
                        for (data_position, definition) in
                            self.data_definitions().iter().enumerate()
                        {
                            for (member_position, member) in
                                self.data_members(definition).iter().enumerate()
                            {
                                let position = (data_position as u32, member_position as u32);
                                match member {
                                    DataMember::Field(field) => fields
                                        .push((field.symbol, (position.0, position.1, u32::MAX))),
                                    DataMember::Variant(variant) => {
                                        variants.push((variant.symbol, position));
                                        variant_names
                                            .push((variant.name.as_str().into(), position));
                                        for (payload_position, field) in
                                            self.data_payload_fields(variant).iter().enumerate()
                                        {
                                            fields.push((
                                                field.symbol,
                                                (position.0, position.1, payload_position as u32),
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                        variant_names.sort_by(|left, right| left.0.cmp(&right.0));
                        DataMemberIndex {
                            fields: SymbolRows::new(fields),
                            variants: SymbolRows::new(variants),
                            variant_names,
                        }
                    },
                    run,
                )
            }

            pub fn data_field_positions_for_symbol(
                &self,
                symbol: SymbolHandle,
            ) -> Vec<(u32, u32, u32)> {
                self.with_data_members(|index| index.fields.get(symbol).to_vec())
            }

            pub fn data_variant_position_for_symbol(
                &self,
                symbol: SymbolHandle,
            ) -> Option<(u32, u32)> {
                self.with_data_members(|index| index.variants.first(symbol).copied())
            }

            pub fn data_variant_positions_for_name(&self, name: &str) -> Vec<(u32, u32)> {
                self.with_data_members(|index| {
                    let start = index
                        .variant_names
                        .partition_point(|(candidate, _)| candidate.as_ref() < name);
                    let end = index
                        .variant_names
                        .partition_point(|(candidate, _)| candidate.as_ref() <= name);
                    index.variant_names[start..end]
                        .iter()
                        .map(|(_, position)| *position)
                        .collect()
                })
            }
        }

        #[cfg(test)]
        mod tests {
            use super::SymbolRows;
            use symbols::SymbolHandle;

            #[test]
            fn symbol_rows_preserve_generations_and_duplicate_roster_order() {
                let positions = [0, 1, 2, 7, 8];
                let generations = [0, 1, 2, u32::MAX / 2, u32::MAX / 2 + 1, u32::MAX];
                let symbols = positions
                    .into_iter()
                    .flat_map(|position| {
                        generations
                            .into_iter()
                            .map(move |generation| SymbolHandle::from_parts(position, generation))
                    })
                    .collect::<Vec<_>>();
                let authored = symbols
                    .iter()
                    .rev()
                    .chain(symbols.iter())
                    .copied()
                    .enumerate()
                    .map(|(position, symbol)| (symbol, position))
                    .collect::<Vec<_>>();
                let sorted = SymbolRows::new(authored.clone());
                let mut dense = sorted.clone();
                dense.densify();
                for symbol in symbols.into_iter().chain([
                    SymbolHandle::from_parts(3, 4),
                    SymbolHandle::from_parts(1, 3),
                    SymbolHandle::from_parts(9, 0),
                    SymbolHandle::from_parts(u32::MAX, 0),
                ]) {
                    let expected = authored
                        .iter()
                        .filter(|(candidate, _)| *candidate == symbol)
                        .map(|(_, position)| *position)
                        .collect::<Vec<_>>();
                    for rows in [&sorted, &dense] {
                        assert_eq!(rows.get(symbol), expected);
                        assert_eq!(rows.first(symbol), expected.first());
                    }
                }
                let mut empty = SymbolRows::<usize>::new(Vec::new());
                empty.densify();
                assert!(empty.get(SymbolHandle::invalid()).is_empty());
                assert!(empty.first(SymbolHandle::invalid()).is_none());
            }
        }
    }
    pub(crate) use relationships::SymbolRows;
    pub(crate) mod requirement_applications {
        use super::relationships::SymbolRows;
        use crate::callable_applications::AppliedCallableSignature;
        use arena::Handle;
        use symbols::SymbolHandle;

        #[derive(Clone, Default)]
        pub(crate) struct RequirementApplicationIndex {
            rows: SymbolRows<Handle<AppliedCallableSignature>>,
            by_template: SymbolRows<Handle<AppliedCallableSignature>>,
        }

        impl RequirementApplicationIndex {
            pub(crate) fn densify(&mut self) {
                self.rows.densify();
                self.by_template.densify();
            }
        }

        impl crate::TypedTrees {
            /// Borrow one call's producer-maintained rows in their original order.
            /// Full application and caller identities select the group; the
            /// consuming query still checks every target and binding predicate.
            pub(crate) fn find_requirement_invocation(
                &self,
                application: crate::static_applications::StaticMachineApplicationHandle,
                lexical_owner: SymbolHandle,
                mut matches: impl FnMut(&AppliedCallableSignature) -> bool,
            ) -> Option<&AppliedCallableSignature> {
                // Original matching rows required a valid application. Skipping this
                // impossible group leaves the caller's outer/closed fallbacks intact.
                if !application.is_valid() {
                    return None;
                }
                self.applied_callable_signatures
                    .requirement_rows(application, lexical_owner)
                    .find_map(|(_, row)| matches(row).then_some(row))
            }

            pub(crate) fn with_requirement_application_index<R>(
                &self,
                run: impl FnOnce(&RequirementApplicationIndex) -> R,
            ) -> R {
                self.declaration_indexes.requirement_applications.with(
                    self.applied_callable_signatures.revision(),
                    || {
                        let mut rows = Vec::new();
                        let mut by_template = Vec::new();
                        for (handle, application) in self.applied_callable_signatures.iter() {
                            if let Some(symbol) = application.requirement_instance_symbol() {
                                rows.push((symbol, handle));
                            }
                            if let Some(template) = application.requirement_template_symbol() {
                                by_template.push((template, handle));
                            }
                        }
                        RequirementApplicationIndex {
                            rows: SymbolRows::new(rows),
                            by_template: SymbolRows::new(by_template),
                        }
                    },
                    run,
                )
            }

            pub(crate) fn closed_requirement_signature(
                &self,
                symbol: SymbolHandle,
            ) -> Option<&AppliedCallableSignature> {
                let handle = self.with_requirement_application_index(|index| {
                    index.rows.first(symbol).copied()
                })?;
                Some(self.applied_callable_signatures.get(handle))
            }

            pub(crate) fn closed_requirement_instance_handles_for_template(
                &self,
                template: SymbolHandle,
            ) -> Vec<Handle<AppliedCallableSignature>> {
                self.with_requirement_application_index(|index| {
                    index.by_template.get(template).to_vec()
                })
            }
        }
    }
    use crate::types::TypeReferenceHandle;
    use relationships::{DataMemberIndex, StateDeclarationIndex, StateDeclarationRevision};
    use requirement_applications::RequirementApplicationIndex;

    #[cfg(test)]
    pub(crate) mod tests {
        use crate::TypedTrees;
        use crate::data::{TypeParameter, TypeParameterKind};
        use crate::types::TypeReferenceHandle;
        use symbols::SymbolHandle;

        fn parameter(symbol: SymbolHandle, kind: TypeParameterKind) -> TypeParameter {
            TypeParameter {
                symbol,
                kind,
                ..TypeParameter::default()
            }
        }

        #[test]
        fn lexical_machine_scope_retains_header_without_executable_root() {
            let symbol = SymbolHandle::from_parts(3, 1);
            let other_generation = SymbolHandle::from_parts(3, 2);
            let mut program = TypedTrees::default();
            let retained = program.tables.machines.insert(crate::machine::Machine {
                symbol,
                ..crate::machine::Machine::default()
            });
            assert!(program.machine_by_symbol(symbol).is_none());
            assert_eq!(
                program.machine_declaration_for_lexical_scope(symbol),
                Some(program.tables.machines.get(retained))
            );
            assert!(
                program
                    .machine_declaration_for_lexical_scope(other_generation)
                    .is_none()
            );
            assert!(
                program
                    .machine_declaration_for_lexical_scope(SymbolHandle::invalid())
                    .is_none()
            );

            let duplicate = program.tables.machines.insert(crate::machine::Machine {
                symbol,
                ..crate::machine::Machine::default()
            });
            assert_eq!(
                program.machine_declaration_for_lexical_scope(symbol),
                Some(program.tables.machines.get(retained))
            );
            program
                .tables
                .machines
                .get_mut(duplicate)
                .lifetime_parameters
                .push(crate::name::Identifier::from("conflicting"));
            assert!(
                program
                    .machine_declaration_for_lexical_scope(symbol)
                    .is_none()
            );
            assert!(program.tables.machines.free(duplicate));
            assert_eq!(
                program.machine_declaration_for_lexical_scope(symbol),
                Some(program.tables.machines.get(retained))
            );
            program.tables.machines.get_mut(retained).symbol = other_generation;
            assert!(
                program
                    .machine_declaration_for_lexical_scope(symbol)
                    .is_none()
            );
            assert_eq!(
                program.machine_declaration_for_lexical_scope(other_generation),
                Some(program.tables.machines.get(retained))
            );
        }

        #[test]
        fn lexical_machine_scope_distinguishes_span_relocation_from_conflicting_headers() {
            let owner = SymbolHandle::from_parts(7, 1);
            let unrelated = SymbolHandle::from_parts(8, 1);
            let next_root = SymbolHandle::from_parts(9, 1);
            let mut program = TypedTrees::default();
            program.push_machine(crate::machine::Machine {
                symbol: owner,
                lifetime_parameters: vec![crate::name::Identifier::from("declared")],
                ..crate::machine::Machine::default()
            });
            program.tables.machines.insert(crate::machine::Machine {
                symbol: unrelated,
                ..crate::machine::Machine::default()
            });
            program.push_machine(crate::machine::Machine {
                symbol: next_root,
                ..crate::machine::Machine::default()
            });
            assert_eq!(
                program
                    .tables
                    .machines
                    .iter()
                    .filter(|(_, machine)| machine.symbol == owner)
                    .count(),
                2
            );
            program.roots.machines = arena::HandleSpan::empty();
            assert!(program.machine_by_symbol(owner).is_none());
            let header = program
                .machine_declaration_for_lexical_scope(owner)
                .unwrap();
            assert_eq!(header.lifetime_parameters[0].as_str(), "declared");
            let mut conflicting = header.clone();
            conflicting.lifetime_parameters.clear();
            program.tables.machines.insert(conflicting);
            assert!(
                program
                    .machine_declaration_for_lexical_scope(owner)
                    .is_none()
            );
        }

        #[test]
        fn lexical_machine_scope_prefers_current_header_and_tracks_projection() {
            let symbol = SymbolHandle::from_parts(5, 1);
            let mut program = TypedTrees::default();
            let retained = program.tables.machines.insert(crate::machine::Machine {
                symbol,
                name: crate::name::Identifier::from("retained"),
                ..crate::machine::Machine::default()
            });
            program.push_machine(crate::machine::Machine {
                symbol,
                name: crate::name::Identifier::from("current"),
                ..crate::machine::Machine::default()
            });
            assert_eq!(
                program.machine_declaration_for_lexical_scope(symbol),
                program.machine_by_symbol(symbol)
            );
            assert_eq!(
                program.machine_by_symbol(symbol).unwrap().name.as_str(),
                "current"
            );
            let mut projection = program.clone();
            projection.roots.machines = arena::HandleSpan::empty();
            assert!(projection.machine_by_symbol(symbol).is_none());
            assert!(
                projection
                    .machine_declaration_for_lexical_scope(symbol)
                    .is_none()
            );
            assert!(projection.tables.machines.free(retained));
            assert_eq!(
                projection
                    .machine_declaration_for_lexical_scope(symbol)
                    .unwrap()
                    .name
                    .as_str(),
                "current"
            );
            assert_eq!(
                program.machine_by_symbol(symbol).unwrap().name.as_str(),
                "current"
            );
        }

        #[test]
        fn parameter_occurrences_select_before_testing_binder_kind() {
            let symbol = SymbolHandle::from_parts(1, 1);
            let reference = TypeReferenceHandle::from_parts(1, 1);
            let mut program = TypedTrees::default();
            program
                .data_type_parameters
                .insert(parameter(symbol, TypeParameterKind::Type));
            program.data_type_parameters.insert(parameter(
                symbol,
                TypeParameterKind::Value {
                    type_reference: reference,
                },
            ));

            let first = program.first_data_type_parameter_by_symbol(symbol).unwrap();
            assert_eq!(first.kind, TypeParameterKind::Type);
            assert!(!first.kind.is_const_or_value());
            assert_eq!(
                program.data_type_parameter_by_symbol(symbol).unwrap().kind,
                TypeParameterKind::Value {
                    type_reference: reference,
                }
            );
        }

        #[test]
        fn parameter_occurrences_keep_complete_symbol_generations() {
            let original = SymbolHandle::from_parts(1, 1);
            let replacement = SymbolHandle::from_parts(1, 2);
            let mut program = TypedTrees::default();
            let first = program
                .data_type_parameters
                .insert(parameter(original, TypeParameterKind::Type));
            let other = program
                .data_type_parameters
                .insert(parameter(replacement, TypeParameterKind::Implementation));
            let last = program.data_type_parameters.insert(parameter(
                original,
                TypeParameterKind::Const {
                    type_reference: TypeReferenceHandle::from_parts(2, 1),
                },
            ));

            assert_eq!(
                program.first_data_type_parameter_by_symbol(original),
                Some(program.data_type_parameters.get(first))
            );
            assert_eq!(
                program.data_type_parameter_by_symbol(original),
                Some(program.data_type_parameters.get(last))
            );
            assert_eq!(
                program.first_data_type_parameter_by_symbol(replacement),
                Some(program.data_type_parameters.get(other))
            );
            assert_eq!(
                program.data_type_parameter_by_symbol(replacement),
                Some(program.data_type_parameters.get(other))
            );
            for absent in [SymbolHandle::from_parts(1, 3), SymbolHandle::invalid()] {
                assert!(
                    program
                        .first_data_type_parameter_by_symbol(absent)
                        .is_none()
                );
                assert!(program.data_type_parameter_by_symbol(absent).is_none());
            }
        }

        #[test]
        fn parameter_occurrences_refresh_mutation_reuse_and_cloned_owners() {
            let original = SymbolHandle::from_parts(1, 1);
            let replacement = SymbolHandle::from_parts(2, 1);
            let mut program = TypedTrees::default();
            assert!(
                program
                    .first_data_type_parameter_by_symbol(original)
                    .is_none()
            );
            assert!(program.data_type_parameter_by_symbol(original).is_none());
            let first = program
                .data_type_parameters
                .insert(parameter(original, TypeParameterKind::Type));
            assert_eq!(
                program.first_data_type_parameter_by_symbol(original),
                program.data_type_parameter_by_symbol(original)
            );
            let last = program.data_type_parameters.insert(parameter(
                original,
                TypeParameterKind::Value {
                    type_reference: TypeReferenceHandle::from_parts(1, 1),
                },
            ));
            assert_eq!(
                program.data_type_parameter_by_symbol(original),
                Some(program.data_type_parameters.get(last))
            );
            program.data_type_parameters.get_mut(first).symbol = replacement;
            assert_eq!(
                program.first_data_type_parameter_by_symbol(original),
                Some(program.data_type_parameters.get(last))
            );
            assert!(program.data_type_parameter_by_symbol(replacement).is_some());

            let mut snapshot = program.clone();
            snapshot.data_type_parameters.get_mut(first).kind = TypeParameterKind::Implementation;
            assert_eq!(
                snapshot
                    .first_data_type_parameter_by_symbol(replacement)
                    .unwrap()
                    .kind,
                TypeParameterKind::Implementation
            );
            assert_eq!(
                program
                    .first_data_type_parameter_by_symbol(replacement)
                    .unwrap()
                    .kind,
                TypeParameterKind::Type
            );

            assert!(program.data_type_parameters.free(first));
            assert!(
                program
                    .first_data_type_parameter_by_symbol(replacement)
                    .is_none()
            );
            let reused = program.data_type_parameters.insert(parameter(
                original,
                TypeParameterKind::Const {
                    type_reference: TypeReferenceHandle::from_parts(2, 1),
                },
            ));
            assert_eq!(first.arena_index(), reused.arena_index());
            assert_ne!(first.generation(), reused.generation());
            assert_eq!(
                program.first_data_type_parameter_by_symbol(original),
                Some(program.data_type_parameters.get(reused))
            );
            assert_eq!(
                program.data_type_parameter_by_symbol(original),
                Some(program.data_type_parameters.get(last))
            );
            assert!(
                snapshot
                    .first_data_type_parameter_by_symbol(replacement)
                    .is_some()
            );
        }

        #[test]
        fn named_value_types_refresh_equal_revision_parameter_replacement() {
            let symbol = SymbolHandle::from_parts(7, 3);
            let first_type = TypeReferenceHandle::from_parts(1, 1);
            let replacement_type = TypeReferenceHandle::from_parts(2, 1);
            let parameter = |type_reference| crate::signature::StateParameter {
                symbol,
                type_reference,
                ..crate::signature::StateParameter::default()
            };
            let mut program = TypedTrees::default();
            let original_handle = program.state_parameters.insert(parameter(first_type));
            assert_eq!(
                program.first_named_value_declared_type(symbol, symbol),
                Some(first_type)
            );
            let snapshot = program.clone();

            let mut replacement = arena::Arena::default();
            let replacement_handle = replacement.insert(parameter(replacement_type));
            assert_eq!(original_handle, replacement_handle);
            assert_eq!(program.state_parameters.revision(), replacement.revision());

            program.state_parameters = replacement;
            assert_eq!(
                program.first_named_value_declared_type(symbol, symbol),
                Some(replacement_type)
            );
            assert_eq!(
                snapshot.first_named_value_declared_type(symbol, symbol),
                Some(first_type)
            );

            let frozen_replacement = crate::FrozenTypedTrees::from(program);
            let frozen_snapshot = crate::FrozenTypedTrees::from(snapshot);
            assert_eq!(
                frozen_replacement.first_named_value_declared_type(symbol, symbol),
                Some(replacement_type)
            );
            assert_eq!(
                frozen_snapshot.first_named_value_declared_type(symbol, symbol),
                Some(first_type)
            );
        }

        #[test]
        fn machine_scope_lookup_follows_storage_replacement_and_independent_clones() {
            use crate::machine::Machine;
            let root = SymbolHandle::from_parts(10, 1);
            let retained = SymbolHandle::from_parts(11, 1);
            let next_root = SymbolHandle::from_parts(12, 1);
            let next_retained = SymbolHandle::from_parts(13, 1);
            let mut program = TypedTrees::default();
            program.push_machine(Machine {
                symbol: root,
                ..Default::default()
            });
            let retained_handle = program.tables.machines.insert(Machine {
                symbol: retained,
                name: "original".into(),
                ..Default::default()
            });
            assert_eq!(program.machine_by_symbol(root).unwrap().symbol, root);
            assert_eq!(
                program
                    .machine_declaration_for_lexical_scope(retained)
                    .unwrap()
                    .name
                    .as_str(),
                "original"
            );
            let mut fork = program.clone();
            fork.tables.machines.get_mut(retained_handle).name = "fork".into();
            assert_eq!(
                fork.machine_declaration_for_lexical_scope(retained)
                    .unwrap()
                    .name
                    .as_str(),
                "fork"
            );
            assert_eq!(
                program
                    .machine_declaration_for_lexical_scope(retained)
                    .unwrap()
                    .name
                    .as_str(),
                "original"
            );
            let revision = program.tables.machines.revision();
            let mut replacement = arena::Arena::new();
            let replacement_root = replacement.insert(Machine {
                symbol: next_root,
                ..Default::default()
            });
            replacement.insert(Machine {
                symbol: next_retained,
                ..Default::default()
            });
            while replacement.revision() < revision {
                let _ = replacement.get_mut(replacement_root);
            }
            assert_eq!(replacement.revision(), revision);
            program.tables.machines = replacement.into();
            assert!(program.machine_by_symbol(root).is_none());
            assert_eq!(
                program.machine_by_symbol(next_root).unwrap().symbol,
                next_root
            );
            assert!(
                program
                    .machine_declaration_for_lexical_scope(retained)
                    .is_none()
            );
            assert_eq!(
                program
                    .machine_declaration_for_lexical_scope(next_retained)
                    .unwrap()
                    .symbol,
                next_retained
            );
            assert!(
                program
                    .machine_declaration_for_lexical_scope(SymbolHandle::from_parts(13, 2))
                    .is_none()
            );
        }
    }

    type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;

    fn roster_marker<T>(span: HandleSpan<T>) -> u64 {
        ((span.start().arena_index() as u64) << 32) | span.count() as u64
    }

    fn tables_fingerprint(tables: &crate::TypedTreeTables) -> u64 {
        tables
            .const_declarations
            .revision()
            .wrapping_add(tables.data_definitions.revision())
            .wrapping_add(tables.data_application_definitions.revision())
            .wrapping_add(tables.data_type_parameters.revision())
            .wrapping_add(tables.data_members.revision())
            .wrapping_add(tables.data_payload_fields.revision())
            .wrapping_add(tables.domain_definitions.revision())
            .wrapping_add(tables.proof_facts.revision())
            .wrapping_add(tables.propositions.revision())
            .wrapping_add(tables.proposition_binders.revision())
            .wrapping_add(tables.mathematical_definitions.revision())
            .wrapping_add(tables.mathematical_parameters.revision())
            .wrapping_add(tables.mathematical_types.revision())
            .wrapping_add(tables.domain_path_members.revision())
            .wrapping_add(tables.operator_path_members.revision())
            .wrapping_add(tables.machines.revision())
            .wrapping_add(tables.measures.revision())
            .wrapping_add(tables.measure_path_members.revision())
            .wrapping_add(tables.operators.revision())
            .wrapping_add(tables.machine_owned_data.revision())
            .wrapping_add(tables.machine_trait_conformances.revision())
            .wrapping_add(tables.machine_states.revision())
            .wrapping_add(tables.state_parameters.revision())
            .wrapping_add(tables.traits.revision())
            .wrapping_add(tables.conformances.revision())
            .wrapping_add(tables.trait_requirements.revision())
            .wrapping_add(tables.trait_machine_signatures.revision())
            .wrapping_add(tables.applied_callable_signatures.revision())
            .wrapping_add(tables.signature_invokes.revision())
            .wrapping_add(tables.signature_contracts.revision())
            .wrapping_add(tables.expression_table.revision())
            .wrapping_add(tables.statement_table.revision())
            .wrapping_add(tables.type_reference_table.revision())
            .wrapping_add(tables.wire_schemas.revision())
            .wrapping_add(tables.wire_members.revision())
    }

    fn roots_fingerprint(roots: &crate::TypedTreeRoots) -> u64 {
        roster_marker(roots.const_declarations)
            .wrapping_add(roster_marker(roots.data_definitions))
            .wrapping_add(roster_marker(roots.domain_definitions))
            .wrapping_add(roster_marker(roots.machines))
            .wrapping_add(roster_marker(roots.measures))
            .wrapping_add(roster_marker(roots.operators))
            .wrapping_add(roster_marker(roots.machine_token_bindings))
            .wrapping_add(roster_marker(roots.propositions))
            .wrapping_add(roster_marker(roots.mathematical_definitions))
            .wrapping_add(roster_marker(roots.traits))
            .wrapping_add(roster_marker(roots.conformances))
            .wrapping_add(roster_marker(roots.wire_schemas))
    }

    type StatePositionIndex = SymbolRows<(u32, u32)>;

    pub use crate::data_definitions::DataDefinitionIndex;

    type DomainPositionIndex = SymbolRows<usize>;

    #[derive(Clone, Default)]
    pub struct CallTargetIndex {
        pub boundary_machine_by_state: symbols::SymbolKeyMap<SymbolHandle, u32>,
        pub parameter_signature: symbols::SymbolKeyMap<SymbolHandle, (u32, u32)>,
        pub trait_by_symbol: symbols::SymbolKeyMap<SymbolHandle, u32>,
        pub trait_signature: symbols::SymbolKeyMap<SymbolHandle, (u32, u32)>,
        pub parameter_contract: symbols::SymbolKeyMap<SymbolHandle, Handle<data::TypeParameter>>,
    }

    #[derive(Clone, Default)]
    pub struct IsolationVerdicts {
        pub isolation: HashMap<
            (
                TypeReferenceHandle,
                bool,
                Vec<(SymbolHandle, TypeReferenceHandle)>,
            ),
            bool,
        >,
        pub storage_match: HashMap<(TypeReferenceHandle, TypeReferenceHandle), bool>,
        pub reference_free: HashMap<
            (
                TypeReferenceHandle,
                Vec<(SymbolHandle, TypeReferenceHandle)>,
            ),
            bool,
        >,
        pub carry_write: HashMap<
            (
                TypeReferenceHandle,
                Vec<(SymbolHandle, TypeReferenceHandle)>,
            ),
            bool,
        >,
    }

    #[derive(Clone, Default)]
    pub struct DomainCandidateIndex {
        pub qualified: Vec<Box<str>>,
        pub by_qualified: HashMap<Box<str>, Vec<u32>>,
        pub by_relative: HashMap<Box<str>, Vec<u32>>,
    }

    #[derive(Clone, Default, compiler_storage::Record)]
    pub struct DeclarationIndexes {
        states: OwnedIndex<StatePositionIndex, (u64, u64, HandleSpan<machine::Machine>)>,
        state_declarations: OwnedIndex<StateDeclarationIndex, StateDeclarationRevision>,
        data_members: OwnedIndex<DataMemberIndex>,
        domains: OwnedIndex<DomainPositionIndex, DomainDeclarationScope>,
        call_targets: OwnedIndex<CallTargetIndex>,
        requirement_applications: OwnedIndex<RequirementApplicationIndex>,
        domain_candidates: OwnedIndex<DomainCandidateIndex, DomainDeclarationScope>,
        operator_spellings: OwnedIndex<OperatorSpellingIndex, OperatorSpellingScope>,
        isolation: OwnedIndex<IsolationVerdicts>,
    }

    impl PartialEq for DeclarationIndexes {
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }

    impl Eq for DeclarationIndexes {}

    impl std::fmt::Debug for DeclarationIndexes {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("DeclarationIndexes")
        }
    }

    impl crate::TypedTrees {
        pub(crate) fn seal_declaration_indexes(&mut self) {
            self.invalidate_state_declaration_index();
            self.declaration_indexes.states.clear();
            let any_symbol = self.symbols.root();
            self.with_machine_position_index(|_| ());
            self.with_state_position_index(|_| ());
            self.with_state_declarations(|_| ());
            self.with_data_members(|_| ());
            self.with_data_definition_index(|_| ());
            self.data_type_parameter_by_symbol(any_symbol);
            self.domain_position_by_symbol(any_symbol);
            self.with_call_target_index(|_| ());
            self.with_requirement_application_index(|_| ());
            self.with_operator_spelling_index(|_| ());
            let indexes = &mut self.declaration_indexes;
            self.tables.machines.seal();
            indexes.states.seal_with(SymbolRows::densify);
            indexes
                .state_declarations
                .seal_with(StateDeclarationIndex::densify);
            indexes.data_members.seal_with(DataMemberIndex::densify);
            self.tables.data_definitions.seal();
            self.tables.data_type_parameters.seal();
            indexes.domains.seal_with(SymbolRows::densify);
            indexes.call_targets.seal();
            indexes
                .requirement_applications
                .seal_with(RequirementApplicationIndex::densify);
            indexes.operator_spellings.seal();
        }

        pub(crate) fn invalidate_state_declaration_index(&mut self) {
            self.declaration_indexes.state_declarations.clear();
        }

        pub(crate) fn unseal_declaration_indexes(&mut self) {
            let indexes = &mut self.declaration_indexes;
            self.tables.machines.unseal();
            indexes.states.unseal();
            indexes.state_declarations.unseal();
            indexes.data_members.unseal();
            self.tables.data_definitions.unseal();
            self.tables.data_type_parameters.unseal();
            indexes.domains.unseal();
            indexes.call_targets.unseal();
            indexes.requirement_applications.unseal();
            indexes.operator_spellings.unseal();
        }

        pub fn structural_fingerprint(&self) -> u64 {
            tables_fingerprint(&self.tables)
                .wrapping_add(roots_fingerprint(&self.roots))
                .wrapping_add(self.service_reaches.definitions().len() as u64)
                .wrapping_add(self.service_reach_rows.len() as u64)
                .wrapping_add(self.authored_service_reach_rows.len() as u64)
                .wrapping_add(self.semantic_domains.len() as u64)
                .wrapping_add(self.external_bindings.identities().len() as u64)
                .wrapping_add(self.plan_laid_layouts.len() as u64)
                .wrapping_add(self.access_view_plans.len() as u64)
                .wrapping_add(self.access_lifecycle_plans.revision())
                .wrapping_add(self.access_policy_invocations.revision())
                .wrapping_add(self.access_borrow_factories.len() as u64)
                .wrapping_add(self.wire_placements.revision())
                .wrapping_add(self.wire_encode_obligations.revision())
                .wrapping_add(self.wire_schema_plans.len() as u64)
                .wrapping_add(self.machine_specializations.len() as u64)
                .wrapping_add(self.data_implementation_applications.revision())
                .wrapping_add(self.trait_implementation_applications.revision())
                .wrapping_add(self.captured_machine_applications.revision())
                .wrapping_add(self.boundary_completion_declarations.revision())
                .wrapping_add(self.evidence_forwardings.len() as u64)
                .wrapping_add(self.proof_output_calls.len() as u64)
                .wrapping_add(self.ranking_expression_custody.len() as u64)
                .wrapping_add(self.pending_const_range_endpoints.len() as u64)
                .wrapping_add(self.constant_recipes.revision())
                .wrapping_add(self.constant_expression_contexts.revision())
                .wrapping_add(self.constant_recipe_closure.values.len() as u64)
                .wrapping_add(self.excluded_authored_selection_occurrences.fingerprint())
        }

        pub fn machine_position_by_symbol(&self, symbol: SymbolHandle) -> Option<usize> {
            if !symbol.is_valid() {
                return None;
            }
            self.with_machine_position_index(|index| {
                index.first(symbol).map(|position| *position as usize)
            })
        }

        pub fn machine_positions_for_symbol(&self, symbol: SymbolHandle) -> Vec<u32> {
            self.with_machine_position_index(|index| index.get(symbol).to_vec())
        }

        fn with_machine_position_index<R>(
            &self,
            run: impl FnOnce(&MachinePositionIndex) -> R,
        ) -> R {
            self.tables
                .machines
                .with_root_index(self.roots.machines, run)
        }

        pub fn machine_by_symbol(&self, symbol: SymbolHandle) -> Option<&machine::Machine> {
            self.machine_position_by_symbol(symbol)
                .map(|position| &self.machines()[position])
        }

        pub fn machine_declaration_for_lexical_scope(
            &self,
            symbol: SymbolHandle,
        ) -> Option<&machine::Machine> {
            if !symbol.is_valid() {
                return None;
            }
            if let Some(machine) = self.machine_by_symbol(symbol) {
                return Some(machine);
            }
            self.tables.machines.retained_declaration(symbol)
        }

        pub fn state_position_by_symbol(
            &self,
            machine: SymbolHandle,
            state: SymbolHandle,
        ) -> Option<usize> {
            if !machine.is_valid() || !state.is_valid() {
                return None;
            }
            self.with_state_position_index(|index| {
                index
                    .get(state)
                    .iter()
                    .find(|(machine_position, _)| {
                        self.machines()[*machine_position as usize].symbol == machine
                    })
                    .map(|(_, state_position)| *state_position as usize)
            })
        }

        fn with_state_position_index<R>(&self, run: impl FnOnce(&StatePositionIndex) -> R) -> R {
            self.declaration_indexes.states.with(
                (
                    self.tables.machines.revision(),
                    self.tables.machine_states.revision(),
                    self.roots.machines,
                ),
                || self.build_state_positions(),
                run,
            )
        }

        pub(crate) fn with_state_positions_for_symbol<R>(
            &self,
            symbol: SymbolHandle,
            read: impl FnOnce(&[(u32, u32)]) -> R,
        ) -> R {
            self.with_state_position_index(|index| read(index.get(symbol)))
        }

        pub(crate) fn state_positions_for_symbol(&self, symbol: SymbolHandle) -> Vec<(u32, u32)> {
            self.with_state_positions_for_symbol(symbol, |positions| positions.to_vec())
        }

        /// The first machine, in table order, that declares a state named
        /// `state`: the answer a scan of every machine's states would give,
        /// read from the state index instead. Index rows keep table order.
        pub fn machine_containing_state(&self, state: SymbolHandle) -> Option<&machine::Machine> {
            let position = self.with_state_position_index(|index| {
                index
                    .first(state)
                    .map(|(machine_position, _)| *machine_position)
            })?;
            Some(&self.machines()[position as usize])
        }

        /// Every machine declared as `symbol`, in table order. A symbol can
        /// name several rows (a template and its requirement, say), so callers
        /// that also test supply mode or parameters filter this instead of
        /// scanning the whole table.
        pub fn machines_with_symbol(
            &self,
            symbol: SymbolHandle,
        ) -> impl Iterator<Item = &machine::Machine> + '_ {
            self.machine_positions_for_symbol(symbol)
                .into_iter()
                .map(|position| &self.machines()[position as usize])
        }

        pub fn machine_by_own_or_state_symbol(
            &self,
            symbol: SymbolHandle,
        ) -> Option<&machine::Machine> {
            let direct = self.with_machine_position_index(|index| index.first(symbol).copied());
            let containing = self.with_state_position_index(|index| {
                index
                    .first(symbol)
                    .map(|(machine_position, _)| *machine_position)
            });
            let position = match (direct, containing) {
                (Some(direct), Some(containing)) => direct.min(containing),
                (direct, containing) => direct.or(containing)?,
            };
            Some(&self.machines()[position as usize])
        }

        pub fn machine_by_entry_state_symbol(
            &self,
            symbol: SymbolHandle,
        ) -> Option<(&machine::Machine, &crate::state::State)> {
            let machine_position = self.with_state_position_index(|index| {
                index
                    .get(symbol)
                    .iter()
                    .find(|(_, state_position)| *state_position == 0)
                    .map(|(machine_position, _)| *machine_position)
            })?;
            let machine = &self.machines()[machine_position as usize];
            Some((machine, &self.machine_states(machine)[0]))
        }

        pub fn unique_machine_by_entry_state_symbol(
            &self,
            symbol: SymbolHandle,
        ) -> Option<(&machine::Machine, &crate::state::State)> {
            let machine_position = self.with_state_position_index(|index| {
                let mut entries = index
                    .get(symbol)
                    .iter()
                    .filter(|(_, state_position)| *state_position == 0)
                    .map(|(machine_position, _)| *machine_position);
                let first = entries.next()?;
                entries.next().is_none().then_some(first)
            })?;
            let machine = &self.machines()[machine_position as usize];
            Some((machine, &self.machine_states(machine)[0]))
        }

        fn build_state_positions(&self) -> StatePositionIndex {
            let mut rows = Vec::new();
            for (machine_position, machine) in self.machines().iter().enumerate() {
                for (state_position, state) in self.machine_states(machine).iter().enumerate() {
                    rows.push((
                        state.symbol,
                        (machine_position as u32, state_position as u32),
                    ));
                }
            }
            SymbolRows::new(rows)
        }

        fn with_type_parameter_index<Result>(
            &self,
            run: impl FnOnce(&TypeParameterIndex) -> Result,
        ) -> Result {
            self.data_type_parameters.with_index(run)
        }

        pub(crate) fn with_type_parameter_declarations<R>(
            &self,
            symbol: SymbolHandle,
            read: impl FnOnce(&[Handle<data::TypeParameter>]) -> R,
        ) -> R {
            self.with_type_parameter_index(|index| read(index.get(symbol)))
        }

        pub fn data_type_parameter_by_symbol(
            &self,
            symbol: SymbolHandle,
        ) -> Option<&data::TypeParameter> {
            let handle = self.data_type_parameter_handle_by_symbol(symbol)?;
            Some(self.data_type_parameters.get(handle))
        }

        pub fn data_type_parameter_handle_by_symbol(
            &self,
            symbol: SymbolHandle,
        ) -> Option<arena::Handle<data::TypeParameter>> {
            if !symbol.is_valid() {
                return None;
            }
            self.with_type_parameter_index(|index| index.get(symbol).last().copied())
        }

        pub fn first_data_type_parameter_by_symbol(
            &self,
            symbol: SymbolHandle,
        ) -> Option<&data::TypeParameter> {
            if !symbol.is_valid() {
                return None;
            }
            let handle = self.with_type_parameter_index(|index| index.first(symbol).copied())?;
            Some(self.data_type_parameters.get(handle))
        }

        pub fn with_data_definition_index<R>(
            &self,
            run: impl FnOnce(&DataDefinitionIndex) -> R,
        ) -> R {
            self.tables
                .data_definitions
                .with_index(self.roots.data_definitions, run)
        }

        pub fn data_definition_position_by_symbol(&self, symbol: SymbolHandle) -> Option<usize> {
            if !symbol.is_valid() {
                return None;
            }
            self.with_data_definition_index(|index| index.first_position(symbol))
        }

        pub fn data_definition_by_symbol(
            &self,
            symbol: SymbolHandle,
        ) -> Option<&data::DataDefinition> {
            self.data_definition_position_by_symbol(symbol)
                .map(|position| &self.data_definitions()[position])
        }

        pub fn unique_data_definition_by_symbol(
            &self,
            symbol: SymbolHandle,
        ) -> Option<&data::DataDefinition> {
            if !symbol.is_valid() {
                return None;
            }
            let position = self.with_data_definition_index(|index| {
                let [position] = index.positions_for_symbol(symbol) else {
                    return None;
                };
                Some(*position as usize)
            })?;
            self.data_definitions().get(position)
        }

        pub fn data_definition_positions_for_symbol(&self, symbol: SymbolHandle) -> Vec<u32> {
            self.with_data_definition_index(|index| index.positions_for_symbol(symbol).to_vec())
        }

        pub fn data_definition_positions_for_name(&self, name: &str) -> Vec<u32> {
            self.with_data_definition_index(|index| index.positions_for_name(name).to_vec())
        }

        pub fn domain_position_by_symbol(&self, symbol: SymbolHandle) -> Option<usize> {
            if !symbol.is_valid() {
                return None;
            }
            self.declaration_indexes.domains.with(
                self.domain_declaration_scope(),
                || {
                    SymbolRows::new(
                        self.domain_definitions()
                            .iter()
                            .enumerate()
                            .map(|(position, domain)| (domain.symbol, position))
                            .collect(),
                    )
                },
                |index| index.first(symbol).copied(),
            )
        }

        pub fn data_selects_cleanup(&self, data_symbol: SymbolHandle) -> bool {
            data_symbol.is_valid()
                && self
                    .unique_data_definition_by_symbol(data_symbol)
                    .is_some_and(|data| data.cleanup_selection.is_some())
        }

        pub fn with_call_target_index<R>(&self, run: impl FnOnce(&CallTargetIndex) -> R) -> R {
            let revision = self
                .tables
                .machines
                .revision()
                .wrapping_add(self.tables.machine_states.revision())
                .wrapping_add(self.tables.data_type_parameters.revision())
                .wrapping_add(self.tables.traits.revision())
                .wrapping_add(self.tables.trait_machine_signatures.revision())
                .wrapping_add(roster_marker(self.roots.machines))
                .wrapping_add(roster_marker(self.roots.traits));
            self.declaration_indexes.call_targets.with(
                revision,
                || self.build_call_target_index(),
                run,
            )
        }

        fn build_call_target_index(&self) -> CallTargetIndex {
            let mut index = CallTargetIndex::default();
            for (machine_index, machine) in self.machines().iter().enumerate() {
                if machine.supply_mode.is_boundary_declaration() {
                    for state in self.machine_states(machine) {
                        index
                            .boundary_machine_by_state
                            .entry(state.symbol)
                            .or_insert(machine_index as u32);
                    }
                }
                for (parameter_index, parameter) in
                    self.machine_type_parameters(machine).iter().enumerate()
                {
                    if matches!(parameter.kind, data::TypeParameterKind::Machine { .. }) {
                        index
                            .parameter_signature
                            .entry(parameter.symbol)
                            .or_insert((machine_index as u32, parameter_index as u32));
                    }
                }
            }
            for (trait_index, trait_definition) in self.traits().iter().enumerate() {
                index
                    .trait_by_symbol
                    .entry(trait_definition.symbol)
                    .or_insert(trait_index as u32);
                for (signature_index, signature) in self
                    .trait_machine_signatures(trait_definition)
                    .iter()
                    .enumerate()
                {
                    index
                        .trait_signature
                        .entry(signature.symbol)
                        .or_insert((trait_index as u32, signature_index as u32));
                }
            }
            for (handle, parameter) in self.data_type_parameters.iter() {
                if matches!(parameter.kind, data::TypeParameterKind::Machine { .. }) {
                    index
                        .parameter_contract
                        .entry(parameter.symbol)
                        .or_insert(handle);
                }
            }
            index
        }

        pub fn with_domain_candidate_index<R>(
            &self,
            build: impl FnOnce(&mut DomainCandidateIndex),
            run: impl FnOnce(&DomainCandidateIndex) -> R,
        ) -> R {
            self.declaration_indexes.domain_candidates.with_mut(
                self.domain_declaration_scope(),
                || {
                    let mut index = DomainCandidateIndex::default();
                    build(&mut index);
                    index
                },
                |index| run(index),
            )
        }

        pub fn with_isolation_verdicts<R>(
            &self,
            revision: u64,
            run: impl FnOnce(&mut IsolationVerdicts) -> R,
        ) -> R {
            self.declaration_indexes
                .isolation
                .with_mut(revision, IsolationVerdicts::default, run)
        }
    }
}

pub(crate) mod declaration_tables {
    use crate::ClosedConformanceApplicationSite::Occurrence;
    use crate::machine::Machine;
    use crate::mathematical::MathematicalDefinition;
    use crate::name::Identifier;
    use crate::static_applications::StaticMachineApplicationHandle;
    use crate::static_applications::StaticMachineBindingValue;
    use crate::types::TypeReferenceHandle;
    use crate::{ClosedConformanceApplication, ClosedConformanceApplicationSite};
    use crate::{TypedTrees, proof_fact_source_span_index};
    use crate::{data, domain, measure, proposition, signature};
    use arena::{Handle, HandleSpan};
    use symbols::JoinedPath;

    pub type ConformanceSite<'a> = (
        ClosedConformanceApplicationSite,
        symbols::SymbolHandle,
        StaticMachineApplicationHandle,
        &'a ClosedConformanceApplication,
    );

    #[derive(Clone, Copy, Default)]
    pub(crate) enum LifetimeTemplateCandidates {
        #[default]
        Absent,
        One(symbols::SymbolHandle),
        Ambiguous,
    }

    impl LifetimeTemplateCandidates {
        pub(crate) fn include(&mut self, template: symbols::SymbolHandle) {
            *self = match self {
                Self::Absent => Self::One(template),
                Self::One(_) | Self::Ambiguous => Self::Ambiguous,
            };
        }
    }

    impl TypedTrees {
        pub fn declaration_scope_parent(
            &self,
            owner: symbols::SymbolHandle,
        ) -> symbols::SymbolHandle {
            self.declaration_scope_parent_for_lifetime_owner(self.lifetime_declaration_owner(owner))
        }

        pub(crate) fn declaration_scope_parent_for_lifetime_owner(
            &self,
            owner: symbols::SymbolHandle,
        ) -> symbols::SymbolHandle {
            if !owner.is_valid() {
                return owner;
            }
            let enclosing = self.symbols.get(owner).declaration_owner;
            if enclosing.is_valid()
                && self
                    .conformance_by_symbol(enclosing)
                    .is_some_and(|conformance| {
                        conformance.rows.iter().any(|row| {
                            row.source.owns_realization() && row.realization_machine == owner
                        })
                    })
            {
                return enclosing;
            }
            self.symbols.get(owner).parent
        }

        pub fn owned_conformance_realization(
            &self,
            machine: symbols::SymbolHandle,
        ) -> Option<(
            &crate::trait_definition::Conformance,
            &crate::trait_definition::ConformanceRow,
        )> {
            if !machine.is_valid() {
                return None;
            }
            let owner = self.symbols.get(machine).declaration_owner;
            let implementation = self.product_declarations().conformance_by_symbol(owner)?;
            let mut rows = implementation
                .rows
                .iter()
                .filter(|row| row.source.owns_realization() && row.realization_machine == machine);
            let row = rows.next()?;
            rows.next().is_none().then_some((implementation, row))
        }

        /// Whether `machine` is the inline member of a conformance row whose
        /// requirement wrote `crashes none`.
        pub fn realizes_crash_free_requirement(&self, machine: symbols::SymbolHandle) -> bool {
            let Some((_, row)) = self.owned_conformance_realization(machine) else {
                return false;
            };
            self.trait_definition_by_symbol(row.declaring_trait)
                .and_then(|declaring| {
                    self.trait_machine_signature_by_symbol(declaring, row.requirement)
                })
                .is_some_and(|requirement| requirement.declares_no_crash_route)
        }

        pub fn owned_conformance_requirement_scope(
            &self,
            realization: symbols::SymbolHandle,
            declaring_trait: symbols::SymbolHandle,
            requirement: symbols::SymbolHandle,
        ) -> Option<crate::trait_definition::AppliedTraitScope<'_>> {
            let (implementation, row) = self.owned_conformance_realization(realization)?;
            if row.declaring_trait != declaring_trait || row.requirement != requirement {
                return None;
            }
            let machine = self.machine_by_symbol(realization)?;
            let entry = self.machine_states(machine).first()?;
            if entry.symbol != row.realization_state {
                return None;
            }
            crate::trait_definition::AppliedTraitScope::from_conformance(self, implementation)?
                .declaring_scope(self, declaring_trait)
        }

        pub fn live_conformance_occurrence_applications(
            &self,
        ) -> Vec<StaticMachineApplicationHandle> {
            crate::references::collect_program_conformance_occurrences(self)
        }

        pub fn live_product_operand_applications(
            &self,
        ) -> Vec<Handle<crate::product_operand::ProductOperandApplication>> {
            if self.product_operand_applications.is_empty() {
                return Vec::new();
            }
            let occurrences = crate::product_operand::collect_product_selection_occurrences(self);
            let mut live = self
                .product_operand_applications
                .iter()
                .filter_map(|(handle, selected)| {
                    let role = selected.role?;
                    let author = self.symbols.authored_origin(selected.owner);
                    occurrences
                        .iter()
                        .any(|occurrence| {
                            self.symbols.authored_origin(occurrence.machine) == author
                                && occurrence.request.operands.iter().enumerate().any(
                                    |(ordinal, operand)| {
                                        occurrence.request.operation.product_operand_role(ordinal)
                                            == Some(role)
                                            && self.same_product_operand_source(
                                                operand,
                                                &selected.operand,
                                            )
                                    },
                                )
                        })
                        .then_some(handle)
                })
                .collect::<Vec<_>>();
            for handle in self
                .retained_product_application_handles
                .span_or_empty(self.roots.retained_product_applications)
            {
                if self.product_operand_applications.is_valid(*handle) && !live.contains(handle) {
                    live.push(*handle);
                }
            }
            live
        }

        pub fn closed_conformance_application_sites(&self) -> Vec<ConformanceSite<'_>> {
            self.closed_conformance_application_sites_matching(|_| true, None)
        }

        pub fn closed_conformance_application_sites_for_owners(
            &self,
            owners: &[symbols::SymbolHandle],
        ) -> Vec<ConformanceSite<'_>> {
            self.closed_conformance_application_sites_matching(
                |owner| owners.contains(&owner),
                None,
            )
        }

        pub(crate) fn closed_conformance_application_sites_from_occurrences(
            &self,
            occurrences: &[StaticMachineApplicationHandle],
        ) -> Vec<ConformanceSite<'_>> {
            self.closed_conformance_application_sites_matching(|_| true, Some(occurrences))
        }

        fn closed_conformance_application_sites_matching(
            &self,
            includes_owner: impl Fn(symbols::SymbolHandle) -> bool,
            known_live: Option<&[StaticMachineApplicationHandle]>,
        ) -> Vec<ConformanceSite<'_>> {
            let mut sites = Vec::new();
            for (s, receipt) in self.machine_specializations.iter().enumerate() {
                if !includes_owner(receipt.instance) {
                    continue;
                }
                for (a, application) in receipt.conformance_applications.iter().enumerate() {
                    sites.push((
                        crate::ClosedConformanceApplicationSite::Invocation {
                            specialization: s,
                            application: a,
                        },
                        receipt.instance,
                        receipt.binding_application,
                        application,
                    ));
                }
            }
            let mut occurrences = self
                .expression_table
                .static_machine_applications()
                .filter(|(_, source)| includes_owner(source.lexical_owner))
                .filter_map(|(handle, source)| {
                    let application = source
                        .conformance_selection
                        .as_ref()?
                        .application
                        .as_deref()?;
                    Some((handle, source.lexical_owner, application))
                })
                .peekable();
            if occurrences.peek().is_some() {
                let discovered;
                let live = match known_live {
                    Some(occurrences) => occurrences,
                    None => {
                        discovered = self.live_conformance_occurrence_applications();
                        &discovered
                    }
                };
                for (handle, owner, application) in occurrences {
                    if live
                        .binary_search_by_key(&handle.arena_index(), |live| live.arena_index())
                        .is_ok_and(|position| live[position] == handle)
                    {
                        sites.push((Occurrence(handle), owner, handle, application));
                    }
                }
            }
            for (handle, selected) in self.trait_implementation_applications.iter() {
                let source = self.static_machine_application(selected.supplying_application);
                if !includes_owner(source.lexical_owner) {
                    continue;
                }
                sites.push((
                    crate::ClosedConformanceApplicationSite::TraitArgument(handle),
                    source.lexical_owner,
                    selected.supplying_application,
                    &selected.application,
                ));
            }
            for handle in self.live_product_operand_applications() {
                let selected = self.product_operand_applications.get(handle);
                let Some(application) = selected.implementation.as_ref() else {
                    continue;
                };
                let source = self.static_machine_application(selected.supplying_application);
                if !includes_owner(source.lexical_owner) {
                    continue;
                }
                sites.push((
                    crate::ClosedConformanceApplicationSite::ProductOperand(handle),
                    source.lexical_owner,
                    selected.supplying_application,
                    application,
                ));
            }
            for (handle, selected) in self.data_implementation_applications.iter() {
                if includes_owner(selected.lexical_owner) {
                    sites.push((
                        crate::ClosedConformanceApplicationSite::DataArgument(handle),
                        selected.lexical_owner,
                        selected.supplying_application,
                        &selected.application,
                    ));
                }
            }
            sites
        }

        pub fn closed_conformance_application_at_mut(
            &mut self,
            site: crate::ClosedConformanceApplicationSite,
        ) -> Option<&mut crate::ClosedConformanceApplication> {
            match site {
                crate::ClosedConformanceApplicationSite::ProductOperand(handle) => self
                    .product_operand_applications
                    .get_mut(handle)
                    .implementation
                    .as_mut(),
                crate::ClosedConformanceApplicationSite::DataArgument(handle) => Some(
                    &mut self
                        .data_implementation_applications
                        .get_mut(handle)
                        .application,
                ),
                crate::ClosedConformanceApplicationSite::TraitArgument(handle) => Some(
                    &mut self
                        .trait_implementation_applications
                        .get_mut(handle)
                        .application,
                ),
                crate::ClosedConformanceApplicationSite::Invocation {
                    specialization,
                    application,
                } => self
                    .machine_specializations
                    .get_mut(specialization)?
                    .conformance_applications
                    .get_mut(application),
                Occurrence(handle) => self
                    .expression_table
                    .static_machine_application_mut(handle)
                    .conformance_selection
                    .as_mut()?
                    .application
                    .as_deref_mut(),
            }
        }

        pub fn retain_live_trait_implementation_applications(
            &mut self,
            removed: impl Fn(symbols::SymbolHandle) -> bool,
        ) {
            let mut captured_supplies =
                std::collections::HashMap::<_, Vec<_>, symbols::BuildSymbolHasher>::default();
            for (_, capture) in self.captured_machine_applications.iter() {
                if self.machine_by_symbol(capture.instance).is_some() {
                    captured_supplies
                        .entry(capture.supplying_application)
                        .or_default()
                        .push(&capture.conformance_application);
                }
            }
            let owner_removed = |mut owner: symbols::SymbolHandle| {
                while owner.is_valid() {
                    if removed(owner) {
                        return true;
                    }
                    owner = self.symbols.get(owner).parent;
                }
                false
            };
            let selected = self
                .data_implementation_applications
                .iter()
                .filter(|(_, selected)| {
                    !owner_removed(selected.lexical_owner)
                        || captured_supplies
                            .get(&selected.supplying_application)
                            .is_some_and(|applications| {
                                applications
                                    .iter()
                                    .any(|application| **application == selected.application)
                            })
                })
                .map(|(_, selected)| selected.clone())
                .collect::<Vec<_>>();
            let mut applications = arena::Arena::new();
            applications.insert_many(selected);
            self.data_implementation_applications = applications;
            let known_live = self
                .trait_implementation_applications
                .iter()
                .filter(|(_, selected)| {
                    let source_owner = selected.source_scope.supplying_owner();
                    let supplying_owner = if selected.supplying_application.is_valid() {
                        self.expression_table
                            .static_machine_application(selected.supplying_application)
                            .lexical_owner
                    } else {
                        symbols::SymbolHandle::invalid()
                    };
                    !(owner_removed(source_owner) && owner_removed(supplying_owner))
                })
                .map(|(_, selected)| selected.clone())
                .collect::<Vec<_>>();
            let mut applications = arena::Arena::default();
            applications.insert_many(known_live);
            self.trait_implementation_applications = applications;
            if self.trait_implementation_applications.is_empty() {
                return;
            }
            let roots = self.applied_trait_roots();
            let selected = self
                .trait_implementation_applications
                .iter()
                .filter(|(_, selected)| {
                    let Some(scope) = selected.source_scope.scope(self) else {
                        return true;
                    };
                    let mut live = false;
                    for root in &roots {
                        root.visit(self, &mut |current| {
                            live |= current.argument_owner == scope.argument_owner
                                && current.same_application(self, &scope);
                        });
                    }
                    live
                })
                .map(|(_, selected)| selected.clone())
                .collect::<Vec<_>>();
            let mut applications = arena::Arena::default();
            applications.insert_many(selected);
            self.trait_implementation_applications = applications;
        }

        pub fn closed_application_selected_machines(
            &self,
            application: &crate::ClosedConformanceApplication,
        ) -> &[crate::expression::StaticMachineArgument] {
            self.expression_table
                .static_machine_application_selected_machines(application.callable_application)
        }

        pub fn specialization_binding(
            &self,
            specialization: &crate::MachineSpecialization,
            parameter: symbols::SymbolHandle,
        ) -> Option<&StaticMachineBindingValue> {
            let mut bindings = self
                .static_machine_application_bindings(specialization.binding_application)
                .iter()
                .filter(|binding| binding.parameter == parameter);
            let selected = bindings.next()?;
            bindings.next().is_none().then_some(&selected.value)
        }

        pub fn selected_conformance_applications(
            &self,
            owner: symbols::SymbolHandle,
            selection_application: StaticMachineApplicationHandle,
            evidence_parameter: symbols::SymbolHandle,
            requirement: symbols::SymbolHandle,
            report_fingerprint: u64,
            commitment: crate::ClosedConformanceApplicationCommitment,
        ) -> Vec<&crate::ClosedConformanceApplication> {
            if selection_application.is_valid() {
                if evidence_parameter.is_valid() {
                    return Vec::new();
                }
                let source = self.static_machine_application(selection_application);
                // A constant has no body of its own: its initializer runs in an
                // evaluation entry generated from the constant, which wraps the
                // same call. The call's application still names the constant as
                // the scope it was written in, and that is the same owner.
                let owned_by_caller = source.lexical_owner == owner
                    || (self.symbols.get(source.lexical_owner).kind == symbols::SymbolKind::Const
                        && self.symbols.get(owner).kind == symbols::SymbolKind::Machine
                        && self.symbols.get(owner).generated_from == source.lexical_owner);
                if !owned_by_caller || source.registration_operation != requirement {
                    return Vec::new();
                }
                let Some(selection) = &source.conformance_selection else {
                    return Vec::new();
                };
                let Some(application) = selection.application.as_deref() else {
                    return Vec::new();
                };
                let Some(operand) = self
                    .static_machine_application_arguments(selection_application)
                    .get(selection.operand_ordinal as usize)
                else {
                    return Vec::new();
                };
                let raw_arguments = operand
                    .application
                    .as_ref()
                    .map_or(&[][..], |a| a.arguments.as_ref());
                let raw_lifetimes = operand
                    .application
                    .as_ref()
                    .map_or(&[][..], |a| a.lifetime_arguments.as_ref());
                if raw_arguments != application.arguments.as_ref()
                    || raw_lifetimes.len() != application.lifetime_arguments.len()
                    || raw_lifetimes
                        .iter()
                        .zip(&application.lifetime_arguments)
                        .any(|(raw, actual)| raw.as_str() != actual)
                    || operand.symbol != application.declaration
                    || application.commitment != commitment
                    || application.report_fingerprint != report_fingerprint
                {
                    return Vec::new();
                }
                return vec![application];
            }
            if evidence_parameter.is_valid() {
                let mut selected = Vec::new();
                for specialization in self
                    .machine_specializations
                    .iter()
                    .filter(|specialization| specialization.instance == owner)
                {
                    let bindings = self
                        .static_machine_application_bindings(specialization.binding_application)
                        .iter()
                        .filter(|binding| {
                            matches!(binding.value, StaticMachineBindingValue::Conformance(_))
                        });
                    if bindings.clone().count() != specialization.conformance_applications.len() {
                        return Vec::new();
                    }
                    for (binding, application) in
                        bindings.zip(&specialization.conformance_applications)
                    {
                        if binding.parameter == evidence_parameter {
                            if application.report_fingerprint != report_fingerprint
                                || application.commitment != commitment
                            {
                                return Vec::new();
                            }
                            selected.push(application);
                        }
                    }
                }
                return selected;
            }
            crate::closed_conformance_applications_of(
                &self.machine_specializations,
                owner,
                report_fingerprint,
                commitment,
            )
        }

        pub fn callable_body_has_open_parameters(&self, machine: &Machine) -> bool {
            machine.has_specialization_parameters()
                || self.conformances().iter().any(|conformance| {
                    (!self.conformance_type_parameters(conformance).is_empty()
                        || !conformance.lifetime_parameters.is_empty())
                        && conformance.rows.iter().any(|row| {
                            row.source.owns_realization()
                                && row.realization_machine == machine.symbol
                        })
                })
        }

        pub fn executable_application_template(
            &self,
            instance: symbols::SymbolHandle,
        ) -> Result<Option<symbols::SymbolHandle>, &'static str> {
            let mut templates = self
                .machine_specializations
                .iter()
                .filter(|row| row.instance == instance)
                .map(|row| row.template)
                .chain(
                    self.captured_machine_applications
                        .iter()
                        .filter(|(_, row)| row.instance == instance)
                        .map(|(_, row)| row.template),
                );
            let template = templates.next();
            if templates.next().is_some() {
                return Err("executable application has ambiguous template ownership");
            }
            Ok(template)
        }

        pub fn machine_invokes_declaration(
            &self,
            mut callable: symbols::SymbolHandle,
            declaration: symbols::SymbolHandle,
        ) -> Result<bool, &'static str> {
            for _ in 0..=self.machines().len() {
                if callable == declaration {
                    return Ok(true);
                }
                if self.target_machine_families.iter().any(|family| {
                    family.canonical == declaration
                        && family
                            .members
                            .iter()
                            .any(|member| member.machine == callable)
                }) {
                    return Ok(true);
                }
                let template = match self.executable_application_template(callable)? {
                    Some(template) => template,
                    None => {
                        let Some(machine) = self.machine(callable) else {
                            return Ok(false);
                        };
                        machine.generic_data_template
                    }
                };
                if !template.is_valid() {
                    return Ok(false);
                }
                if template == callable {
                    return Err("executable application has cyclic template ownership");
                }
                callable = template;
            }
            Err("executable application template chain exceeds retained declarations")
        }

        pub fn realization_entry_template(
            &self,
            entry: symbols::SymbolHandle,
            application: &crate::ClosedConformanceApplication,
        ) -> Option<(symbols::SymbolHandle, symbols::SymbolHandle)> {
            self.realization_entry_lineage(entry, application)?
                .last()
                .copied()
        }

        pub fn realization_entry_lineage(
            &self,
            entry: symbols::SymbolHandle,
            application: &crate::ClosedConformanceApplication,
        ) -> Option<Vec<(symbols::SymbolHandle, symbols::SymbolHandle)>> {
            self.entry_lineage_with(entry, None, |capture| {
                self.closed_conformance_applications_match(
                    &capture.conformance_application,
                    application,
                ) && self
                    .conformance_by_symbol(application.declaration)
                    .is_some_and(|selecting| {
                        selecting.rows.iter().any(|row| {
                            row.source.owns_realization()
                                && row.realization_machine == capture.template
                        })
                    })
            })
        }

        pub fn requirement_realization_entry_lineage(
            &self,
            entry: symbols::SymbolHandle,
            requirement: symbols::SymbolHandle,
            selecting: symbols::SymbolHandle,
            realization: symbols::SymbolHandle,
        ) -> Option<Vec<(symbols::SymbolHandle, symbols::SymbolHandle)>> {
            let provider = self.machine_holding_state(entry)?;
            if !self.requirement_realization_matches(requirement, realization, provider.symbol) {
                return None;
            }
            let lineage = self.entry_lineage_with(entry, Some(realization), |capture| {
                capture.conformance_application.declaration == selecting
                    && self
                        .conformance_by_symbol(selecting)
                        .is_some_and(|declaration| {
                            declaration.rows.iter().any(|row| {
                                row.source.owns_realization()
                                    && row.realization_machine == capture.template
                            })
                        })
                    && capture
                        .conformance_application
                        .trait_scope
                        .scope(self)
                        .is_some_and(|scope| {
                            self.requirement_application_matches_scope(requirement, &scope)
                        })
            })?;
            (lineage.last()?.0 == realization).then_some(lineage)
        }

        fn entry_lineage_with(
            &self,
            mut entry: symbols::SymbolHandle,
            stop: Option<symbols::SymbolHandle>,
            capture_matches: impl Fn(&crate::CapturedMachineApplication) -> bool,
        ) -> Option<Vec<(symbols::SymbolHandle, symbols::SymbolHandle)>> {
            let mut visited = Vec::new();
            loop {
                let machine = self.machine_containing_state(entry)?;
                if visited.iter().any(|(symbol, _)| *symbol == machine.symbol) {
                    return None;
                }
                visited.push((machine.symbol, entry));
                if stop == Some(machine.symbol) {
                    return Some(visited);
                }
                let invocations = self
                    .machine_specializations
                    .iter()
                    .filter(|invocation| invocation.instance == machine.symbol)
                    .collect::<Vec<_>>();
                let captures = self
                    .captured_machine_applications
                    .iter()
                    .filter(|(_, capture)| capture.instance == machine.symbol)
                    .map(|(_, capture)| capture)
                    .collect::<Vec<_>>();
                if invocations.len() + captures.len() > 1 {
                    return None;
                }
                if let [invocation] = invocations.as_slice() {
                    let ordinal = self
                        .machine_states(machine)
                        .iter()
                        .position(|state| state.symbol == entry)?;
                    let template = self.machine_by_symbol(invocation.template)?;
                    entry = self.machine_states(template).get(ordinal)?.symbol;
                    continue;
                }
                if let [capture] = captures.as_slice() {
                    if !capture_matches(capture) {
                        return None;
                    }
                    let states = self.captured_machine_states.span_or_empty(capture.states);
                    let mut states = states.iter().filter(|state| state.instance == entry);
                    let template = states.next()?.template;
                    if states.next().is_some() {
                        return None;
                    }
                    entry = template;
                    continue;
                }
                return Some(visited);
            }
        }

        pub fn lifetime_declaration_owner(
            &self,
            owner: symbols::SymbolHandle,
        ) -> symbols::SymbolHandle {
            self.lifetime_declaration_owner_with_templates(
                owner,
                |owner| {
                    let mut templates = self
                        .machine_specializations
                        .iter()
                        .filter(|application| application.instance == owner)
                        .map(|application| application.template)
                        .chain(
                            self.captured_machine_applications
                                .iter()
                                .filter(|(_, application)| application.instance == owner)
                                .map(|(_, application)| application.template),
                        );
                    match templates.next() {
                        None => LifetimeTemplateCandidates::Absent,
                        Some(template) => {
                            if templates.next().is_some() {
                                LifetimeTemplateCandidates::Ambiguous
                            } else {
                                LifetimeTemplateCandidates::One(template)
                            }
                        }
                    }
                },
                |symbol| self.retained_static_machine_contract(symbol).is_some(),
            )
        }

        pub(crate) fn lifetime_declaration_owner_with_templates(
            &self,
            mut owner: symbols::SymbolHandle,
            mut templates_for: impl FnMut(symbols::SymbolHandle) -> LifetimeTemplateCandidates,
            mut retained_contract: impl FnMut(symbols::SymbolHandle) -> bool,
        ) -> symbols::SymbolHandle {
            let mut visited = Vec::new();
            if let Some(application) = self.closed_requirement_application(owner) {
                owner = application.template;
            }
            loop {
                let mut templates = templates_for(owner);
                if let Some(original) = retained_contract(owner)
                    .then(|| {
                        let original = self.symbols.get(owner).generated_from;
                        (original.is_valid() && retained_contract(original)).then_some(original)
                    })
                    .flatten()
                {
                    templates.include(original);
                }
                let template = match templates {
                    LifetimeTemplateCandidates::Absent => break,
                    LifetimeTemplateCandidates::One(template) => template,
                    LifetimeTemplateCandidates::Ambiguous => {
                        return symbols::SymbolHandle::invalid();
                    }
                };
                if visited.contains(&owner) {
                    return symbols::SymbolHandle::invalid();
                }
                visited.push(owner);
                owner = template;
            }
            owner
        }
        crate::typed_tree_table! {
            const_declarations => const_declarations(crate::constant::ConstDeclaration) [push_const_declaration];
            data_definitions => data_definitions(data::DataDefinition) [push_data_definition], inline;
            type_aliases => type_aliases(crate::type_aliases::TypeAliasDefinition) [push_type_alias_definition], inline;
            domain_definitions => domain_definitions(domain::DomainDefinition) [push_domain_definition];
            propositions => propositions(proposition::PropositionDefinition) [push_proposition];
            mathematical_definitions => mathematical_definitions(MathematicalDefinition);
            measures => measures(measure::MeasureDefinition) [push_measure];
            operators => operators(crate::operator::OperatorDefinition) [push_operator];
        }

        pub fn retain_authored_declaration_selections(
            &mut self,
            selections: language_semantics::declaration_selection::AuthoredDeclarationSelections,
        ) {
            self.tables.authored_declaration_selections = selections;
        }

        pub fn authored_declaration_selections(
            &self,
        ) -> &language_semantics::declaration_selection::AuthoredDeclarationSelections {
            &self.tables.authored_declaration_selections
        }

        pub fn const_declaration_by_symbol(
            &self,
            symbol: symbols::SymbolHandle,
        ) -> Option<&crate::constant::ConstDeclaration> {
            self.const_declarations()
                .iter()
                .find(|declaration| declaration.symbol == symbol)
        }

        pub fn const_declaration_for_evaluation_entry(
            &self,
            entry: symbols::SymbolHandle,
        ) -> Option<&crate::constant::ConstDeclaration> {
            if !entry.is_valid() {
                return None;
            }
            self.const_declarations().iter().find(|declaration| {
                self.tables.machines.is_valid(declaration.evaluation_entry)
                    && self
                        .tables
                        .machines
                        .get(declaration.evaluation_entry)
                        .symbol
                        == entry
            })
        }

        pub fn expression_const_declaration(
            &self,
            expression: crate::expression::ExpressionHandle,
        ) -> Option<&crate::constant::ConstDeclaration> {
            let mut declaration: Option<&crate::constant::ConstDeclaration> = None;
            for occurrence in self
                .expression_table
                .authored_selection_occurrences(expression)
            {
                let selection = self.authored_declaration_selections().get(occurrence)?;
                if selection.source_span() != self.expression_table.source_span(expression) {
                    continue;
                }
                let target = selection.target();
                let Some(selected) = target.as_resolved() else {
                    continue;
                };
                let Some(current) = self.const_declaration_by_symbol(selected.selected_symbol())
                else {
                    continue;
                };
                if declaration.is_some_and(|previous| previous.symbol != current.symbol) {
                    return None;
                }
                declaration = Some(current);
            }
            declaration
        }

        pub fn record_late_bound_authored_declaration_selection(
            &mut self,
            source_span: source::SourceSpan,
            exposure: language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
            kind: language_semantics::declaration_selection::AuthoredDeclarationSelectionKind,
            binding: language_semantics::declaration_selection::AuthoredDeclarationSelectionLateBinding,
        ) -> Result<
            language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
            language_semantics::declaration_selection::AuthoredDeclarationSelectionRecordError,
        > {
            self.tables
                .authored_declaration_selections
                .record_late_bound(source_span, exposure, kind, binding)
        }

        pub fn record_resolved_authored_declaration_selection_once(
            &mut self,
            source_span: source::SourceSpan,
            exposure: language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
            kind: language_semantics::declaration_selection::AuthoredDeclarationSelectionKind,
            selected_symbol: symbols::SymbolHandle,
        ) -> Result<
            language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
            language_semantics::declaration_selection::AuthoredDeclarationSelectionRecordError,
        > {
            if let Some(existing) = self
                .tables
                .authored_declaration_selections
                .resolved_occurrence(source_span, exposure, kind, selected_symbol)
            {
                return Ok(existing);
            }
            self.tables.authored_declaration_selections.record_resolved(
                source_span,
                exposure,
                kind,
                selected_symbol,
            )
        }

        pub fn retain_data_application_definition(
            &mut self,
            application: TypeReferenceHandle,
            definition: data::DataDefinition,
            is_closed: bool,
        ) {
            while self.tables.data_application_definitions.len()
                < self.type_reference_table.type_reference_count()
            {
                self.tables
                    .data_application_definitions
                    .insert(crate::DataApplicationView::default());
            }
            let row = Handle::from_parts(application.arena_index(), application.generation());
            let definition = self.tables.data_definitions.insert(definition);
            *self.tables.data_application_definitions.get_mut(row) = crate::DataApplicationView {
                definition,
                is_closed,
            };
        }

        pub fn data_application_definition(
            &self,
            application: TypeReferenceHandle,
        ) -> Option<&data::DataDefinition> {
            if !self.contains_type_reference(application) {
                return None;
            }
            let row = Handle::from_parts(application.arena_index(), application.generation());
            let definition = self.tables.data_application_definitions.get(row).definition;
            self.tables
                .data_definitions
                .is_valid(definition)
                .then(|| self.tables.data_definitions.get(definition))
        }

        pub(crate) fn data_application_is_closed(&self, application: TypeReferenceHandle) -> bool {
            let row = Handle::from_parts(application.arena_index(), application.generation());
            self.tables.data_application_definitions.get(row).is_closed
        }

        pub fn data_definition_by_name(&self, name: &Identifier) -> Option<&data::DataDefinition> {
            self.data_definitions()
                .iter()
                .find(|definition| definition.name == *name)
        }

        pub fn data_definition_by_name_str(&self, name: &str) -> Option<&data::DataDefinition> {
            self.data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name)
        }

        pub fn attached_data_definition(&self, machine: &Machine) -> Option<&data::DataDefinition> {
            if machine.attached_data_symbol.is_valid() {
                return self.data_definition_by_symbol(machine.attached_data_symbol);
            }
            let attached = machine.attached_data.as_ref()?;
            let mut matches = self
                .data_definitions()
                .iter()
                .filter(|definition| definition.name.as_str() == attached.as_str());
            let first = matches.next()?;
            matches.next().is_none().then_some(first)
        }

        crate::declaration_columns! {
            data_type_parameters(data_definition: data::DataDefinition)
                -> &[data::TypeParameter] = data_type_parameters.type_parameters;
            #[inline]
            data_members(data_definition: data::DataDefinition)
                -> &[data::DataMember] = data_members.members
                [push_data_member];
            #[inline]
            data_payload_fields(variant: data::DataVariant)
                -> &[data::DataField] = data_payload_fields.payload
                [push_data_payload_field];
            domain_type_parameters(domain: domain::DomainDefinition)
                -> &[data::TypeParameter] = data_type_parameters.type_parameters;
            domain_operators(domain: domain::DomainDefinition)
                -> &[crate::operator::OperatorDefinition] = operators.operators
                [push_domain_operator];
            proposition_binders(proposition: proposition::PropositionDefinition)
                -> &[proposition::PropositionBinder] = proposition_binders.binders
                [push_proposition_binder];
            proposition_parameters(proposition: proposition::PropositionDefinition)
                -> &[signature::StateParameter] = state_parameters.parameters
                [push_proposition_parameter];
            proof_facts(domain: domain::DomainDefinition)
                -> &[domain::ProofFact] = proof_facts.facts;
            operator_type_parameters(operator: crate::operator::OperatorDefinition)
                -> &[data::TypeParameter] = data_type_parameters.type_parameters;
            operator_parameters(operator: crate::operator::OperatorDefinition)
                -> &[signature::StateParameter] = state_parameters.parameters
                [push_operator_parameter];
            operator_contracts(operator: crate::operator::OperatorDefinition)
                -> &[signature::SignatureContract] = signature_contracts.contracts
                [push_operator_contract];
        }

        crate::declaration_columns! {
            append push_mathematical_parameter(
                definition: MathematicalDefinition,
                crate::mathematical::MathematicalParameter
            ) = mathematical_parameters.parameters;
            append push_measure_path_member(
                measure: measure::MeasureDefinition,
                Identifier
            ) = measure_path_members.name;
            append push_operator_path_member(
                operator: crate::operator::OperatorDefinition,
                Identifier
            ) = operator_path_members.name;
        }

        pub fn data_value_parameter_fields<'a>(
            &'a self,
            definition: &data::DataDefinition,
        ) -> impl Iterator<Item = (&'a data::TypeParameter, &'a data::DataField)> + 'a {
            self.data_type_parameters(definition)
                .iter()
                .filter(|parameter| matches!(parameter.kind, data::TypeParameterKind::Value { .. }))
                .zip(self.data_members(definition))
                .filter_map(|(parameter, member)| {
                    let data::DataMember::Field(field) = member else {
                        return None;
                    };
                    (parameter.name == field.name).then_some((parameter, field))
                })
        }

        pub fn data_application_value_fields(
            &self,
            reference: TypeReferenceHandle,
        ) -> Vec<(&data::TypeParameter, &data::DataField, TypeReferenceHandle)> {
            use crate::types::TypeReferenceNode;
            let table = &self.type_reference_table;
            let reference = table.peeled_type_reference(reference);
            let concrete = match table.type_reference(reference) {
                TypeReferenceNode::Named { symbol, .. } => {
                    self.unique_data_definition_by_symbol(*symbol)
                }
                _ => None,
            };
            let application = concrete
                .and_then(|data| data.generic_instance)
                .unwrap_or(reference);
            let TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = table.type_reference(application)
            else {
                return Vec::new();
            };
            let Some(template) = self.unique_data_definition_by_symbol(*base_symbol) else {
                return Vec::new();
            };
            let parameters = self.data_type_parameters(template);
            let arguments = table.type_reference_handles(*arguments);
            if parameters.len() != arguments.len() {
                return Vec::new();
            }
            let represented = self
                .data_application_definition(reference)
                .or(concrete)
                .unwrap_or(template);
            self.data_value_parameter_fields(template)
                .enumerate()
                .filter_map(|(ordinal, (binder, _))| {
                    let position = parameters
                        .iter()
                        .position(|parameter| parameter.symbol == binder.symbol)?;
                    let field = self.data_members(represented).get(ordinal)?.as_field()?;
                    Some((binder, field, arguments[position]))
                })
                .collect()
        }

        pub fn data_member_fields<'a>(
            &'a self,
            members: &'a [data::DataMember],
        ) -> impl Iterator<Item = &'a data::DataField> + 'a {
            members.iter().flat_map(|member| match member {
                data::DataMember::Field(field) => std::slice::from_ref(field),
                data::DataMember::Variant(variant) => self.data_payload_fields(variant),
            })
        }

        pub fn live_data_fields<'a>(
            &'a self,
            members: &'a [data::DataMember],
        ) -> impl Iterator<Item = &'a data::DataField> + 'a {
            members.iter().filter_map(|member| match member {
                data::DataMember::Field(field) if !field.relevance.is_erased() => Some(field),
                _ => None,
            })
        }

        pub fn data_construction_fields<'a>(
            &'a self,
            members: &'a [data::DataMember],
            selected_case: Option<symbols::SymbolHandle>,
        ) -> Vec<&'a data::DataField> {
            let mut fields: Vec<&data::DataField> = Vec::new();
            for member in members {
                match member {
                    data::DataMember::Field(field) => fields.push(field),
                    data::DataMember::Variant(variant) if selected_case == Some(variant.symbol) => {
                        fields.extend(self.data_payload_fields(variant));
                    }
                    _ => {}
                }
            }
            fields
        }

        pub fn exact_named_data_definition(
            &self,
            field_label: &str,
            type_reference: TypeReferenceHandle,
        ) -> Result<Option<&data::DataDefinition>, String> {
            if self.primitive_type_reference(type_reference).is_some() {
                return Ok(None);
            }
            let crate::types::TypeReferenceNode::Named { symbol, name } =
                self.type_reference_table.type_reference(type_reference)
            else {
                return Ok(None);
            };
            if !symbol.is_valid() {
                return Err(format!(
                    "{field_label} `{name}` has no exact nominal identity"
                ));
            }
            let mut matches = self
                .data_definitions()
                .iter()
                .filter(|definition| definition.symbol == *symbol);
            let Some(data) = matches.next() else {
                return Ok(None);
            };
            if matches.next().is_some() || data.name.as_str() != name.as_str() {
                return Err(format!(
                    "{field_label} `{name}` has ambiguous or mismatched nominal identity"
                ));
            }
            Ok(Some(data))
        }

        pub fn data_field(
            &self,
            definition: &data::DataDefinition,
            symbol: symbols::SymbolHandle,
        ) -> Option<&data::DataField> {
            self.data_members(definition).iter().find_map(|member| {
                let data::DataMember::Field(field) = member else {
                    return None;
                };
                (field.symbol == symbol).then_some(field)
            })
        }

        pub fn any_field_owner(
            &self,
            symbol: symbols::SymbolHandle,
        ) -> Option<(&data::DataDefinition, &data::DataField)> {
            if let Some(&(data_position, member_position, payload_position)) =
                self.data_field_positions_for_symbol(symbol).first()
            {
                let definition = self.data_definitions().get(data_position as usize)?;
                let member = self
                    .data_members(definition)
                    .get(member_position as usize)?;
                let field = if payload_position == u32::MAX {
                    let data::DataMember::Field(field) = member else {
                        return None;
                    };
                    field
                } else {
                    let data::DataMember::Variant(variant) = member else {
                        return None;
                    };
                    self.data_payload_fields(variant)
                        .get(payload_position as usize)?
                };
                return Some((definition, field));
            }
            None
        }

        pub fn data_variant(
            &self,
            definition: &data::DataDefinition,
            symbol: symbols::SymbolHandle,
        ) -> Option<&data::DataVariant> {
            self.data_members(definition).iter().find_map(|member| {
                let data::DataMember::Variant(variant) = member else {
                    return None;
                };
                (variant.symbol == symbol).then_some(variant)
            })
        }

        pub fn data_field_named(
            &self,
            definition: &data::DataDefinition,
            name: &str,
        ) -> Option<&data::DataField> {
            self.data_members(definition).iter().find_map(|member| {
                let data::DataMember::Field(field) = member else {
                    return None;
                };
                (field.name.as_str() == name).then_some(field)
            })
        }

        pub fn field_owner(
            &self,
            symbol: symbols::SymbolHandle,
        ) -> Option<(&data::DataDefinition, &data::DataField)> {
            for (data_position, member_position, payload_position) in
                self.data_field_positions_for_symbol(symbol)
            {
                if payload_position != u32::MAX {
                    continue;
                }
                let definition = self.data_definitions().get(data_position as usize)?;
                let data::DataMember::Field(field) = self
                    .data_members(definition)
                    .get(member_position as usize)?
                else {
                    continue;
                };
                return Some((definition, field));
            }
            None
        }

        pub fn variant_owner(
            &self,
            symbol: symbols::SymbolHandle,
        ) -> Option<(&data::DataDefinition, &data::DataVariant)> {
            let (data_position, member_position) = self.data_variant_position_for_symbol(symbol)?;
            let definition = self.data_definitions().get(data_position as usize)?;
            let data::DataMember::Variant(variant) = self
                .data_members(definition)
                .get(member_position as usize)?
            else {
                return None;
            };
            Some((definition, variant))
        }

        pub fn data_payload_field_by_symbol(
            &self,
            variant: &data::DataVariant,
            symbol: symbols::SymbolHandle,
        ) -> Option<&data::DataField> {
            self.data_payload_fields(variant)
                .iter()
                .find(|candidate| candidate.symbol == symbol)
        }

        pub fn domain_definition(
            &self,
            symbol: symbols::SymbolHandle,
        ) -> Option<&domain::DomainDefinition> {
            self.domain_position_by_symbol(symbol)
                .map(|position| &self.domain_definitions()[position])
        }

        pub fn proof_fact_source_span(
            &self,
            handle: Handle<domain::ProofFact>,
        ) -> Option<source::SourceSpan> {
            self.tables
                .proof_fact_source_spans
                .get(proof_fact_source_span_index(handle))
                .copied()
                .flatten()
        }

        pub fn set_proof_fact_source_span(
            &mut self,
            handle: Handle<domain::ProofFact>,
            source_span: source::SourceSpan,
        ) {
            let index = proof_fact_source_span_index(handle);
            self.tables.proof_fact_source_spans.resize(index + 1, None);
            self.tables.proof_fact_source_spans[index] = Some(source_span);
        }

        pub fn proposition_definition_by_symbol(
            &self,
            symbol: symbols::SymbolHandle,
        ) -> Option<&proposition::PropositionDefinition> {
            self.propositions()
                .iter()
                .find(|definition| definition.symbol == symbol)
        }

        pub fn push_mathematical_definition(&mut self, definition: MathematicalDefinition) {
            self.tables
                .mathematical_definitions
                .append_to_span(&mut self.roots.mathematical_definitions, definition);
        }

        pub fn mathematical_parameters(
            &self,
            span: HandleSpan<crate::mathematical::MathematicalParameter>,
        ) -> &[crate::mathematical::MathematicalParameter] {
            self.tables.mathematical_parameters.span_or_empty(span)
        }

        pub fn insert_mathematical_type(
            &mut self,
            ty: crate::mathematical::MathematicalType,
        ) -> crate::mathematical::MathematicalTypeHandle {
            self.tables.mathematical_types.insert(ty)
        }

        pub fn mathematical_type(
            &self,
            handle: crate::mathematical::MathematicalTypeHandle,
        ) -> &crate::mathematical::MathematicalType {
            self.tables.mathematical_types.get(handle)
        }

        pub fn domain_path_members(&self, span: HandleSpan<Identifier>) -> &[Identifier] {
            self.domain_path_members.span_or_empty(span)
        }

        pub fn measure_path_members(&self, span: HandleSpan<Identifier>) -> &[Identifier] {
            self.measure_path_members.span_or_empty(span)
        }

        pub fn operator_definition_by_symbol(
            &self,
            symbol: symbols::SymbolHandle,
        ) -> Option<&crate::operator::OperatorDefinition> {
            self.operators()
                .iter()
                .find(|definition| definition.symbol == symbol)
        }

        pub fn push_machine_token_binding(&mut self, view: crate::operator::OperatorDefinition) {
            self.tables
                .operators
                .append_to_span(&mut self.roots.machine_token_bindings, view);
        }

        pub fn machine_token_bindings(&self) -> &[crate::operator::OperatorDefinition] {
            self.tables
                .operators
                .span_or_empty(self.roots.machine_token_bindings)
        }

        pub fn machine_token_binding_by_symbol(
            &self,
            symbol: symbols::SymbolHandle,
        ) -> Option<&crate::operator::OperatorDefinition> {
            self.machine_token_bindings()
                .iter()
                .find(|binding| binding.symbol == symbol)
        }

        pub fn all_operators(
            &self,
        ) -> impl Iterator<Item = &crate::operator::OperatorDefinition> + '_ {
            self.operators().iter().chain(
                self.domain_definitions()
                    .iter()
                    .flat_map(|domain| self.domain_operators(domain)),
            )
        }

        pub fn operator_path_members(&self, span: HandleSpan<Identifier>) -> &[Identifier] {
            self.operator_path_members.span_or_empty(span)
        }

        pub fn operator_path_string(&self, span: HandleSpan<Identifier>) -> String {
            self.operator_path_members(span).joined_path()
        }
    }

    impl TypedTrees {
        pub fn retain_designated_static_type_argument(
            &mut self,
            owner: symbols::SymbolHandle,
            reference: TypeReferenceHandle,
            source_span: source::SourceSpan,
        ) {
            let row = crate::DesignatedStaticTypeArgument {
                owner,
                reference,
                source_span,
            };
            if !self
                .tables
                .designated_static_type_arguments
                .iter()
                .any(|(_, prior)| prior == &row)
            {
                self.tables.designated_static_type_arguments.insert(row);
            }
        }
        pub fn designated_static_type_arguments(
            &self,
        ) -> impl Iterator<Item = &crate::DesignatedStaticTypeArgument> {
            self.tables
                .designated_static_type_arguments
                .iter()
                .map(|(_, row)| row)
        }
    }

    #[cfg(test)]
    mod captured_evidence_retention_tests {
        use super::*;

        #[test]
        fn captured_supply_keeps_only_its_exact_application_without_executing_the_owner() {
            let mut symbols = symbols::SymbolTableBuilder::default();
            let owner = symbols.insert_root(
                symbols::SymbolKind::Machine,
                symbols::SymbolNameRef::Static("source"),
            );
            let instance = symbols.insert_root(
                symbols::SymbolKind::Machine,
                symbols::SymbolNameRef::Static("captured"),
            );
            let constructor = symbols.insert_root(
                symbols::SymbolKind::Data,
                symbols::SymbolNameRef::Static("StorageConstructor"),
            );
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            program.push_machine(crate::machine::Machine {
                symbol: instance,
                ..Default::default()
            });
            let source_application =
                program
                    .type_reference_table
                    .insert(crate::types::TypeReferenceNode::Named {
                        symbol: constructor,
                        name: crate::name::Identifier::generated("StorageConstructor"),
                    });
            let supplying = program.expression_table.retain_static_machine_application(
                constructor,
                Default::default(),
                &[],
            );
            program
                .expression_table
                .static_machine_application_mut(supplying)
                .lexical_owner = owner;
            let application = crate::ClosedConformanceApplication {
                trait_arguments: vec!["exact".to_owned()],
                ..Default::default()
            };
            let selected = crate::DataImplementationApplication {
                source_application,
                lexical_owner: owner,
                supplying_application: supplying,
                application: application.clone(),
                ..Default::default()
            };
            program
                .data_implementation_applications
                .insert(selected.clone());
            let mut wrong_supplier = selected.clone();
            wrong_supplier.supplying_application =
                crate::static_applications::StaticMachineApplicationHandle::from_parts(3, 2);
            program
                .data_implementation_applications
                .insert(wrong_supplier);
            let mut wrong_application = selected.clone();
            wrong_application.application.trait_arguments = vec!["other".to_owned()];
            program
                .data_implementation_applications
                .insert(wrong_application);
            program
                .captured_machine_applications
                .insert(crate::CapturedMachineApplication {
                    instance,
                    supplying_application: supplying,
                    conformance_application: application,
                    ..Default::default()
                });
            let removed = std::collections::HashSet::from([owner]);
            program
                .retain_live_trait_implementation_applications(|symbol| removed.contains(&symbol));
            assert!(program.machine_by_symbol(owner).is_none());
            assert_eq!(program.data_implementation_applications.len(), 1);
            assert_eq!(
                program
                    .data_implementation_applications
                    .iter()
                    .next()
                    .unwrap()
                    .1,
                &selected
            );

            let references = crate::references::collect_program_references(&program);
            assert!(references.symbols.contains(&constructor));
            assert!(references.descriptor_symbols.contains(&owner));
            assert!(!references.symbols.contains(&owner));

            program.roots.machines = Default::default();
            program
                .retain_live_trait_implementation_applications(|symbol| removed.contains(&symbol));
            assert!(program.data_implementation_applications.is_empty());
        }
    }
}

pub(crate) mod machine_declarations {
    use crate::declaration_index::SymbolRows;
    use crate::machine::Machine;
    use arena::{Arena, Handle, HandleSpan, OwnedIndex};
    use symbols::SymbolHandle;

    pub(crate) type MachinePositionIndex = SymbolRows<u32>;

    #[derive(Clone, Default)]
    struct RetainedDeclarationIndex {
        rows: SymbolRows<Option<Handle<Machine>>>,
    }

    fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
        (symbol.arena_index(), symbol.generation())
    }

    impl RetainedDeclarationIndex {
        fn build(arena: &Arena<Machine>) -> Self {
            let mut declarations = arena
                .iter()
                .map(|(handle, machine)| (machine.symbol, handle))
                .collect::<Vec<_>>();
            declarations.sort_by_key(|(symbol, _)| symbol_key(*symbol));
            let mut rows: Vec<(SymbolHandle, Option<Handle<Machine>>)> = Vec::new();
            for (symbol, handle) in declarations {
                if let Some((previous, selected)) = rows.last_mut()
                    && *previous == symbol
                {
                    if selected.is_some_and(|first| arena.get(first) != arena.get(handle)) {
                        *selected = None;
                    }
                } else {
                    rows.push((symbol, Some(handle)));
                }
            }
            Self {
                rows: SymbolRows::new(rows),
            }
        }

        fn declaration(&self, symbol: SymbolHandle) -> Option<Handle<Machine>> {
            *self.rows.first(symbol)?
        }
    }

    #[derive(Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct MachineDeclarationTable {
        arena: Arena<Machine>,
        roots: OwnedIndex<MachinePositionIndex, HandleSpan<Machine>>,
        retained: OwnedIndex<RetainedDeclarationIndex, ()>,
    }

    impl From<Arena<Machine>> for MachineDeclarationTable {
        fn from(arena: Arena<Machine>) -> Self {
            Self {
                arena,
                roots: OwnedIndex::default(),
                retained: OwnedIndex::default(),
            }
        }
    }

    impl std::fmt::Debug for MachineDeclarationTable {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.arena, formatter)
        }
    }

    impl std::ops::Deref for MachineDeclarationTable {
        type Target = Arena<Machine>;

        fn deref(&self) -> &Self::Target {
            &self.arena
        }
    }

    impl std::ops::DerefMut for MachineDeclarationTable {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.roots.clear();
            self.retained.clear();
            &mut self.arena
        }
    }

    impl MachineDeclarationTable {
        pub(crate) fn with_root_index<Result>(
            &self,
            roster: HandleSpan<Machine>,
            read: impl FnOnce(&MachinePositionIndex) -> Result,
        ) -> Result {
            self.roots.with(
                roster,
                || {
                    SymbolRows::new(
                        self.arena
                            .span_or_empty(roster)
                            .iter()
                            .enumerate()
                            .map(|(position, machine)| (machine.symbol, position as u32))
                            .collect(),
                    )
                },
                read,
            )
        }

        pub(crate) fn retained_declaration(&self, symbol: SymbolHandle) -> Option<&Machine> {
            let handle = self.retained.with(
                (),
                || RetainedDeclarationIndex::build(&self.arena),
                |index| index.declaration(symbol),
            )?;
            Some(self.arena.get(handle))
        }

        pub(crate) fn seal(&mut self) {
            self.retained
                .with((), || RetainedDeclarationIndex::build(&self.arena), |_| ());
            self.roots.seal_with(SymbolRows::densify);
            self.retained.seal_with(|index| index.rows.densify());
        }

        pub(crate) fn unseal(&mut self) {
            self.roots.unseal();
            self.retained.unseal();
        }
    }
}

pub(crate) mod machine_parameter_signatures {
    use crate::TypedTrees;
    use crate::declaration_tables::LifetimeTemplateCandidates;
    use crate::{machine, signature};
    use symbols::SymbolHandle;

    pub struct MachineParameterSignatureQueries<'program> {
        program: &'program TypedTrees,
        scope_parents: symbols::SymbolKeyMap<SymbolHandle, SymbolHandle>,
        templates: symbols::SymbolKeyMap<SymbolHandle, LifetimeTemplateCandidates>,
    }

    impl TypedTrees {
        pub fn machine_parameter_signature_queries(&self) -> MachineParameterSignatureQueries<'_> {
            let mut templates: symbols::SymbolKeyMap<SymbolHandle, LifetimeTemplateCandidates> =
                Default::default();
            for (instance, template) in self
                .machine_specializations
                .iter()
                .map(|application| (application.instance, application.template))
                .chain(
                    self.captured_machine_applications
                        .iter()
                        .map(|(_, application)| (application.instance, application.template)),
                )
            {
                templates.entry(instance).or_default().include(template);
            }
            MachineParameterSignatureQueries {
                program: self,
                scope_parents: Default::default(),
                templates,
            }
        }
    }

    impl<'program> MachineParameterSignatureQueries<'program> {
        pub fn get(
            &mut self,
            symbol: SymbolHandle,
        ) -> Option<(
            &'program machine::Machine,
            &'program signature::StateSignature,
        )> {
            let program = self.program;
            program.retained_static_machine_contract(symbol)?;
            let parents = &mut self.scope_parents;
            let templates = &self.templates;
            program.machines().iter().find_map(|machine| {
                program
                    .machine_parameter_signature_in_with_scope_parents(machine, symbol, |owner| {
                        *parents.entry(owner).or_insert_with(|| {
                            let normalized = program.lifetime_declaration_owner_with_templates(
                                owner,
                                |owner| templates.get(&owner).copied().unwrap_or_default(),
                                |symbol| program.retained_static_machine_contract(symbol).is_some(),
                            );
                            program.declaration_scope_parent_for_lifetime_owner(normalized)
                        })
                    })
                    .map(|signature| (machine, signature))
            })
        }
    }

    #[cfg(test)]
    pub(crate) mod tests {
        use super::{MachineParameterSignatureQueries, SymbolHandle, TypedTrees};
        use crate::MachineSpecialization;
        use crate::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
        use crate::name::Identifier;
        use crate::signature::StateSignature;
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

        fn symbol(
            symbols: &mut SymbolTableBuilder,
            kind: SymbolKind,
            name: &'static str,
        ) -> SymbolHandle {
            symbols.insert_root(kind, SymbolNameRef::Static(name))
        }

        fn children(
            symbols: &mut SymbolTableBuilder,
            owner: SymbolHandle,
            members: &[(SymbolKind, &'static str)],
        ) -> Vec<SymbolHandle> {
            symbols
                .insert_children(
                    owner,
                    members
                        .iter()
                        .map(|(kind, name)| (*kind, SymbolNameRef::Static(name))),
                )
                .handles()
                .collect()
        }

        fn parameter(
            symbol: SymbolHandle,
            name: &'static str,
            blocking: Option<bool>,
        ) -> TypeParameter {
            TypeParameter {
                symbol,
                name: Identifier::from(name),
                kind: blocking.map_or(TypeParameterKind::Type, |blocks| {
                    TypeParameterKind::Machine {
                        contract: MachineParameterContract::Structural(StateSignature {
                            symbol,
                            blocks,
                            ..StateSignature::default()
                        }),
                    }
                }),
                ..TypeParameter::default()
            }
        }

        fn machine(program: &mut TypedTrees, symbol: SymbolHandle, parameters: Vec<TypeParameter>) {
            let type_parameters = program.data_type_parameters.insert_many(parameters);
            program.push_machine(crate::machine::Machine {
                symbol,
                type_parameters,
                ..crate::machine::Machine::default()
            });
        }

        fn same(
            program: &TypedTrees,
            queries: &mut MachineParameterSignatureQueries<'_>,
            symbol: SymbolHandle,
        ) {
            match (
                program.machine_parameter_signature(symbol),
                queries.get(symbol),
            ) {
                (Some((old_owner, old_signature)), Some((owner, signature))) => {
                    assert!(std::ptr::eq(old_owner, owner));
                    assert!(std::ptr::eq(old_signature, signature));
                }
                (None, None) => {}
                _ => panic!("batched lookup changed the exact owner or signature for {symbol:?}"),
            }
        }

        #[test]
        fn batch_preserves_first_owner_generations_and_negative_questions() {
            let mut symbols = SymbolTableBuilder::default();
            let first = symbol(&mut symbols, SymbolKind::Machine, "first");
            let second = symbol(&mut symbols, SymbolKind::Machine, "second");
            let first_members = children(
                &mut symbols,
                first,
                &[
                    (SymbolKind::TypeParameter, "T"),
                    (SymbolKind::TypeParameter, "Shared"),
                ],
            );
            let (ty, shared) = (first_members[0], first_members[1]);
            let selected = children(
                &mut symbols,
                second,
                &[(SymbolKind::TypeParameter, "Selected")],
            )[0];
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..TypedTrees::default()
            };
            machine(
                &mut program,
                first,
                vec![
                    parameter(ty, "T", None),
                    parameter(shared, "Shared", Some(false)),
                ],
            );
            machine(
                &mut program,
                second,
                vec![
                    parameter(shared, "Shared", Some(true)),
                    parameter(selected, "Selected", Some(true)),
                ],
            );
            let mut queries = program.machine_parameter_signature_queries();
            same(&program, &mut queries, ty);
            same(
                &program,
                &mut queries,
                SymbolHandle::from_parts(selected.arena_index(), selected.generation() + 1),
            );
            assert!(queries.scope_parents.is_empty());
            for _ in 0..8 {
                same(&program, &mut queries, shared);
                same(&program, &mut queries, selected);
            }
            assert_eq!(queries.get(shared).unwrap().0.symbol, first);
            assert!(!queries.get(shared).unwrap().1.blocks);
            assert_eq!(queries.get(selected).unwrap().0.symbol, second);
            assert!(queries.get(selected).unwrap().1.blocks);
            assert_eq!(queries.scope_parents.len(), 1);
            drop(queries);
            let mut projection = program.clone();
            projection.roots.machines = arena::HandleSpan::empty();
            let mut projected = projection.machine_parameter_signature_queries();
            same(&projection, &mut projected, selected);
            assert!(projected.get(selected).is_none());
        }

        #[test]
        fn batch_keeps_conformance_shadowing_and_first_visible_owner() {
            let mut symbols = SymbolTableBuilder::default();
            let map = symbol(&mut symbols, SymbolKind::Conformance, "Map");
            let members = children(
                &mut symbols,
                map,
                &[
                    (SymbolKind::Machine, "hidden"),
                    (SymbolKind::Machine, "visible"),
                    (SymbolKind::TypeParameter, "Callback"),
                ],
            );
            let (hidden, visible, callback) = (members[0], members[1], members[2]);
            let shadow = children(
                &mut symbols,
                hidden,
                &[(SymbolKind::TypeParameter, "Callback")],
            )[0];
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..TypedTrees::default()
            };
            let type_parameters = program.data_type_parameters.insert_many([parameter(
                callback,
                "Callback",
                Some(true),
            )]);
            program.push_conformance(crate::trait_definition::Conformance {
                symbol: map,
                type_parameters,
                ..crate::trait_definition::Conformance::default()
            });
            machine(
                &mut program,
                hidden,
                vec![parameter(shadow, "Callback", None)],
            );
            machine(&mut program, visible, Vec::new());
            let mut queries = program.machine_parameter_signature_queries();
            for _ in 0..8 {
                same(&program, &mut queries, callback);
            }
            let (owner, signature) = queries.get(callback).unwrap();
            assert_eq!(owner.symbol, visible);
            assert!(signature.blocks);
            assert_eq!(queries.scope_parents.len(), 2);
        }

        #[test]
        fn batches_do_not_carry_answers_across_changed_specialization_ancestry() {
            let mut symbols = SymbolTableBuilder::default();
            let map = symbol(&mut symbols, SymbolKind::Conformance, "Map");
            let members = children(
                &mut symbols,
                map,
                &[
                    (SymbolKind::Machine, "template"),
                    (SymbolKind::TypeParameter, "Callback"),
                ],
            );
            let (template, callback) = (members[0], members[1]);
            let instance = symbol(&mut symbols, SymbolKind::Machine, "instance");
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..TypedTrees::default()
            };
            let type_parameters = program.data_type_parameters.insert_many([parameter(
                callback,
                "Callback",
                Some(true),
            )]);
            program.push_conformance(crate::trait_definition::Conformance {
                symbol: map,
                type_parameters,
                ..crate::trait_definition::Conformance::default()
            });
            machine(&mut program, instance, Vec::new());
            program.machine_specializations.push(MachineSpecialization {
                instance,
                template,
                ..MachineSpecialization::default()
            });
            {
                let mut queries = program.machine_parameter_signature_queries();
                same(&program, &mut queries, callback);
                assert!(queries.get(callback).is_some());
            }
            program.machine_specializations[0].template = instance;
            {
                let mut queries = program.machine_parameter_signature_queries();
                same(&program, &mut queries, callback);
                assert!(queries.get(callback).is_none());
            }
            program.machine_specializations[0].template = template;
            {
                let mut queries = program.machine_parameter_signature_queries();
                same(&program, &mut queries, callback);
                assert!(queries.get(callback).is_some());
            }
            let duplicate = program.machine_specializations[0].clone();
            program.machine_specializations.push(duplicate);
            let mut queries = program.machine_parameter_signature_queries();
            same(&program, &mut queries, callback);
            assert!(queries.get(callback).is_none());
        }

        #[test]
        fn first_viewless_contract_does_not_hide_a_later_retained_view() {
            let mut symbols = SymbolTableBuilder::default();
            let first = symbol(&mut symbols, SymbolKind::Machine, "first");
            let second = symbol(&mut symbols, SymbolKind::Machine, "second");
            let callback = children(
                &mut symbols,
                first,
                &[(SymbolKind::TypeParameter, "Callback")],
            )[0];
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..TypedTrees::default()
            };
            let mut identity_only = parameter(callback, "Callback", Some(false));
            identity_only.kind = TypeParameterKind::Machine {
                contract: MachineParameterContract::RequirementIdentity,
            };
            machine(&mut program, first, vec![identity_only]);
            machine(
                &mut program,
                second,
                vec![parameter(callback, "Callback", Some(true))],
            );
            assert!(
                program
                    .retained_static_machine_contract(callback)
                    .unwrap()
                    .signature()
                    .blocks
            );
            {
                let mut queries = program.machine_parameter_signature_queries();
                same(&program, &mut queries, callback);
                assert_eq!(queries.get(callback).unwrap().0.symbol, second);
            }
            let later = program.machines()[1].type_parameters.start();
            program.data_type_parameters.get_mut(later).kind = TypeParameterKind::Machine {
                contract: MachineParameterContract::RequirementIdentity,
            };
            assert!(program.retained_static_machine_contract(callback).is_none());
            let mut queries = program.machine_parameter_signature_queries();
            same(&program, &mut queries, callback);
            assert!(queries.get(callback).is_none());
        }

        #[test]
        fn logical_callable_derivation_uses_any_retained_view_and_refreshes_after_mutation() {
            let mut symbols = SymbolTableBuilder::default();
            let map = symbol(&mut symbols, SymbolKind::Conformance, "Map");
            let original = children(
                &mut symbols,
                map,
                &[(SymbolKind::TypeParameter, "Callback")],
            )[0];
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..TypedTrees::default()
            };
            let derived = program.symbols.insert_generated_root_from(
                original,
                SymbolKind::Machine,
                "derived",
            );
            let type_parameters = program.data_type_parameters.insert_many([parameter(
                original,
                "Callback",
                Some(true),
            )]);
            program.push_conformance(crate::trait_definition::Conformance {
                symbol: map,
                type_parameters,
                ..Default::default()
            });
            let mut identity_only = parameter(derived, "derived", Some(false));
            identity_only.kind = TypeParameterKind::Machine {
                contract: MachineParameterContract::RequirementIdentity,
            };
            program.data_type_parameters.insert(identity_only);
            let later =
                program
                    .data_type_parameters
                    .insert(parameter(derived, "derived", Some(false)));
            machine(&mut program, derived, Vec::new());
            assert_eq!(program.lifetime_declaration_owner(derived), original);
            {
                let mut queries = program.machine_parameter_signature_queries();
                same(&program, &mut queries, original);
                assert!(queries.get(original).unwrap().1.blocks);
            }
            program.data_type_parameters.get_mut(later).kind = TypeParameterKind::Machine {
                contract: MachineParameterContract::RequirementIdentity,
            };
            assert_eq!(program.lifetime_declaration_owner(derived), derived);
            let mut queries = program.machine_parameter_signature_queries();
            same(&program, &mut queries, original);
            assert!(queries.get(original).is_none());
        }
    }
}

pub(crate) mod spelled_declarations {
    use arena::Arena;
    use std::ops::{Deref, DerefMut};
    use std::sync::Arc;

    #[derive(Clone, Default)]
    pub(crate) struct DeclarationStorageIdentity(Arc<()>);

    impl PartialEq for DeclarationStorageIdentity {
        fn eq(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.0, &other.0)
        }
    }
    impl Eq for DeclarationStorageIdentity {}

    #[derive(Clone)]
    pub struct SpelledDeclarationTable<T> {
        arena: Arena<T>,
        identity: DeclarationStorageIdentity,
    }

    impl<T: Default> Default for SpelledDeclarationTable<T> {
        fn default() -> Self {
            Arena::default().into()
        }
    }
    impl<T> From<Arena<T>> for SpelledDeclarationTable<T> {
        fn from(arena: Arena<T>) -> Self {
            Self {
                arena,
                identity: DeclarationStorageIdentity::default(),
            }
        }
    }
    impl<T> Deref for SpelledDeclarationTable<T> {
        type Target = Arena<T>;
        fn deref(&self) -> &Self::Target {
            &self.arena
        }
    }
    impl<T> DerefMut for SpelledDeclarationTable<T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.identity = DeclarationStorageIdentity::default();
            &mut self.arena
        }
    }
    impl<T> SpelledDeclarationTable<T> {
        pub(crate) fn storage_identity(&self) -> DeclarationStorageIdentity {
            self.identity.clone()
        }
    }
    impl<T: PartialEq> PartialEq for SpelledDeclarationTable<T> {
        fn eq(&self, other: &Self) -> bool {
            self.arena == other.arena
        }
    }
    impl<T: Eq> Eq for SpelledDeclarationTable<T> {}
    impl<T: std::fmt::Debug> std::fmt::Debug for SpelledDeclarationTable<T> {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.arena, formatter)
        }
    }

    impl<T: compiler_storage::Record> compiler_storage::Record for SpelledDeclarationTable<T> {
        fn write(
            &self,
            output: &mut compiler_storage::RecordWriter,
        ) -> Result<(), compiler_storage::RecordError> {
            compiler_storage::Record::write(&self.arena, output)
        }
        fn read<Context>(
            input: &mut compiler_storage::RecordReader<'_, Context>,
        ) -> Result<Self, compiler_storage::RecordError> {
            <Arena<T> as compiler_storage::Record>::read(input).map(Self::from)
        }
    }
}

pub(crate) mod table_reads {
    use crate::expression::StaticMachineArgument;
    use crate::static_applications::{
        StaticMachineApplication, StaticMachineApplicationHandle, StaticMachineBinding,
        StaticMachineLifetimeBinding,
    };
    use crate::types::{TypeReferenceHandle, TypeReferenceNode};
    use arena::HandleSpan;

    impl crate::TypedTreeTables {
        pub(crate) fn type_reference_handles(
            &self,
            span: HandleSpan<TypeReferenceHandle>,
        ) -> &[TypeReferenceHandle] {
            self.type_reference_table.type_reference_handles(span)
        }

        pub(crate) fn type_reference_node(
            &self,
            handle: TypeReferenceHandle,
        ) -> &TypeReferenceNode {
            self.type_reference_table.type_reference(handle)
        }

        pub(crate) fn contains_type_reference(&self, handle: TypeReferenceHandle) -> bool {
            self.type_reference_table.contains_type_reference(handle)
        }

        pub(crate) fn static_machine_application(
            &self,
            application: StaticMachineApplicationHandle,
        ) -> &StaticMachineApplication {
            self.expression_table
                .static_machine_application(application)
        }

        pub(crate) fn static_machine_application_arguments(
            &self,
            application: StaticMachineApplicationHandle,
        ) -> &[StaticMachineArgument] {
            self.expression_table
                .static_machine_application_arguments(application)
        }

        pub(crate) fn static_machine_application_bindings(
            &self,
            application: StaticMachineApplicationHandle,
        ) -> &[StaticMachineBinding] {
            self.expression_table
                .static_machine_application_bindings(application)
        }

        pub(crate) fn static_machine_application_lifetimes(
            &self,
            application: StaticMachineApplicationHandle,
        ) -> &[StaticMachineLifetimeBinding] {
            self.expression_table
                .static_machine_application_lifetimes(application)
        }
    }
}

pub(crate) mod trait_and_machine_tables {
    use crate::TypedTrees;
    use crate::name::Identifier;
    use crate::state::state_by_symbol;
    use crate::statement::StatementNode::LocalData;
    use crate::{data, machine, signature, trait_definition};
    use symbols::SymbolHandle;

    impl TypedTrees {
        pub fn target_family_member(
            &self,
            head: SymbolHandle,
            target: &str,
        ) -> Option<&machine::Machine> {
            let family = self
                .target_machine_families
                .iter()
                .find(|family| family.canonical == head)?;
            let member = family
                .members
                .iter()
                .find(|member| member.target.as_str() == target)?;
            self.machine_by_symbol(member.machine)
        }

        pub fn machine_for_target<'program>(
            &'program self,
            machine: &'program machine::Machine,
            target: &str,
        ) -> Option<&'program machine::Machine> {
            if machine.target_family_interface {
                return self.target_family_member(machine.symbol, target);
            }
            self.machine_authored_target(machine)
                .is_none_or(|declared| declared == target)
                .then_some(machine)
        }

        crate::typed_tree_table! {
            traits => traits(trait_definition::TraitDefinition) [push_trait_definition];
            conformances => conformances(trait_definition::Conformance) [push_conformance];
            machines => machines(machine::Machine) [push_machine], inline;
        }

        pub fn trait_definition_by_symbol(
            &self,
            symbol: symbols::SymbolHandle,
        ) -> Option<&trait_definition::TraitDefinition> {
            self.with_call_target_index(|index| index.trait_by_symbol.get(&symbol).copied())
                .map(|position| &self.traits()[position as usize])
        }

        pub fn conformance_by_symbol(
            &self,
            symbol: symbols::SymbolHandle,
        ) -> Option<&trait_definition::Conformance> {
            self.conformances()
                .iter()
                .find(|conformance| conformance.symbol == symbol)
        }

        /*
            The value identity of an implementation about to join the table.
            Only selection rows leave the constructor's binders out of the
            fields, so only then can a binder be unused or two names construct
            one value; every form this walk does not read counts as a use.
        */
        pub fn implementation_value_identity(
            &self,
            conformance: &trait_definition::Conformance,
        ) -> trait_definition::ImplementationValueIdentity {
            use crate::types::{TypeReferenceHandle, TypeReferenceNode};
            if conformance
                .rows
                .iter()
                .any(|row| row.source != trait_definition::ConformanceRowSource::Reference)
            {
                return Default::default();
            }
            /*
                One entry per node of the schema arguments in written order, so
                two argument lists are the same application exactly when their
                shapes are equal. A primitive has no declaration and is told
                apart by its name.
            */
            #[derive(PartialEq, Eq)]
            enum Shape {
                Declared(symbols::SymbolHandle),
                Primitive(String),
                Slice,
                Unit,
            }
            struct Uses<'program> {
                program: &'program TypedTrees,
                shape: Vec<Shape>,
                lifetimes: Vec<String>,
                closed: bool,
            }
            impl Uses<'_> {
                fn read(&mut self, reference: TypeReferenceHandle) {
                    match self.program.type_reference_table.type_reference(reference) {
                        TypeReferenceNode::Named { symbol, name } => {
                            self.shape.push(if symbol.is_valid() {
                                Shape::Declared(*symbol)
                            } else {
                                Shape::Primitive(name.as_str().to_owned())
                            })
                        }
                        TypeReferenceNode::Slice { element_type } => {
                            self.shape.push(Shape::Slice);
                            self.read(*element_type);
                        }
                        TypeReferenceNode::Reference {
                            referee, lifetime, ..
                        } => {
                            self.lifetimes
                                .extend(lifetime.iter().map(|name| name.as_str().to_owned()));
                            self.closed &= lifetime.is_some();
                            self.read(*referee);
                        }
                        TypeReferenceNode::Generic {
                            base_symbol,
                            lifetime_arguments,
                            arguments,
                            ..
                        } => {
                            self.shape.push(Shape::Declared(*base_symbol));
                            self.lifetimes.extend(
                                lifetime_arguments
                                    .iter()
                                    .map(|name| name.as_str().to_owned()),
                            );
                            for argument in self.program.type_reference_handles(*arguments) {
                                self.read(*argument);
                            }
                        }
                        TypeReferenceNode::Unit => self.shape.push(Shape::Unit),
                        TypeReferenceNode::Constrained { .. }
                        | TypeReferenceNode::FixedArray { .. }
                        | TypeReferenceNode::ConstExpression(_)
                        | TypeReferenceNode::Callable { .. }
                        | TypeReferenceNode::DynamicTrait { .. } => self.closed = false,
                    }
                }
            }
            let uses = |conformance: &trait_definition::Conformance| {
                let mut uses = Uses {
                    program: self,
                    shape: Vec::new(),
                    lifetimes: Vec::new(),
                    closed: true,
                };
                for argument in self.type_reference_handles(conformance.arguments) {
                    uses.read(*argument);
                }
                uses
            };
            let own = uses(conformance);
            if !own.closed {
                return Default::default();
            }
            let unused_lifetimes = conformance
                .lifetime_parameters
                .iter()
                .enumerate()
                .map(|(ordinal, lifetime)| {
                    !conformance
                        .trait_lifetime_arguments
                        .iter()
                        .any(|argument| *argument as usize == ordinal)
                        && !own.lifetimes.iter().any(|name| name == lifetime.as_str())
                })
                .collect::<Vec<_>>();
            let unused_parameters = self
                .data_type_parameters
                .span_or_empty(conformance.type_parameters)
                .iter()
                .map(|parameter| !own.shape.contains(&Shape::Declared(parameter.symbol)))
                .collect::<Vec<_>>();
            let all_unused = |identity: &trait_definition::ImplementationValueIdentity| {
                identity
                    .unused_lifetimes
                    .iter()
                    .chain(&identity.unused_parameters)
                    .all(|unused| *unused)
            };
            let mut identity = trait_definition::ImplementationValueIdentity {
                representative: symbols::SymbolHandle::invalid(),
                unused_lifetimes,
                unused_parameters,
            };
            if all_unused(&identity) && own.lifetimes.is_empty() {
                identity.representative =
                    self.conformances()
                        .iter()
                        .find(|earlier| {
                            let known = &earlier.value_identity;
                            !known.representative.is_valid()
                                && known.unused_parameters.len()
                                    == self.conformance_type_parameters(earlier).len()
                                && known.unused_lifetimes.len() == earlier.lifetime_parameters.len()
                                && all_unused(known)
                                && earlier.trait_symbol == conformance.trait_symbol
                                && earlier.rows.len() == conformance.rows.len()
                                && earlier.rows.iter().zip(&conformance.rows).all(
                                    |(left, right)| {
                                        left.source
                                            == trait_definition::ConformanceRowSource::Reference
                                            && left.declaring_trait == right.declaring_trait
                                            && left.requirement == right.requirement
                                            && left.realization_machine == right.realization_machine
                                            && left.realization_state == right.realization_state
                                    },
                                )
                                && {
                                    let earlier = uses(earlier);
                                    earlier.closed
                                        && earlier.lifetimes.is_empty()
                                        && earlier.shape == own.shape
                                }
                        })
                        .map_or_else(symbols::SymbolHandle::invalid, |earlier| earlier.symbol);
            }
            identity
        }

        /* The declaration whose name stands for the value `symbol` constructs. */
        pub fn implementation_value_head(
            &self,
            symbol: symbols::SymbolHandle,
        ) -> symbols::SymbolHandle {
            self.implementation_value_identity_of(symbol)
                .map(|identity| identity.representative)
                .filter(|representative| representative.is_valid())
                .unwrap_or(symbol)
        }

        /* Whether the argument at a binder position is part of the constructed value. */
        pub fn application_lifetime_is_relevant(
            &self,
            symbol: symbols::SymbolHandle,
            ordinal: usize,
        ) -> bool {
            !self
                .implementation_value_identity_of(symbol)
                .and_then(|identity| identity.unused_lifetimes.get(ordinal))
                .is_some_and(|unused| *unused)
        }

        pub fn application_argument_is_relevant(
            &self,
            symbol: symbols::SymbolHandle,
            ordinal: usize,
        ) -> bool {
            !self
                .implementation_value_identity_of(symbol)
                .and_then(|identity| identity.unused_parameters.get(ordinal))
                .is_some_and(|unused| *unused)
        }

        fn implementation_value_identity_of(
            &self,
            symbol: symbols::SymbolHandle,
        ) -> Option<&trait_definition::ImplementationValueIdentity> {
            (symbol.is_valid() && self.symbols.get(symbol).kind == symbols::SymbolKind::Conformance)
                .then(|| self.conformance_by_symbol(symbol))
                .flatten()
                .map(|conformance| &conformance.value_identity)
        }

        crate::declaration_columns! {
            conformance_type_parameters(conformance: trait_definition::Conformance)
                -> &[data::TypeParameter] = data_type_parameters.type_parameters;
            trait_type_parameters(trait_definition: trait_definition::TraitDefinition)
                -> &[data::TypeParameter] = data_type_parameters.type_parameters;
            trait_requirements(trait_definition: trait_definition::TraitDefinition)
                -> &[trait_definition::TraitRequirement] = trait_requirements.requires
                [push_trait_requirement];
            trait_machine_signatures(trait_definition: trait_definition::TraitDefinition)
                -> &[signature::StateSignature] = trait_machine_signatures.machines
                [push_trait_machine_signature];
            #[inline]
            machine_type_parameters(machine: machine::Machine)
                -> &[data::TypeParameter] = data_type_parameters.type_parameters;
            machine_owned_data(machine: machine::Machine)
                -> &[machine::OwnedData] = machine_owned_data.owned_data
                [push_machine_owned_data];
            machine_trait_conformances(machine: machine::Machine)
                -> &[machine::TraitConformance] = machine_trait_conformances.satisfies
                [push_machine_trait_conformance];
            machine_invokes(machine: machine::Machine)
                -> &[signature::AuthoredInvocation] = signature_invokes.invokes
                [push_machine_invoke];
            machine_contracts(machine: machine::Machine)
                -> &[signature::SignatureContract] = signature_contracts.contracts
                [push_machine_contract];
            #[inline]
            machine_states(machine: machine::Machine)
                -> &[crate::state::State] = machine_states.states
                [push_machine_state];
            #[inline]
            state_parameters(state: crate::state::State)
                -> &[signature::StateParameter] = state_parameters.parameters
                [push_state_parameter];
            #[inline]
            state_contracts(state: crate::state::State)
                -> &[signature::SignatureContract] = signature_contracts.contracts
                [push_state_contract];
            state_signature_parameters(signature: signature::StateSignature)
                -> &[signature::StateParameter] = state_parameters.parameters
                [push_state_signature_parameter];
            state_signature_type_parameters(signature: signature::StateSignature)
                -> &[data::TypeParameter] = data_type_parameters.type_parameters;
            state_signature_invokes(signature: signature::StateSignature)
                -> &[signature::AuthoredInvocation] = signature_invokes.invokes
                [push_state_signature_invoke];
            state_signature_contracts(signature: signature::StateSignature)
                -> &[signature::SignatureContract] = signature_contracts.contracts
                [push_state_signature_contract];
        }

        pub fn trait_machine_instances<'program>(
            &'program self,
            trait_definition: &'program trait_definition::TraitDefinition,
        ) -> impl Iterator<Item = &'program signature::StateSignature> {
            self.trait_machine_signatures(trait_definition)
                .iter()
                .flat_map(|signature| self.requirement_machine_instances(signature))
        }

        pub fn requirement_machine_instances<'program>(
            &'program self,
            signature: &'program signature::StateSignature,
        ) -> impl Iterator<Item = &'program signature::StateSignature> {
            let instances = self.closed_requirement_instance_handles_for_template(signature.symbol);
            let has_instances = !instances.is_empty();
            std::iter::once(signature)
                .filter(move |_| !has_instances)
                .chain(
                    instances.into_iter().map(move |handle| {
                        self.applied_callable_signatures.get(handle).signature()
                    }),
                )
        }

        pub fn executable_trait_machine_instances<'program>(
            &'program self,
            definition: &'program trait_definition::TraitDefinition,
        ) -> impl Iterator<Item = &'program signature::StateSignature> {
            self.trait_machine_instances(definition)
                .filter(|signature| self.executable_requirement_telescope(signature))
        }

        pub fn executable_requirement_machine_instances<'program>(
            &'program self,
            signature: &'program signature::StateSignature,
        ) -> impl Iterator<Item = &'program signature::StateSignature> {
            self.requirement_machine_instances(signature)
                .filter(|signature| self.executable_requirement_telescope(signature))
        }

        fn executable_requirement_telescope(&self, signature: &signature::StateSignature) -> bool {
            self.state_signature_type_parameters(signature)
                .iter()
                .all(|parameter| {
                    !matches!(
                        parameter.kind,
                        data::TypeParameterKind::Type
                            | data::TypeParameterKind::Const { .. }
                            | data::TypeParameterKind::Value { .. }
                    )
                })
        }

        pub fn trait_machine_signature_by_symbol(
            &self,
            trait_definition: &trait_definition::TraitDefinition,
            symbol: symbols::SymbolHandle,
        ) -> Option<&signature::StateSignature> {
            self.trait_machine_signatures(trait_definition)
                .iter()
                .find(|candidate| candidate.symbol == symbol)
                .or_else(|| {
                    self.trait_definition_by_symbol(trait_definition.symbol)?
                        .refinement_clauses
                        .iter()
                        .map(|clause| &clause.signature)
                        .find(|signature| signature.symbol == symbol)
                })
                .or_else(|| {
                    self.closed_requirement_application(symbol)
                        .filter(|application| {
                            application.declaring_trait == trait_definition.symbol
                        })
                        .map(|application| application.signature)
                })
        }

        pub fn machine_parameter_contract_view<'program>(
            &'program self,
            contract: &'program data::MachineParameterContract,
        ) -> Option<data::MachineParameterContractView<'program>> {
            match contract {
                data::MachineParameterContract::RequirementIdentity => None,
                data::MachineParameterContract::Structural(signature) => {
                    Some(data::MachineParameterContractView::Structural(signature))
                }
                data::MachineParameterContract::Nominal {
                    trait_definition,
                    requirement,
                    instantiated,
                    ..
                } => {
                    let trait_definition = self.trait_definition_by_symbol(*trait_definition)?;
                    let declared =
                        self.trait_machine_signature_by_symbol(trait_definition, *requirement)?;
                    Some(data::MachineParameterContractView::Nominal {
                        trait_definition,
                        requirement: instantiated.as_deref().unwrap_or(declared),
                    })
                }
            }
        }

        pub fn realized_machine_named(&self, name: &str) -> Option<&machine::Machine> {
            self.machines()
                .iter()
                .find(|machine| machine.target.is_none() && machine.name.as_str() == name)
        }

        pub fn machine_states_named(
            &self,
            symbol: SymbolHandle,
        ) -> impl Iterator<Item = (&machine::Machine, &crate::state::State)> + '_ {
            self.state_positions_for_symbol(symbol).into_iter().map(
                |(machine_position, state_position)| {
                    let machine = &self.machines()[machine_position as usize];
                    (
                        machine,
                        &self.machine_states(machine)[state_position as usize],
                    )
                },
            )
        }

        pub fn machine_or_state_symbol(
            &self,
            machine: &machine::Machine,
            symbol: SymbolHandle,
        ) -> bool {
            machine.symbol == symbol
                || self
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == symbol)
        }

        pub fn trait_machine_signature(
            &self,
            symbol: SymbolHandle,
        ) -> Option<(
            &trait_definition::TraitDefinition,
            &signature::StateSignature,
        )> {
            if let Some((trait_index, signature_index)) =
                self.with_call_target_index(|index| index.trait_signature.get(&symbol).copied())
            {
                let trait_definition = &self.traits()[trait_index as usize];
                let signature =
                    &self.trait_machine_signatures(trait_definition)[signature_index as usize];
                return Some((trait_definition, signature));
            }
            if symbol.is_valid()
                && let Some(owner) =
                    self.trait_definition_by_symbol(self.symbols.get(symbol).parent)
                && let Some(clause) = owner
                    .refinement_clauses
                    .iter()
                    .find(|clause| clause.signature.symbol == symbol)
            {
                return Some((owner, &clause.signature));
            }
            let application = self.closed_requirement_application(symbol)?;
            let trait_definition = self.trait_definition_by_symbol(application.declaring_trait)?;
            Some((trait_definition, application.signature))
        }

        pub fn machine(&self, symbol: symbols::SymbolHandle) -> Option<&machine::Machine> {
            self.machine_by_symbol(symbol)
        }

        pub fn machine_by_name_str(&self, name: &str) -> Option<&machine::Machine> {
            self.machines()
                .iter()
                .find(|machine| machine.name.as_str() == name)
        }

        pub fn machine_state(
            &self,
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
        ) -> Option<(&machine::Machine, &crate::state::State)> {
            let machine = self.machine(machine_symbol)?;
            let state = state_by_symbol(self.machine_states(machine), state_symbol)?;
            Some((machine, state))
        }

        #[inline]
        pub fn machines_mut(&mut self) -> &mut [machine::Machine] {
            self.tables.machines.span_mut_or_empty(self.roots.machines)
        }

        pub fn declaration_type_parameters(&self, symbol: SymbolHandle) -> &[data::TypeParameter] {
            use symbols::SymbolKind;
            match self.symbols.get(symbol).kind {
                SymbolKind::Data => self
                    .data_definition_by_symbol(symbol)
                    .map(|declaration| self.data_type_parameters(declaration)),
                SymbolKind::TypeAlias => self
                    .type_aliases()
                    .iter()
                    .find(|alias| alias.symbol == symbol)
                    .map(|alias| {
                        self.data_type_parameters
                            .span_or_empty(alias.type_parameters)
                    }),
                SymbolKind::Domain => crate::domain::domain_by_symbol(self, symbol)
                    .map(|declaration| self.domain_type_parameters(declaration)),
                SymbolKind::Trait => self
                    .trait_definition_by_symbol(symbol)
                    .map(|declaration| self.trait_type_parameters(declaration)),
                SymbolKind::Conformance => self
                    .conformance_by_symbol(symbol)
                    .map(|declaration| self.conformance_type_parameters(declaration)),
                SymbolKind::Machine | SymbolKind::State => self
                    .machine_by_own_or_state_symbol(symbol)
                    .map(|declaration| self.machine_type_parameters(declaration))
                    .or_else(|| {
                        self.trait_machine_signature(symbol)
                            .map(|(_, signature)| self.state_signature_type_parameters(signature))
                    })
                    .or_else(|| {
                        self.retained_static_machine_contract(symbol)
                            .map(|contract| {
                                self.state_signature_type_parameters(contract.signature())
                            })
                    })
                    .or_else(|| {
                        self.prepared_callable_signatures()
                            .find(|signature| signature.symbol == symbol)
                            .map(|signature| self.state_signature_type_parameters(signature))
                    }),
                SymbolKind::Operator => {
                    self.operator_definition_by_symbol(symbol)
                        .map(|declaration| {
                            self.data_type_parameters
                                .span_or_empty(declaration.type_parameters)
                        })
                }
                _ => None,
            }
            .unwrap_or_default()
        }

        pub fn machine_kind_type_parameters<'a>(
            &'a self,
            machine: &'a machine::Machine,
        ) -> impl Iterator<Item = &'a data::TypeParameter> + 'a {
            self.machine_type_parameters(machine)
                .iter()
                .filter(|parameter| {
                    matches!(parameter.kind, data::TypeParameterKind::Machine { .. })
                })
        }

        pub fn machine_declaration_scope(
            &self,
            machine: &machine::Machine,
            same_type_parameter: impl Fn(&data::TypeParameter, &data::TypeParameter) -> bool,
        ) -> (Vec<data::TypeParameter>, Vec<Identifier>) {
            let mut type_parameters = self.machine_type_parameters(machine).to_vec();
            let mut lifetime_parameters = machine.lifetime_parameters.clone();
            let mut enclosing = self.declaration_scope_parent(machine.symbol);
            while enclosing.is_valid() {
                if let Some(declaration) = self.machine_declaration_for_lexical_scope(enclosing) {
                    for parameter in self.machine_type_parameters(declaration) {
                        if !type_parameters
                            .iter()
                            .any(|existing| same_type_parameter(existing, parameter))
                        {
                            type_parameters.push(parameter.clone());
                        }
                    }
                    for parameter in &declaration.lifetime_parameters {
                        if !lifetime_parameters.contains(parameter) {
                            lifetime_parameters.push(parameter.clone());
                        }
                    }
                }
                if let Some(conformance) = self.conformance_by_symbol(enclosing) {
                    for parameter in self.conformance_type_parameters(conformance) {
                        if !type_parameters
                            .iter()
                            .any(|existing| same_type_parameter(existing, parameter))
                        {
                            type_parameters.push(parameter.clone());
                        }
                    }
                    for parameter in &conformance.lifetime_parameters {
                        if !lifetime_parameters.contains(parameter) {
                            lifetime_parameters.push(parameter.clone());
                        }
                    }
                }
                enclosing = self.declaration_scope_parent(enclosing);
            }
            if machine.attached_data_symbol.is_valid()
                && let Some(definition) =
                    self.data_definition_by_symbol(machine.attached_data_symbol)
            {
                for parameter in self.data_type_parameters(definition) {
                    if !type_parameters
                        .iter()
                        .any(|existing| same_type_parameter(existing, parameter))
                    {
                        type_parameters.push(parameter.clone());
                    }
                }
                for parameter in &definition.lifetime_parameters {
                    if !lifetime_parameters
                        .iter()
                        .any(|existing| existing == parameter)
                    {
                        lifetime_parameters.push(parameter.clone());
                    }
                }
            }
            (type_parameters, lifetime_parameters)
        }

        pub fn machine_type_reference_scope(
            &self,
            machine: &machine::Machine,
            same_type_parameter: impl Fn(&data::TypeParameter, &data::TypeParameter) -> bool,
        ) -> (Vec<data::TypeParameter>, Vec<Identifier>) {
            let (type_parameters, mut lifetime_parameters) =
                self.machine_declaration_scope(machine, same_type_parameter);
            let mut owner = machine.symbol;
            for _ in 0..self.machine_specializations.len() {
                let Some(specialization) = self
                    .machine_specializations
                    .iter()
                    .find(|specialization| specialization.instance == owner)
                else {
                    break;
                };
                if !specialization.binding_application.is_valid() {
                    break;
                }
                let lexical_owner = self
                    .static_machine_application(specialization.binding_application)
                    .lexical_owner;
                if lexical_owner == owner {
                    break;
                }
                let captured = self
                    .machine_declaration_for_lexical_scope(lexical_owner)
                    .map(|machine| &machine.lifetime_parameters)
                    .or_else(|| {
                        self.data_definition_by_symbol(lexical_owner)
                            .map(|definition| &definition.lifetime_parameters)
                    })
                    .or_else(|| {
                        self.conformance_by_symbol(lexical_owner)
                            .map(|conformance| &conformance.lifetime_parameters)
                    });
                if let Some(captured) = captured {
                    for parameter in captured {
                        if !lifetime_parameters.contains(parameter) {
                            lifetime_parameters.push(parameter.clone());
                        }
                    }
                }
                owner = lexical_owner;
            }
            (type_parameters, lifetime_parameters)
        }

        pub fn machine_parameter_signature_in(
            &self,
            machine: &machine::Machine,
            symbol: SymbolHandle,
        ) -> Option<&signature::StateSignature> {
            self.machine_parameter_signature_in_with_scope_parents(machine, symbol, |owner| {
                self.declaration_scope_parent(owner)
            })
        }

        pub(crate) fn machine_parameter_signature_in_with_scope_parents<'program>(
            &'program self,
            machine: &machine::Machine,
            symbol: SymbolHandle,
            mut scope_parent: impl FnMut(SymbolHandle) -> SymbolHandle,
        ) -> Option<&'program signature::StateSignature> {
            let own = self.machine_type_parameters(machine);
            let mut parameters = own;
            let mut enclosing = machine.symbol;
            let mut visited = Vec::new();
            loop {
                if let Some(parameter) = parameters
                    .iter()
                    .find(|parameter| parameter.symbol == symbol)
                {
                    return match &parameter.kind {
                        data::TypeParameterKind::Machine { contract } => self
                            .machine_parameter_contract_view(contract)
                            .map(data::MachineParameterContractView::signature),
                        _ => None,
                    };
                }
                enclosing = scope_parent(enclosing);
                if !enclosing.is_valid() || visited.contains(&enclosing) {
                    return None;
                }
                visited.push(enclosing);
                if let Some(map) = self.conformance_by_symbol(enclosing) {
                    parameters = self.conformance_type_parameters(map);
                    if parameters.iter().any(|parameter| {
                        parameter.symbol == symbol
                            && own.iter().any(|local| local.name == parameter.name)
                    }) {
                        return None;
                    }
                } else {
                    parameters = &[];
                }
            }
        }

        pub fn retained_static_machine_contract(
            &self,
            symbol: SymbolHandle,
        ) -> Option<data::MachineParameterContractView<'_>> {
            if !symbol.is_valid() {
                return None;
            }
            self.with_type_parameter_declarations(symbol, |rows| {
                rows.iter().find_map(|handle| {
                    let parameter = self.data_type_parameters.get(*handle);
                    let data::TypeParameterKind::Machine { contract } = &parameter.kind else {
                        return None;
                    };
                    self.machine_parameter_contract_view(contract)
                })
            })
        }

        pub fn machine_parameter_signature(
            &self,
            symbol: SymbolHandle,
        ) -> Option<(&machine::Machine, &signature::StateSignature)> {
            self.retained_static_machine_contract(symbol)?;
            self.machines().iter().find_map(|machine| {
                self.machine_parameter_signature_in(machine, symbol)
                    .map(|signature| (machine, signature))
            })
        }

        pub fn machine_owned_data_by_symbol(
            &self,
            machine: &machine::Machine,
            symbol: symbols::SymbolHandle,
        ) -> Option<&machine::OwnedData> {
            self.machine_owned_data(machine)
                .iter()
                .find(|candidate| candidate.symbol == symbol)
        }

        pub fn for_each_machine_state_contract_frame(
            &self,
            mut visit: impl FnMut(
                &machine::Machine,
                &crate::state::State,
                Vec<&signature::SignatureContract>,
                String,
            ),
        ) {
            for machine in self.machines() {
                for (state_index, state) in self.machine_states(machine).iter().enumerate() {
                    let mut contracts = self.state_contracts(state).iter().collect::<Vec<_>>();
                    if state_index == 0 {
                        contracts.extend(self.machine_contracts(machine));
                    }
                    let label = if state_index == 0 {
                        machine.name.to_string()
                    } else {
                        format!("{}::{}", machine.name, state.name)
                    };
                    visit(machine, state, contracts, label);
                }
            }
        }

        pub fn contract_facts<'a>(
            &'a self,
            contracts: impl Iterator<Item = &'a signature::SignatureContract> + 'a,
            kind: signature::SignatureContractKind,
        ) -> impl Iterator<Item = &'a crate::domain::ProofFact> + 'a {
            contracts
                .filter(move |contract| contract.kind == kind)
                .flat_map(|contract| self.proof_facts.span_or_empty(contract.facts))
        }

        pub fn contract_expressions<'a>(
            &'a self,
            contracts: impl Iterator<Item = &'a signature::SignatureContract> + 'a,
            kind: signature::SignatureContractKind,
        ) -> impl Iterator<Item = crate::expression::ExpressionHandle> + 'a {
            self.contract_facts(contracts, kind)
                .filter_map(|fact| match fact {
                    crate::domain::ProofFact::Expression(expression) => Some(*expression),
                    _ => None,
                })
        }

        /*
            A machine's `crashes` clause is a ceiling its body and calls are
            checked against, instead of a request to infer the routes, when
            anything fixes that ceiling: a public or bodyless declaration, an
            authored route, or `crashes none`. The last leaves no contract row
            to find, so a private body that wrote it is told from one that
            omitted the clause by `declares_no_crash_route` alone. A member
            that realizes a trait requirement which wrote `crashes none` is
            held to that empty ceiling too: generics.md, "`crashes none`
            admitting only crash-free members".
        */
        pub fn machine_publishes_crash_ceiling(&self, machine: &machine::Machine) -> bool {
            machine.is_public
                || machine.declares_no_crash_route
                || self.realizes_crash_free_requirement(machine.symbol)
                || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                || self.machine_contracts(machine).iter().any(|contract| {
                    matches!(
                        contract.kind,
                        signature::SignatureContractKind::Crashes { .. }
                    )
                })
        }

        pub fn state_flow_site(
            &self,
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
        ) -> Option<(&machine::Machine, &crate::state::State)> {
            let machine = self.machine_by_symbol(machine_symbol)?;
            let state = self.state_in_machine(machine, state_symbol)?;
            Some((machine, state))
        }

        pub fn state_in_machine(
            &self,
            machine: &machine::Machine,
            symbol: SymbolHandle,
        ) -> Option<&crate::state::State> {
            state_by_symbol(self.machine_states(machine), symbol)
        }

        #[inline]
        pub fn machine_states_mut(
            &mut self,
            machine: &machine::Machine,
        ) -> &mut [crate::state::State] {
            self.machine_states.span_mut_or_empty(machine.states)
        }

        pub fn data_binding_parameter<'parameters>(
            &self,
            machine: &machine::Machine,
            parameters: &'parameters [signature::StateParameter],
        ) -> Option<&'parameters signature::StateParameter> {
            self.data_binding_position(machine.attached_data_symbol, parameters)
                .map(|index| &parameters[index])
        }

        pub fn state_data_binding(
            &self,
            machine: &machine::Machine,
            state: &crate::state::State,
        ) -> Option<&Identifier> {
            self.data_binding_parameter(machine, self.state_parameters(state))
                .map(|parameter| &parameter.name)
        }

        pub fn machine_data_binding(&self, machine: &machine::Machine) -> Option<&Identifier> {
            let entry = self.machine_states(machine).first()?;
            self.state_data_binding(machine, entry)
        }

        pub fn parameter_declares_attached_data(
            &self,
            machine: &machine::Machine,
            parameter: &signature::StateParameter,
        ) -> bool {
            self.type_reference_names_attached_data(
                parameter.type_reference,
                machine.attached_data_symbol,
            )
        }

        pub fn data_binding_position(
            &self,
            attached_data_symbol: SymbolHandle,
            parameters: &[signature::StateParameter],
        ) -> Option<usize> {
            if !attached_data_symbol.is_valid() {
                return None;
            }
            let mut declared = parameters.iter().enumerate().filter(|(_, parameter)| {
                self.type_reference_names_attached_data(
                    parameter.type_reference,
                    attached_data_symbol,
                )
            });
            let (index, _) = declared.next()?;
            declared.next().is_none().then_some(index)
        }

        pub(crate) fn type_reference_names_attached_data(
            &self,
            type_reference: crate::types::TypeReferenceHandle,
            attached_data_symbol: SymbolHandle,
        ) -> bool {
            if !type_reference.is_valid() {
                return false;
            }
            match self.type_reference_table.type_reference(type_reference) {
                crate::types::TypeReferenceNode::Reference { referee, .. } => {
                    self.type_reference_names_attached_data(*referee, attached_data_symbol)
                }
                crate::types::TypeReferenceNode::Constrained { base_type, .. } => {
                    self.type_reference_names_attached_data(*base_type, attached_data_symbol)
                }
                crate::types::TypeReferenceNode::Named { symbol, .. }
                | crate::types::TypeReferenceNode::Generic {
                    base_symbol: symbol,
                    ..
                } => *symbol == attached_data_symbol,
                _ => false,
            }
        }

        pub fn machine_holding_state(
            &self,
            state_symbol: SymbolHandle,
        ) -> Option<&machine::Machine> {
            if !state_symbol.is_valid() {
                return None;
            }
            let symbol = self.symbols.get(state_symbol);
            let parent = symbol.parent;
            let parent_machine = self.machine(parent);
            self.with_state_positions_for_symbol(state_symbol, |positions| {
                let mut other = None;
                for (machine_position, _) in positions {
                    let machine = &self.machines()[*machine_position as usize];
                    if parent_machine.is_some_and(|candidate| std::ptr::eq(candidate, machine)) {
                        return Some(machine);
                    }
                    if other.is_none() && machine.symbol != parent {
                        other = Some(machine);
                    }
                }
                if !self.may_name_stored_state(symbol) {
                    return None;
                }
                other
            })
        }

        pub fn may_name_stored_state(&self, symbol: &symbols::Symbol) -> bool {
            matches!(
                symbol.kind,
                symbols::SymbolKind::State | symbols::SymbolKind::Unknown
            ) && self.symbols.get(symbol.parent).kind != symbols::SymbolKind::Trait
        }

        pub fn state_by_symbol(&self, state_symbol: SymbolHandle) -> Option<&crate::state::State> {
            self.machine_holding_state(state_symbol)
                .and_then(|machine| state_by_symbol(self.machine_states(machine), state_symbol))
        }

        pub fn state_parameter_by_symbol(
            &self,
            state: &crate::state::State,
            symbol: SymbolHandle,
        ) -> Option<&signature::StateParameter> {
            self.state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == symbol)
        }

        pub fn public_state_parameters<'a>(
            &'a self,
            state: &'a crate::state::State,
        ) -> impl Iterator<Item = &'a signature::StateParameter> + 'a {
            self.state_parameters(state)
                .iter()
                .filter(|parameter| !parameter.is_const)
        }

        pub fn machine_declares_crash(&self, machine: &machine::Machine) -> bool {
            self.machine_contracts(machine)
                .iter()
                .chain(
                    self.machine_states(machine)
                        .iter()
                        .flat_map(|state| self.state_contracts(state)),
                )
                .any(|contract| {
                    matches!(
                        contract.kind,
                        signature::SignatureContractKind::Crashes { .. }
                    )
                })
        }

        pub fn state_local_declaration(
            &self,
            state: &crate::state::State,
            symbol: SymbolHandle,
        ) -> Option<(usize, &crate::statement::TableLocalData)> {
            let mut declarations = self
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
                .filter_map(|(ordinal, statement)| match statement {
                    LocalData(local) if local.symbol == symbol => Some((ordinal, local)),
                    _ => None,
                });
            let declaration = declarations.next()?;
            declarations.next().is_none().then_some(declaration)
        }

        pub fn immutable_scalar_locals_before(
            &self,
            state: &crate::state::State,
            statement_index: usize,
        ) -> usize {
            self.statement_table
                .statements(state.statement_nodes)
                .iter()
                .take(statement_index)
                .filter(|statement| {
                    matches!(statement, LocalData(local)
                    if !local.is_mutable
                        && self.primitive_type_reference(local.type_reference).is_some())
                })
                .count()
        }
    }

    #[cfg(test)]
    mod tests {
        use crate::TypedTrees;
        use crate::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
        use crate::machine::Machine;
        use crate::signature::StateSignature;
        use crate::state::State;
        use arena::HandleSpan;
        use symbols::SymbolHandle;

        fn assert_state_lookups_match_roster(program: &TypedTrees, symbols: &[SymbolHandle]) {
            for &symbol in symbols {
                let expected: Vec<_> = program
                    .machines()
                    .iter()
                    .flat_map(|machine| {
                        program
                            .machine_states(machine)
                            .iter()
                            .filter(move |state| state.symbol == symbol)
                            .map(move |state| (machine, state))
                    })
                    .collect();
                let actual: Vec<_> = program.machine_states_named(symbol).collect();
                assert_eq!(actual.len(), expected.len());
                for ((actual_machine, actual_state), (expected_machine, expected_state)) in
                    actual.iter().zip(&expected)
                {
                    assert!(std::ptr::eq(*actual_machine, *expected_machine));
                    assert!(std::ptr::eq(*actual_state, *expected_state));
                }
                let expected_owner = program
                    .machines()
                    .iter()
                    .find(|machine| program.machine_or_state_symbol(machine, symbol));
                assert_eq!(
                    program
                        .machine_by_own_or_state_symbol(symbol)
                        .map(std::ptr::from_ref),
                    expected_owner.map(std::ptr::from_ref)
                );
                let expected_entry = program.machines().iter().find_map(|machine| {
                    program
                        .machine_states(machine)
                        .first()
                        .filter(|entry| entry.symbol == symbol)
                        .map(|entry| (machine, entry))
                });
                assert_eq!(
                    program
                        .machine_by_entry_state_symbol(symbol)
                        .map(|(machine, state)| (
                            std::ptr::from_ref(machine),
                            std::ptr::from_ref(state)
                        )),
                    expected_entry.map(|(machine, state)| (
                        std::ptr::from_ref(machine),
                        std::ptr::from_ref(state)
                    ))
                );
                for &machine_symbol in symbols {
                    let expected_position = (machine_symbol.is_valid() && symbol.is_valid())
                        .then(|| {
                            program
                                .machines()
                                .iter()
                                .filter(|machine| machine.symbol == machine_symbol)
                                .find_map(|machine| {
                                    program
                                        .machine_states(machine)
                                        .iter()
                                        .position(|state| state.symbol == symbol)
                                })
                        })
                        .flatten();
                    assert_eq!(
                        program.state_position_by_symbol(machine_symbol, symbol),
                        expected_position
                    );
                }
            }
        }

        #[test]
        fn state_lookup_preserves_roster_order_duplicates_and_mutation() {
            let first = SymbolHandle::from_parts(1, 1);
            let second = SymbolHandle::from_parts(2, 1);
            let shared = SymbolHandle::from_parts(3, 1);
            let replaced = SymbolHandle::from_parts(3, 2);
            let symbols = [first, second, shared, replaced, SymbolHandle::invalid()];
            let mut program = TypedTrees::default();
            assert_state_lookups_match_roster(&program, &symbols);
            for (machine_symbol, states) in [
                (first, vec![first, shared, second]),
                (second, vec![shared, shared]),
                (first, vec![replaced]),
                (SymbolHandle::invalid(), vec![SymbolHandle::invalid()]),
            ] {
                let mut machine = Machine {
                    symbol: machine_symbol,
                    ..Machine::default()
                };
                for symbol in states {
                    program.push_machine_state(
                        &mut machine,
                        State {
                            symbol,
                            ..State::default()
                        },
                    );
                }
                program.push_machine(machine);
                assert_state_lookups_match_roster(&program, &symbols);
            }
            let snapshot = program.clone();
            let machine = program.machines()[0].clone();
            program.machine_states_mut(&machine)[1].symbol = replaced;
            assert_state_lookups_match_roster(&program, &symbols);
            assert_state_lookups_match_roster(&snapshot, &symbols);

            let original = program.roots.machines;
            program.machines_mut()[0].symbol = replaced;
            program.roots.machines = HandleSpan::from_parts(original.start(), original.count() - 1);
            assert_state_lookups_match_roster(&program, &symbols);
            program.roots.machines = HandleSpan::from_parts(original.start(), 2);
            assert_state_lookups_match_roster(&program, &symbols);
            program.roots.machines = HandleSpan::from_parts(original.handle_at(1), 2);
            assert_state_lookups_match_roster(&program, &symbols);
            program.roots.machines = original;
            program.machines_mut()[0].symbol = replaced;
            assert_state_lookups_match_roster(&program, &symbols);
            program.roots.machines = HandleSpan::from_parts(
                arena::Handle::from_parts(
                    original.start().arena_index(),
                    original.start().generation() + 1,
                ),
                original.count(),
            );
            assert!(program.machines().is_empty());
            assert_state_lookups_match_roster(&program, &symbols);
            program.roots.machines = original;
            assert_state_lookups_match_roster(&program, &symbols);
        }

        #[test]
        fn machine_parameter_lookup_refreshes_misses_without_selecting_index_duplicates() {
            let original = SymbolHandle::from_parts(1, 1);
            let replacement = SymbolHandle::from_parts(1, 2);
            let contract_symbol = SymbolHandle::from_parts(2, 1);
            let mut program = TypedTrees::default();
            assert!(program.machine_parameter_signature(original).is_none());
            let mut machine = Machine::default();
            let parameter = program.data_type_parameters.append_to_span(
                &mut machine.type_parameters,
                TypeParameter {
                    symbol: original,
                    kind: TypeParameterKind::Machine {
                        contract: MachineParameterContract::Structural(StateSignature {
                            symbol: contract_symbol,
                            ..StateSignature::default()
                        }),
                    },
                    ..TypeParameter::default()
                },
            );
            program.push_machine(machine);
            assert_eq!(
                program
                    .machine_parameter_signature(original)
                    .map(|(_, signature)| signature.symbol),
                Some(contract_symbol)
            );
            assert!(program.machine_parameter_signature(replacement).is_none());
            program.data_type_parameters.get_mut(parameter).symbol = replacement;
            assert!(program.machine_parameter_signature(original).is_none());
            assert_eq!(
                program
                    .machine_parameter_signature(replacement)
                    .map(|(_, signature)| signature.symbol),
                Some(contract_symbol)
            );

            program.data_type_parameters.insert(TypeParameter {
                symbol: replacement,
                ..TypeParameter::default()
            });
            assert_eq!(
                program
                    .machine_parameter_signature(replacement)
                    .map(|(_, signature)| signature.symbol),
                Some(contract_symbol)
            );
            assert!(
                program
                    .machine_parameter_signature(SymbolHandle::invalid())
                    .is_none()
            );
        }

        #[test]
        fn machine_lookup_retains_first_match_and_refreshes_after_mutation() {
            let original = SymbolHandle::from_parts(1, 1);
            let replacement = SymbolHandle::from_parts(1, 2);
            let mut program = TypedTrees::default();
            program.push_machine(Machine {
                symbol: original,
                ..Machine::default()
            });
            assert_eq!(
                program.machine(original).map(|machine| machine.is_public),
                Some(false)
            );
            assert!(program.machine(replacement).is_none());
            assert!(program.machine(SymbolHandle::invalid()).is_none());

            program.push_machine(Machine {
                symbol: original,
                is_public: true,
                ..Machine::default()
            });
            assert_eq!(
                program.machine(original).map(|machine| machine.is_public),
                Some(false)
            );

            let snapshot = program.clone();
            program.machines_mut()[0].symbol = replacement;
            assert_eq!(
                program.machine(original).map(|machine| machine.is_public),
                Some(true)
            );
            assert_eq!(
                program
                    .machine(replacement)
                    .map(|machine| machine.is_public),
                Some(false)
            );
            assert_eq!(
                snapshot.machine(original).map(|machine| machine.is_public),
                Some(false)
            );
            assert!(snapshot.machine(replacement).is_none());
        }
    }
}

pub(crate) mod type_parameters {
    use crate::data::TypeParameter;
    use crate::declaration_index::SymbolRows;
    use arena::{Arena, Handle, OwnedIndex};

    pub(crate) type TypeParameterIndex = SymbolRows<Handle<TypeParameter>>;

    #[derive(Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TypeParameterTable {
        arena: Arena<TypeParameter>,
        index: OwnedIndex<TypeParameterIndex, ()>,
    }

    impl From<Arena<TypeParameter>> for TypeParameterTable {
        fn from(arena: Arena<TypeParameter>) -> Self {
            Self {
                arena,
                index: OwnedIndex::default(),
            }
        }
    }
    impl std::fmt::Debug for TypeParameterTable {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.arena.fmt(f)
        }
    }
    impl std::ops::Deref for TypeParameterTable {
        type Target = Arena<TypeParameter>;
        fn deref(&self) -> &Self::Target {
            &self.arena
        }
    }
    impl std::ops::DerefMut for TypeParameterTable {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.index.clear();
            &mut self.arena
        }
    }
    impl TypeParameterTable {
        pub(crate) fn with_index<R>(&self, read: impl FnOnce(&TypeParameterIndex) -> R) -> R {
            self.index.with(
                (),
                || {
                    SymbolRows::new(
                        self.arena
                            .iter()
                            .filter(|(_, parameter)| parameter.symbol.is_valid())
                            .map(|(handle, parameter)| (parameter.symbol, handle))
                            .collect(),
                    )
                },
                read,
            )
        }
        pub(crate) fn seal(&mut self) {
            self.with_index(|_| ());
            self.index.seal_with(SymbolRows::densify);
        }
        pub(crate) fn unseal(&mut self) {
            self.index.unseal();
        }
    }

    #[cfg(test)]
    pub(crate) mod tests {
        use super::*;
        use crate::data::{MachineParameterContract, TypeParameterKind};
        use crate::name::Identifier;
        use crate::signature::StateSignature;
        use crate::{FrozenTypedTrees, TypedTrees};
        use symbols::SymbolHandle;

        fn parameter(symbol: SymbolHandle, name: &str, kind: TypeParameterKind) -> TypeParameter {
            TypeParameter {
                symbol,
                name: Identifier::generated(name),
                kind,
                bounds: Default::default(),
            }
        }
        fn callable(symbol: SymbolHandle, name: &str) -> TypeParameter {
            parameter(
                symbol,
                name,
                TypeParameterKind::Machine {
                    contract: MachineParameterContract::Structural(StateSignature {
                        name: Identifier::generated(name),
                        ..Default::default()
                    }),
                },
            )
        }
        fn assert_queries(program: &TypedTrees, symbols: &[SymbolHandle]) {
            for &symbol in symbols {
                let rows = program
                    .data_type_parameters
                    .iter()
                    .filter(|(_, p)| p.symbol == symbol && symbol.is_valid())
                    .map(|(_, p)| p)
                    .collect::<Vec<_>>();
                assert_eq!(
                    program.first_data_type_parameter_by_symbol(symbol),
                    rows.first().copied()
                );
                assert_eq!(
                    program.data_type_parameter_by_symbol(symbol),
                    rows.last().copied()
                );
                let retained = rows.iter().find_map(|p| {
                    let TypeParameterKind::Machine { contract } = &p.kind else {
                        return None;
                    };
                    program.machine_parameter_contract_view(contract)
                });
                assert_eq!(
                    program
                        .retained_static_machine_contract(symbol)
                        .map(|v| v.signature()),
                    retained.map(|v| v.signature())
                );
            }
        }

        #[test]
        fn contract_index_preserves_generations_duplicates_and_kind_selection() {
            let first = SymbolHandle::from_parts(7, 1);
            let next = SymbolHandle::from_parts(7, 2);
            let absent = SymbolHandle::from_parts(8, 1);
            let symbols = [first, next, absent, SymbolHandle::invalid()];
            let mut p = TypedTrees::default();
            assert_queries(&p, &symbols);
            for row in [
                parameter(first, "type", TypeParameterKind::Type),
                parameter(
                    first,
                    "identity",
                    TypeParameterKind::Machine {
                        contract: MachineParameterContract::RequirementIdentity,
                    },
                ),
                parameter(
                    first,
                    "unresolved",
                    TypeParameterKind::Machine {
                        contract: MachineParameterContract::Nominal {
                            receiving_trait: SymbolHandle::invalid(),
                            receiving_arguments: vec![],
                            trait_definition: SymbolHandle::invalid(),
                            requirement: SymbolHandle::invalid(),
                            trait_arguments: vec![],
                            instantiated: None,
                        },
                    },
                ),
                callable(next, "generation-two"),
                callable(first, "first-contract"),
                callable(first, "later-contract"),
            ] {
                p.tables.data_type_parameters.insert(row);
                assert_queries(&p, &symbols);
            }
            assert_eq!(
                p.retained_static_machine_contract(first)
                    .unwrap()
                    .signature()
                    .name
                    .as_str(),
                "first-contract"
            );
            assert_eq!(
                p.retained_static_machine_contract(next)
                    .unwrap()
                    .signature()
                    .name
                    .as_str(),
                "generation-two"
            );
            assert_queries(&FrozenTypedTrees::from(p), &symbols);
        }

        #[test]
        fn contract_index_follows_mutation_clone_and_equal_revision_replacement() {
            let first = SymbolHandle::from_parts(11, 1);
            let other = SymbolHandle::from_parts(12, 1);
            let symbols = [first, other];
            let mut p = TypedTrees::default();
            let handle = p
                .tables
                .data_type_parameters
                .insert(callable(first, "original"));
            assert_queries(&p, &symbols);
            let mut cloned = p.clone();
            cloned.tables.data_type_parameters.get_mut(handle).symbol = other;
            assert_queries(&cloned, &symbols);
            assert_queries(&p, &symbols);
            let mut replacement = Arena::default();
            replacement.insert(callable(other, "replacement"));
            assert_eq!(
                replacement.revision(),
                p.tables.data_type_parameters.revision()
            );
            *p.tables.data_type_parameters = replacement;
            assert_queries(&p, &symbols);
            let frozen = FrozenTypedTrees::from(p);
            assert_queries(&frozen, &symbols);
            let mut p = frozen.into_inner();
            p.tables.data_type_parameters.get_mut(handle).symbol = first;
            assert_queries(&p, &symbols);
            p.tables.data_type_parameters = cloned.tables.data_type_parameters.clone();
            assert_queries(&p, &symbols);
            assert_queries(&cloned, &symbols);
        }
    }
}

pub(crate) mod type_reference_queries {
    use crate::access_views::named_type_reference_through_shells;
    use crate::{
        AccessFieldPlan, AccessViewPlan, TypedTrees, static_const_argument_spelling,
        static_const_identity_from_spelling,
    };
    use crate::{expression, name, types};

    pub(crate) mod application_indices {
        use crate::TypedTrees;
        use crate::data::TypeParameterKind;
        use crate::types::{TypeReferenceHandle, TypeReferenceNode};

        #[derive(Clone, Copy, Debug, Default)]
        pub struct ApplicationValueIndices {
            const_values: bool,
            runtime_values: bool,
        }

        impl ApplicationValueIndices {
            pub fn is_empty(self) -> bool {
                !self.const_values && !self.runtime_values
            }

            pub fn has_runtime_values(self) -> bool {
                self.runtime_values
            }
        }

        impl TypedTrees {
            pub fn type_application_value_indices(
                &self,
                reference: TypeReferenceHandle,
            ) -> ApplicationValueIndices {
                let mut indices = ApplicationValueIndices::default();
                let mut pending = vec![reference];
                let mut visited = Vec::new();
                while let Some(reference) = pending.pop() {
                    if !self.type_reference_table.contains_type_reference(reference)
                        || visited.contains(&reference)
                    {
                        continue;
                    }
                    visited.push(reference);
                    match self.type_reference_table.type_reference(reference) {
                        TypeReferenceNode::Constrained { base_type, .. } => {
                            pending.push(*base_type)
                        }
                        TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
                        TypeReferenceNode::FixedArray { element_type, .. }
                        | TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
                        TypeReferenceNode::Generic {
                            base_symbol,
                            arguments,
                            ..
                        } => {
                            if base_symbol.is_valid() {
                                for parameter in self.declaration_type_parameters(*base_symbol) {
                                    match parameter.kind {
                                        TypeParameterKind::Const { .. } => {
                                            indices.const_values = true
                                        }
                                        TypeParameterKind::Value { .. } => {
                                            indices.runtime_values = true
                                        }
                                        _ => {}
                                    }
                                }
                                if arguments.count() == 0
                                    && let Some(origin) = self
                                        .data_definition_by_symbol(*base_symbol)
                                        .and_then(|declaration| declaration.generic_instance)
                                {
                                    pending.push(origin);
                                }
                            }
                            pending.extend_from_slice(
                                self.type_reference_table.type_reference_handles(*arguments),
                            );
                        }
                        TypeReferenceNode::Named { symbol, .. } => {
                            if let Some(origin) = self
                                .data_definition_by_symbol(*symbol)
                                .and_then(|declaration| declaration.generic_instance)
                            {
                                pending.push(origin);
                            }
                        }
                        _ => {}
                    }
                }
                indices
            }
        }
    }
    pub(crate) mod reference_descriptors {
        use crate::{TypedTrees, types::TypeReferenceHandle, types::TypeReferenceNode};
        use symbols::{BuiltinTypeAtom, SymbolKind};

        impl TypedTrees {
            pub fn reference_descriptor_word_count(
                &self,
                mut referee: TypeReferenceHandle,
            ) -> Option<u64> {
                let mut visited = Vec::new();
                loop {
                    if !self.type_reference_table.contains_type_reference(referee)
                        || visited.contains(&referee)
                    {
                        return None;
                    }
                    visited.push(referee);
                    match self.type_reference_table.type_reference(referee) {
                        TypeReferenceNode::Constrained { base_type, .. } => referee = *base_type,
                        TypeReferenceNode::Slice { .. } => return Some(2),
                        TypeReferenceNode::FixedArray { .. }
                        | TypeReferenceNode::Reference { .. }
                        | TypeReferenceNode::DynamicTrait { .. }
                        | TypeReferenceNode::Callable { .. }
                        | TypeReferenceNode::Unit => return Some(1),
                        TypeReferenceNode::Named { symbol, .. }
                        | TypeReferenceNode::Generic {
                            base_symbol: symbol,
                            ..
                        } => {
                            if !symbol.is_valid() {
                                return None;
                            }
                            if let Some(atom) = self.symbols.builtin_type_atom(*symbol) {
                                return (atom != BuiltinTypeAtom::Slice).then_some(1);
                            }
                            return match self.symbols.get(*symbol).kind {
                                SymbolKind::Data
                                    if self.unique_data_definition_by_symbol(*symbol).is_some() =>
                                {
                                    Some(1)
                                }
                                SymbolKind::Machine
                                    if self.machine_by_symbol(*symbol).is_some() =>
                                {
                                    Some(1)
                                }
                                _ => None,
                            };
                        }
                        TypeReferenceNode::ConstExpression(_) => return None,
                    }
                }
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::{
                data::{DataDefinition, TypeParameter, TypeParameterKind},
                name::Identifier,
            };
            use symbols::{SymbolHandle, SymbolNameRef, SymbolTableBuilder};

            #[test]
            fn descriptor_shape_requires_known_referent_kind_without_pointee_geometry() {
                let mut symbols = SymbolTableBuilder::default();
                let root =
                    symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("descriptors"));
                let children = symbols.insert_children(
                    root,
                    symbols::builtin_type_symbols().into_iter().chain([
                        (SymbolKind::Data, SymbolNameRef::Static("Open")),
                        (SymbolKind::Data, SymbolNameRef::Static("string")),
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("u64")),
                        (SymbolKind::BuiltinType, SymbolNameRef::Static("Slice")),
                    ]),
                );
                let data = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
                let string = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 1);
                let unknown = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 2);
                let primitive_spelling = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 3);
                let fake_slice = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 4);
                let marker = symbols
                    .insert_children(
                        data,
                        [(SymbolKind::TypeParameter, SymbolNameRef::Static("N"))],
                    )
                    .handle_at(0);
                let mut program = TypedTrees {
                    symbols: symbols.finish(),
                    ..Default::default()
                };
                let named = |program: &mut TypedTrees, symbol: SymbolHandle, name: &str| {
                    program
                        .type_reference_table
                        .insert(TypeReferenceNode::Named {
                            symbol,
                            name: Identifier::generated(name),
                        })
                };
                let u64_type = named(
                    &mut program,
                    children.handle_at(BuiltinTypeAtom::U64.ordinal() as u32),
                    "u64",
                );
                let marker_type = named(&mut program, marker, "N");
                let parameters = program.data_type_parameters.insert_many([TypeParameter {
                    symbol: marker,
                    name: Identifier::generated("N"),
                    kind: TypeParameterKind::Const {
                        type_reference: u64_type,
                    },
                    ..Default::default()
                }]);
                program.push_data_definition(DataDefinition {
                    symbol: data,
                    name: Identifier::generated("Open"),
                    type_parameters: parameters,
                    ..Default::default()
                });
                program.push_data_definition(DataDefinition {
                    symbol: string,
                    name: Identifier::generated("string"),
                    ..Default::default()
                });
                let arguments = program
                    .type_reference_table
                    .insert_type_reference_handles([marker_type]);
                let open = program
                    .type_reference_table
                    .insert(TypeReferenceNode::Generic {
                        base_symbol: data,
                        base_name: Identifier::generated("Open"),
                        lifetime_arguments: Vec::new(),
                        arguments,
                    });
                assert!(program.closed_data_definition(open).is_none());
                assert_eq!(program.reference_descriptor_word_count(open), Some(1));
                assert_eq!(program.reference_descriptor_word_count(u64_type), Some(1));
                let unknown = named(&mut program, unknown, "T");
                assert_eq!(program.reference_descriptor_word_count(unknown), None);
                let primitive_spelling = named(&mut program, primitive_spelling, "u64");
                assert_eq!(
                    program.reference_descriptor_word_count(primitive_spelling),
                    None
                );
                let string = named(&mut program, string, "string");
                assert_eq!(program.reference_descriptor_word_count(string), Some(1));
                let unresolved = named(&mut program, SymbolHandle::invalid(), "string");
                assert_eq!(program.reference_descriptor_word_count(unresolved), None);
                let fake_slice = named(&mut program, fake_slice, "Slice");
                assert_eq!(program.reference_descriptor_word_count(fake_slice), None);
                let builtin_slice = named(
                    &mut program,
                    children.handle_at(BuiltinTypeAtom::Slice.ordinal() as u32),
                    "Slice",
                );
                assert_eq!(program.reference_descriptor_word_count(builtin_slice), None);
                let slice = program
                    .type_reference_table
                    .insert(TypeReferenceNode::Slice {
                        element_type: unknown,
                    });
                let constrained =
                    program
                        .type_reference_table
                        .insert(TypeReferenceNode::Constrained {
                            base_type: slice,
                            constraints: Default::default(),
                        });
                assert_eq!(program.reference_descriptor_word_count(slice), Some(2));
                assert_eq!(
                    program.reference_descriptor_word_count(constrained),
                    Some(2)
                );
                let reference = program
                    .type_reference_table
                    .insert(TypeReferenceNode::Reference {
                        referee: slice,
                        access: language_semantics::ReferenceAccess::Shared,
                        lifetime: None,
                    });
                assert_eq!(program.reference_descriptor_word_count(reference), Some(1));
                assert_eq!(
                    program.reference_descriptor_word_count(TypeReferenceHandle::invalid()),
                    None
                );
                assert_eq!(
                    program.reference_descriptor_word_count(TypeReferenceHandle::from_parts(
                        open.arena_index(),
                        open.generation() + 1
                    )),
                    None
                );
            }
        }
    }
    pub use application_indices::ApplicationValueIndices;

    impl TypedTrees {
        pub fn retain_collection_result_types(&mut self, reference: types::TypeReferenceHandle) {
            self.tables
                .type_reference_table
                .retain_collection_result_types(&self.symbols, reference);
        }

        pub fn unwrapped_type_reference(
            &self,
            type_reference: types::TypeReferenceHandle,
        ) -> Option<types::TypeReferenceHandle> {
            type_reference.is_valid().then(|| {
                self.type_reference_table
                    .peeled_type_reference(type_reference)
            })
        }

        pub fn erased_scalar_observation_type(
            &self,
            reference: types::TypeReferenceHandle,
        ) -> Option<types::PrimitiveType> {
            if let Some(primitive) = self.primitive_type_reference(reference) {
                return Some(primitive);
            }
            if !reference.is_valid() {
                return None;
            }
            let types::TypeReferenceNode::Reference {
                access, referee, ..
            } = self.type_reference_table.type_reference(reference)
            else {
                return None;
            };
            if *access != language_core::ReferenceAccess::Shared {
                return None;
            }
            let primitive = self.primitive_type_reference(*referee)?;
            (primitive != types::PrimitiveType::Addr).then_some(primitive)
        }

        /*
            A nominal whose values a contract can name field by field: a
            checked-shape, non-generic declaration with no quotient and no laid
            layout, access view or wire schema that would give its fields a
            representation the proof side cannot read structurally.
        */
        pub fn closed_contract_nominal(&self, definition: &crate::data::DataDefinition) -> bool {
            definition.supply_mode == language_core::DataSupplyMode::CheckedShape
                && definition.quotient.is_none()
                && self.data_type_parameters(definition).is_empty()
                && !self
                    .plan_laid_layouts
                    .iter()
                    .any(|plan| plan.data_symbol == definition.symbol)
                && !self
                    .access_view_plans
                    .iter()
                    .any(|plan| plan.data_symbol == definition.symbol)
                && !self
                    .wire_schemas()
                    .iter()
                    .any(|schema| schema.name == definition.name)
        }

        /*
            The erased scalar lanes of one erased binding type, in declaration
            order. A primitive, or a shared reference to one, is a single lane
            with an empty path. A closed record of runtime-relevant scalar
            fields (nested records included), by value or behind a shared
            reference, contributes one lane per scalar leaf, so a contract can
            name `record.field` without the record acquiring a runtime field,
            address, read or cleanup. Every other type has no lanes: arrays,
            sums, erased fields and proof-only carriers stay with the
            contract-term lane or are refused by its admission.

            This is the only owner of the expansion. Erased formal positions,
            parameter plans and call-site actuals all index the lanes it
            returns; none re-derives the leaf order.
        */
        pub fn erased_scalar_lanes(
            &self,
            reference: types::TypeReferenceHandle,
        ) -> Vec<crate::proof_only::ErasedScalarLane> {
            if let Some(primitive_type) = self.erased_scalar_observation_type(reference) {
                return vec![crate::proof_only::ErasedScalarLane {
                    fields: Vec::new(),
                    primitive_type,
                }];
            }
            if !reference.is_valid() {
                return Vec::new();
            }
            let record = match self.type_reference_table.type_reference(reference) {
                types::TypeReferenceNode::Reference {
                    access: language_core::ReferenceAccess::Shared,
                    referee,
                    ..
                } => *referee,
                types::TypeReferenceNode::Reference { .. } => return Vec::new(),
                _ => reference,
            };
            let mut lanes = Vec::new();
            let mut prefix = Vec::new();
            let mut visiting = Vec::new();
            if self.collect_erased_record_lanes(record, &mut prefix, &mut visiting, &mut lanes) {
                lanes
            } else {
                Vec::new()
            }
        }

        fn collect_erased_record_lanes(
            &self,
            reference: types::TypeReferenceHandle,
            prefix: &mut Vec<symbols::SymbolHandle>,
            visiting: &mut Vec<symbols::SymbolHandle>,
            lanes: &mut Vec<crate::proof_only::ErasedScalarLane>,
        ) -> bool {
            if !reference.is_valid() {
                return false;
            }
            let types::TypeReferenceNode::Named { symbol, .. } =
                self.type_reference_table.type_reference(reference)
            else {
                return false;
            };
            if visiting.contains(symbol) {
                return false;
            }
            let Some(definition) = self.data_definition_by_symbol(*symbol) else {
                return false;
            };
            let members = self.data_members(definition);
            if !self.closed_contract_nominal(definition)
                || crate::data::DataDefinition::shape_kind_from_members(members)
                    != crate::data::DataShapeKind::Record
            {
                return false;
            }
            visiting.push(*symbol);
            let mut admitted = true;
            for member in members {
                let crate::data::DataMember::Field(field) = member else {
                    admitted = false;
                    break;
                };
                if field.relevance.is_erased() {
                    admitted = false;
                    break;
                }
                prefix.push(field.symbol);
                if let Some(primitive_type) = self.primitive_type_reference(field.type_reference) {
                    if primitive_type == types::PrimitiveType::Addr {
                        admitted = false;
                    } else {
                        lanes.push(crate::proof_only::ErasedScalarLane {
                            fields: prefix.clone(),
                            primitive_type,
                        });
                    }
                } else {
                    admitted = self.collect_erased_record_lanes(
                        field.type_reference,
                        prefix,
                        visiting,
                        lanes,
                    );
                }
                prefix.pop();
                if !admitted {
                    break;
                }
            }
            visiting.pop();
            admitted
        }

        /*
            Dense index of the first erased scalar lane owned by the parameter at
            `position`: the lanes of every erased formal authored before it.
        */
        pub fn erased_scalar_lane_base(
            &self,
            parameters: &[crate::signature::StateParameter],
            position: usize,
        ) -> usize {
            parameters
                .iter()
                .take(position)
                .filter(|parameter| parameter.relevance.is_erased())
                .map(|parameter| self.erased_scalar_lanes(parameter.type_reference).len())
                .sum()
        }

        pub fn display_type_reference(&self, type_reference: types::TypeReferenceHandle) -> String {
            self.type_reference_table.display_name(type_reference)
        }

        pub fn display_type_reference_with_constraints(
            &self,
            type_reference: types::TypeReferenceHandle,
        ) -> String {
            self.type_reference_table
                .display_name_with_constraints(type_reference, &self.expression_table)
        }

        pub fn primitive_type_reference(
            &self,
            type_reference: types::TypeReferenceHandle,
        ) -> Option<types::PrimitiveType> {
            self.type_reference_table.primitive_type(type_reference)
        }

        pub fn static_const_argument_identity(
            &self,
            argument: &expression::StaticMachineArgument,
        ) -> Option<String> {
            static_const_argument_spelling(self, argument)
                .map(|spelling| static_const_identity_from_spelling(&spelling))
        }

        pub fn static_const_type_reference_identity(
            &self,
            reference: types::TypeReferenceHandle,
        ) -> Option<String> {
            if let Some(literal) = self.static_const_literal_from_type_reference(reference) {
                return self.static_const_argument_identity(&expression::StaticMachineArgument {
                    const_literal: Some(literal),
                    ..expression::StaticMachineArgument::default()
                });
            }
            match self.type_reference_table.type_reference(reference) {
                types::TypeReferenceNode::ConstExpression(value) => {
                    let expression::ExpressionNode::Boolean(value) =
                        self.expression_table.expression(*value)
                    else {
                        return None;
                    };
                    Some(static_const_identity_from_spelling(
                        &language_semantics::const_value::CanonicalConstValue::boolean(*value)
                            .atom(),
                    ))
                }
                types::TypeReferenceNode::Named { symbol, name } if symbol.is_valid() => self
                    .static_const_argument_identity(&expression::StaticMachineArgument {
                        symbol: *symbol,
                        path: vec![name.clone()].into_boxed_slice(),
                        ..expression::StaticMachineArgument::default()
                    }),
                types::TypeReferenceNode::Named { symbol, name } => self
                    .static_const_argument_identity(&expression::StaticMachineArgument {
                        symbol: *symbol,
                        path: vec![name.clone()].into_boxed_slice(),
                        ..expression::StaticMachineArgument::default()
                    })
                    .or_else(|| Some(self.normalized_type_identity_text(reference))),
                _ => None,
            }
        }

        pub fn static_binding_matches_argument(
            &self,
            kind: &crate::data::TypeParameterKind,
            value: &crate::static_applications::StaticMachineBindingValue,
            argument: &crate::expression::StaticMachineArgument,
            source_owner: symbols::SymbolHandle,
            binding_owner: symbols::SymbolHandle,
        ) -> bool {
            use crate::data::TypeParameterKind;
            use crate::static_applications::StaticMachineBindingValue;
            match (kind, value) {
                (TypeParameterKind::Type, StaticMachineBindingValue::Type(actual)) => {
                    use crate::type_identity::{IndexIdentityArgument, applied_index_identity};
                    applied_index_identity(
                        self,
                        IndexIdentityArgument::StaticArgument(argument),
                        source_owner,
                        &[],
                    )
                    .zip(applied_index_identity(
                        self,
                        IndexIdentityArgument::TypeArgument(*actual),
                        binding_owner,
                        &[],
                    ))
                    .is_some_and(|(expected, actual)| expected == actual)
                }
                (
                    TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. },
                    StaticMachineBindingValue::Const(actual),
                ) => {
                    if let Some((expected, actual)) = self
                        .static_const_argument_identity(argument)
                        .zip(self.static_const_type_reference_identity(*actual))
                    {
                        return expected == actual;
                    }
                    let expression = argument.type_reference.is_valid()
                        && matches!(
                            self.type_reference_table
                                .type_reference(argument.type_reference),
                            types::TypeReferenceNode::ConstExpression(_)
                        );
                    let value = if expression {
                        crate::type_identity::IndexIdentityArgument::StaticArgument(argument)
                    } else if argument.symbol.is_valid()
                        && self.symbols.get(argument.symbol).kind
                            == symbols::SymbolKind::TypeParameter
                        && self
                            .data_type_parameter_by_symbol(argument.symbol)
                            .is_some_and(|parameter| {
                                matches!(parameter.kind, TypeParameterKind::Const { .. })
                            })
                        && argument.const_literal.is_none()
                        && argument.application.is_none()
                        && argument.evidence_projection.is_none()
                        && (!argument.type_reference.is_valid()
                            || matches!(
                                self.type_reference_table.type_reference(argument.type_reference),
                                types::TypeReferenceNode::Named { symbol, .. } if *symbol == argument.symbol
                            ))
                    {
                        crate::type_identity::IndexIdentityArgument::Subject(argument.symbol)
                    } else {
                        return false;
                    };
                    crate::type_identity::applied_index_identity(self, value, source_owner, &[])
                        .zip(crate::type_identity::applied_index_identity(
                            self,
                            crate::type_identity::IndexIdentityArgument::TypeReference(*actual),
                            binding_owner,
                            &[],
                        ))
                        .is_some_and(|(expected, actual)| expected == actual)
                }
                (
                    TypeParameterKind::Machine {
                        contract: crate::data::MachineParameterContract::RequirementIdentity,
                    },
                    StaticMachineBindingValue::Machine(actual),
                )
                | (
                    TypeParameterKind::Implementation,
                    StaticMachineBindingValue::Conformance(actual),
                ) => {
                    self.same_static_machine_operand(argument, source_owner, actual, binding_owner)
                }
                (TypeParameterKind::Machine { .. }, StaticMachineBindingValue::Machine(actual)) => {
                    // Executable callable selections can name the machine or
                    // its entry. Declaration-only operands keep their exact
                    // authored correspondence in the preceding arm.
                    use crate::type_identity::{
                        IndexIdentityArgument, portable_applied_index_identity,
                    };
                    argument.const_literal == actual.const_literal
                        && argument.evidence_projection == actual.evidence_projection
                        && portable_applied_index_identity(
                            self,
                            IndexIdentityArgument::StaticArgument(argument),
                            source_owner,
                            &[],
                        )
                        .zip(portable_applied_index_identity(
                            self,
                            IndexIdentityArgument::StaticArgument(actual),
                            binding_owner,
                            &[],
                        ))
                        .is_some_and(|(left, right)| left == right)
                }
                _ => false,
            }
        }

        /// Whether two machine or conformance operands select the same thing.
        ///
        /// Each call spells its operand through its own type reference, so two
        /// calls that select one operand differ by handle and never compare
        /// equal field for field. Everything authored must still match exactly;
        /// only the type reference is compared by what it denotes, as type and
        /// constant operands already are.
        pub(crate) fn same_static_machine_operand(
            &self,
            left: &crate::expression::StaticMachineArgument,
            left_owner: symbols::SymbolHandle,
            right: &crate::expression::StaticMachineArgument,
            right_owner: symbols::SymbolHandle,
        ) -> bool {
            use crate::type_identity::applied_type_identity;
            self.expression_table
                .static_arguments_equal_with(left, right, &|left, right| {
                    left == right
                        || left.is_valid()
                            && right.is_valid()
                            && applied_type_identity(self, left, left_owner, &[])
                                .zip(applied_type_identity(self, right, right_owner, &[]))
                                .is_some_and(|(left, right)| left == right)
                })
        }

        pub fn access_field_plan_for_type_reference(
            &self,
            type_reference: types::TypeReferenceHandle,
        ) -> Option<&AccessFieldPlan> {
            self.access_view_field_plan_for_type_reference(type_reference)
                .map(|(_, field)| field)
        }

        pub fn access_view_plan_for_type_reference(
            &self,
            type_reference: types::TypeReferenceHandle,
        ) -> Option<&AccessViewPlan> {
            let view_symbol = self.type_reference_table.type_symbol(type_reference);
            if !view_symbol.is_valid() {
                return None;
            }
            self.access_view_plans
                .iter()
                .find(|view| view.data_symbol == view_symbol)
        }

        pub fn access_view_plan_for_input_type_reference(
            &self,
            type_reference: types::TypeReferenceHandle,
        ) -> Option<&AccessViewPlan> {
            let types::TypeReferenceNode::Reference { referee, .. } =
                self.type_reference_table.type_reference(type_reference)
            else {
                return None;
            };
            self.access_view_plan_for_type_reference(*referee)
        }

        pub fn access_view_field_plan_for_type_reference(
            &self,
            type_reference: types::TypeReferenceHandle,
        ) -> Option<(&AccessViewPlan, &AccessFieldPlan)> {
            let accessor_type =
                named_type_reference_through_shells(&self.type_reference_table, type_reference)?;
            self.access_view_plans.iter().find_map(|view| {
                view.fields
                    .iter()
                    .find(|field| {
                        self.type_reference_table
                            .type_reference(accessor_type)
                            .as_named_symbol()
                            == Some(field.accessor_data_symbol)
                            && (field.accessor_type == accessor_type
                                || field.scalar_projection_leaf(self).is_ok_and(|(_, leaf)| {
                                    self.normalized_type_identity(leaf)
                                        == self.normalized_type_identity(accessor_type)
                                }))
                    })
                    .map(|field| (view, field))
            })
        }

        pub fn named_type_reference(
            &self,
            type_reference: types::TypeReferenceHandle,
        ) -> Option<&name::Identifier> {
            self.type_reference_table.named_type(type_reference)
        }

        pub fn is_borrowed_byte_slice(&self, type_reference: types::TypeReferenceHandle) -> bool {
            self.type_reference_table
                .is_borrowed_byte_slice(type_reference)
        }

        pub fn arithmetic_domain_for_type_reference(
            &self,
            type_reference: types::TypeReferenceHandle,
        ) -> numerics::arithmetic::ArithmeticDomain {
            self.type_reference_table.arithmetic_domain(type_reference)
        }

        pub fn type_multiplicity(
            &self,
            type_reference: types::TypeReferenceHandle,
        ) -> language_semantics::Multiplicity {
            use language_semantics::Multiplicity;
            use types::TypeReferenceNode;

            if !type_reference.is_valid() {
                return Multiplicity::Affine;
            }
            if let Some(storage) = self.layout_storage_arguments(type_reference) {
                return self.type_multiplicity(storage.semantic_type);
            }
            match self.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { .. }
                | TypeReferenceNode::ConstExpression(_)
                | TypeReferenceNode::Unit => Multiplicity::Unrestricted,
                TypeReferenceNode::Constrained { base_type, .. } => {
                    self.type_multiplicity(*base_type)
                }
                TypeReferenceNode::FixedArray { element_type, .. } => {
                    self.stored_value_multiplicity(*element_type)
                }
                TypeReferenceNode::Named { symbol, name } => {
                    let parameter = if symbol.is_valid() {
                        self.first_data_type_parameter_by_symbol(*symbol)
                    } else {
                        self.data_type_parameters.iter().find_map(|(_, parameter)| {
                            (parameter.symbol == *symbol).then_some(parameter)
                        })
                    };
                    if let Some(parameter) = parameter {
                        return parameter.bounds.multiplicity;
                    }
                    if types::PrimitiveType::from_name(name.as_str()).is_some() {
                        return Multiplicity::Unrestricted;
                    }
                    let data_definitions = self.data_definitions();
                    data_definitions
                        .iter()
                        .find(|definition| symbol.is_valid() && definition.symbol == *symbol)
                        .or_else(|| {
                            data_definitions
                                .iter()
                                .find(|definition| definition.name.as_str() == name.as_str())
                        })
                        .map(|definition| definition.properties.multiplicity)
                        .unwrap_or(Multiplicity::Affine)
                }
                TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    ..
                } => {
                    let data_definitions = self.data_definitions();
                    data_definitions
                        .iter()
                        .find(|definition| {
                            base_symbol.is_valid() && definition.symbol == *base_symbol
                        })
                        .or_else(|| {
                            data_definitions
                                .iter()
                                .find(|definition| definition.name.as_str() == base_name.as_str())
                        })
                        .map(|definition| definition.properties.multiplicity)
                        .unwrap_or(Multiplicity::Affine)
                }
                TypeReferenceNode::Callable { .. } | TypeReferenceNode::DynamicTrait { .. } => {
                    Multiplicity::Unrestricted
                }
                TypeReferenceNode::Slice { .. } => Multiplicity::Affine,
            }
        }

        pub fn stored_value_multiplicity(
            &self,
            type_reference: types::TypeReferenceHandle,
        ) -> language_semantics::Multiplicity {
            match self.type_reference_table.type_reference(type_reference) {
                types::TypeReferenceNode::Reference { access, .. }
                    if *access != language_core::ReferenceAccess::Shared =>
                {
                    language_semantics::Multiplicity::Affine
                }
                types::TypeReferenceNode::Constrained { base_type, .. } => {
                    self.stored_value_multiplicity(*base_type)
                }
                _ => self.type_multiplicity(type_reference),
            }
        }

        pub fn type_reference_symbol(
            &self,
            type_reference: types::TypeReferenceHandle,
        ) -> symbols::SymbolHandle {
            self.type_reference_table.type_symbol(type_reference)
        }
    }
    #[cfg(test)]
    pub(crate) mod static_const_binding_tests {
        use crate::TypedTrees;
        use crate::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
        use crate::expression::{
            EvidenceProjection, ExpressionNode, StaticMachineArgument, StaticSymbolApplication,
            TableNamePath,
        };
        use crate::name::Identifier;
        use crate::static_applications::StaticMachineBindingValue;
        use crate::types::{TypeReferenceHandle, TypeReferenceNode};
        use numerics::literals::{IntegerLiteral, IntegerRadix};
        use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};

        macro_rules! same {
            ($fixture:expr => $argument:expr => $reference:expr => $owner:expr) => {
                matches($fixture, $argument, $reference, $owner)
            };
        }

        struct Fixture {
            program: TypedTrees,
            owner: SymbolHandle,
            foreign_owner: SymbolHandle,
            carrier: TypeReferenceHandle,
            parameters: [SymbolHandle; 6],
            references: [TypeReferenceHandle; 6],
        }

        fn fixture() -> Fixture {
            let mut builder = SymbolTableBuilder::default();
            let root =
                builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("binding_tests"));
            let roots = builder
                .insert_children(
                    root,
                    [
                        (SymbolKind::Machine, SymbolNameRef::Static("Owner")),
                        (SymbolKind::Machine, SymbolNameRef::Static("Foreign")),
                        (SymbolKind::Data, SymbolNameRef::Static("i32")),
                    ],
                )
                .handles()
                .collect::<Vec<_>>();
            let (owner, foreign_owner, carrier_symbol) = (roots[0], roots[1], roots[2]);
            let local = builder
                .insert_children(
                    owner,
                    [
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("Tag")),
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("Other")),
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("Type")),
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("Value")),
                        (
                            SymbolKind::MachineParameter,
                            SymbolNameRef::Static("Machine"),
                        ),
                    ],
                )
                .handles()
                .collect::<Vec<_>>();
            let foreign = builder
                .insert_children(
                    foreign_owner,
                    [(SymbolKind::TypeParameter, SymbolNameRef::Static("Tag"))],
                )
                .start();
            let parameters = [local[0], local[1], local[2], local[3], local[4], foreign];
            let mut program = TypedTrees {
                symbols: builder.finish(),
                ..TypedTrees::default()
            };
            let carrier = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: carrier_symbol,
                    name: Identifier::from("i32"),
                });
            let kinds = [
                TypeParameterKind::Const {
                    type_reference: carrier,
                },
                TypeParameterKind::Const {
                    type_reference: carrier,
                },
                TypeParameterKind::Type,
                TypeParameterKind::Value {
                    type_reference: carrier,
                },
                TypeParameterKind::Machine {
                    contract: MachineParameterContract::RequirementIdentity,
                },
                TypeParameterKind::Const {
                    type_reference: carrier,
                },
            ];
            for (symbol, kind) in parameters.into_iter().zip(kinds) {
                let name = Identifier::from(program.symbols.name(symbol));
                program.data_type_parameters.insert(TypeParameter {
                    symbol,
                    name,
                    kind,
                    ..TypeParameter::default()
                });
            }
            let references = parameters.map(|symbol| {
                let name = Identifier::from(program.symbols.name(symbol));
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named { symbol, name })
            });
            Fixture {
                program,
                owner,
                foreign_owner,
                carrier,
                parameters,
                references,
            }
        }

        fn named(fixture: &Fixture, index: usize) -> StaticMachineArgument {
            StaticMachineArgument {
                symbol: fixture.parameters[index],
                path: vec![Identifier::from(
                    fixture.program.symbols.name(fixture.parameters[index]),
                )]
                .into_boxed_slice(),
                ..StaticMachineArgument::default()
            }
        }

        fn matches(
            fixture: &Fixture,
            argument: &StaticMachineArgument,
            retained: TypeReferenceHandle,
            retained_owner: SymbolHandle,
        ) -> bool {
            fixture.program.static_binding_matches_argument(
                &TypeParameterKind::Const {
                    type_reference: fixture.carrier,
                },
                &StaticMachineBindingValue::Const(retained),
                argument,
                fixture.owner,
                retained_owner,
            )
        }

        fn integer(value: &str) -> IntegerLiteral {
            IntegerLiteral::from_parts(false, IntegerRadix::Decimal, value).unwrap()
        }

        #[test]
        fn open_const_rejoins_its_exact_retained_named_value() {
            let fixture = fixture();
            let argument = named(&fixture, 0);
            assert!(same!(&fixture => &argument => fixture.references[0] => fixture.owner));
            let typed_argument = StaticMachineArgument {
                type_reference: fixture.references[0],
                ..argument
            };
            assert!(same!(&fixture => &typed_argument => fixture.references[0] => fixture.owner));
        }

        #[test]
        fn another_const_or_same_spelled_foreign_const_is_not_the_selected_value() {
            let fixture = fixture();
            let argument = named(&fixture, 0);
            assert!(!same!(&fixture => &argument => fixture.references[1] => fixture.owner));
            assert!(
                !same!(&fixture => &argument => fixture.references[5] => fixture.foreign_owner)
            );
            assert!(
                !same!(&fixture => &named(&fixture, 5) => fixture.references[0] => fixture.owner)
            );
        }

        #[test]
        fn receiving_const_kind_does_not_reclassify_type_value_or_machine_binders() {
            let fixture = fixture();
            for index in [2, 3, 4] {
                let argument = StaticMachineArgument {
                    type_reference: fixture.references[index],
                    ..named(&fixture, index)
                };
                assert!(
                    !same!(&fixture => &argument => fixture.references[index] => fixture.owner)
                );
            }
        }

        #[test]
        fn named_const_rejects_conflicting_or_mixed_source_payloads() {
            let fixture = fixture();
            let original = named(&fixture, 0);
            let conflicting_reference = StaticMachineArgument {
                type_reference: fixture.references[1],
                ..original.clone()
            };
            assert!(
                !same!(&fixture => &conflicting_reference => fixture.references[0] => fixture.owner)
            );
            let mixed_literal = StaticMachineArgument {
                const_literal: Some(integer("7")),
                ..original.clone()
            };
            assert!(!same!(&fixture => &mixed_literal => fixture.references[0] => fixture.owner));
            let mixed_application = StaticMachineArgument {
                application: Some(Box::new(StaticSymbolApplication {
                    member: arena::Handle::invalid(),
                    lifetime_arguments: Box::default(),
                    arguments: Box::default(),
                })),
                ..original.clone()
            };
            assert!(
                !same!(&fixture => &mixed_application => fixture.references[0] => fixture.owner)
            );
            let mixed_evidence = StaticMachineArgument {
                evidence_projection: Some(EvidenceProjection {
                    term: Identifier::from("proof"),
                    member: Identifier::from("value"),
                }),
                ..original
            };
            assert!(!same!(&fixture => &mixed_evidence => fixture.references[0] => fixture.owner));
        }

        #[test]
        fn closed_literal_value_comparison_keeps_equal_and_unequal_values_distinct() {
            let mut fixture = fixture();
            let seven = fixture
                .program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: SymbolHandle::invalid(),
                    name: Identifier::from("7"),
                });
            let nineteen = fixture
                .program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: SymbolHandle::invalid(),
                    name: Identifier::from("19"),
                });
            let argument = StaticMachineArgument {
                const_literal: Some(integer("7")),
                ..StaticMachineArgument::default()
            };
            assert!(matches(&fixture, &argument, seven, fixture.owner));
            assert!(!matches(&fixture, &argument, nineteen, fixture.owner));
        }

        #[test]
        fn existing_const_expression_value_route_keeps_exact_open_subject_identity() {
            let mut fixture = fixture();
            let expression = fixture
                .program
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    head_symbol: fixture.parameters[0],
                    symbol: fixture.parameters[0],
                    ..TableNamePath::default()
                }));
            let reference = fixture
                .program
                .type_reference_table
                .insert(TypeReferenceNode::ConstExpression(expression));
            let argument = StaticMachineArgument {
                type_reference: reference,
                ..StaticMachineArgument::default()
            };
            assert!(same!(&fixture => &argument => fixture.references[0] => fixture.owner));
            assert!(!same!(&fixture => &argument => fixture.references[1] => fixture.owner));
        }
    }
}
