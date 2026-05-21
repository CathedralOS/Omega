use std::sync::Arc;

use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::source::SourceMap;
use omega_core::symbols::{
    BuiltinType, SymbolHandle, SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder,
    builtin_function_symbols, builtin_type_member_symbols, builtin_type_symbols,
};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

type SymbolSeed<'name> = (SymbolKind, SymbolNameRef<'name>);

pub(crate) fn assign_symbols(program: &mut SymbolResolvedTrees, sources: Option<Arc<SourceMap>>) {
    let symbols = build_symbol_table(program, sources);
    assign_top_level_symbols(program, &symbols);
    assign_type_reference_symbols(program, &symbols);
    assign_statement_call_symbols(program, &symbols);
    program.symbols = symbols;
}

fn build_symbol_table(
    program: &SymbolResolvedTrees,
    sources: Option<Arc<SourceMap>>,
) -> SymbolTable {
    let has_sources = sources.is_some();
    let mut builder = SymbolTableBuilder::with_sources(sources);
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let root_children =
        builder.insert_children(
            root,
            builtin_type_symbols()
                .into_iter()
                .chain(builtin_function_symbols())
                .chain(program.invariant_definitions.iter().map(|invariant| {
                    symbol_seed(SymbolKind::Invariant, &invariant.name, has_sources)
                }))
                .chain(
                    program
                        .data_definitions
                        .iter()
                        .map(|data| symbol_seed(SymbolKind::Data, &data.name, has_sources)),
                )
                .chain(
                    program.machines.iter().map(|machine| {
                        symbol_seed(SymbolKind::Machine, &machine.name, has_sources)
                    }),
                )
                .chain(
                    program.platforms.iter().map(|platform| {
                        symbol_seed(SymbolKind::Platform, &platform.name, has_sources)
                    }),
                )
                .chain(program.traits.iter().map(|trait_definition| {
                    symbol_seed(SymbolKind::Trait, &trait_definition.name, has_sources)
                })),
        );
    let mut root_children = SymbolTableBuilder::child_handles(root_children);

    for builtin_type in builtin_type_symbols() {
        if let Some(builtin_symbol) = root_children.next() {
            insert_builtin_type_symbol_children(&mut builder, builtin_symbol, builtin_type);
        }
    }
    for _ in 0..builtin_function_symbols().len() {
        let _ = root_children.next();
    }
    for _ in &program.invariant_definitions {
        let _ = root_children.next();
    }
    for data_definition in &program.data_definitions {
        if let Some(data_symbol) = root_children.next() {
            insert_data_symbol_children(
                &mut builder,
                program,
                data_symbol,
                data_definition,
                has_sources,
            );
        }
    }
    for machine in &program.machines {
        if let Some(machine_symbol) = root_children.next() {
            insert_machine_symbol_children(
                &mut builder,
                program,
                machine_symbol,
                machine,
                has_sources,
            );
        }
    }
    for platform in &program.platforms {
        if let Some(platform_symbol) = root_children.next() {
            insert_platform_symbol_children(
                &mut builder,
                program,
                platform_symbol,
                platform,
                has_sources,
            );
        }
    }
    for trait_definition in &program.traits {
        if let Some(trait_symbol) = root_children.next() {
            insert_trait_symbol_children(
                &mut builder,
                program,
                trait_symbol,
                trait_definition,
                has_sources,
            );
        }
    }

    builder.finish()
}

fn insert_builtin_type_symbol_children(
    builder: &mut SymbolTableBuilder,
    builtin_symbol: SymbolHandle,
    builtin_type: (SymbolKind, SymbolNameRef<'static>),
) {
    let SymbolNameRef::Static(name) = builtin_type.1 else {
        return;
    };
    let Some(builtin_type) = BuiltinType::from_name(name) else {
        return;
    };

    builder.insert_children(builtin_symbol, builtin_type_member_symbols(builtin_type));
}

fn insert_data_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    data_symbol: SymbolHandle,
    data_definition: &omega_symbol_resolved_trees::data::DataDefinition,
    has_sources: bool,
) {
    builder.insert_children(
        data_symbol,
        program
            .data_type_parameters(data_definition.type_parameters)
            .iter()
            .map(|parameter| symbol_seed(SymbolKind::TypeParameter, &parameter.name, has_sources))
            .chain(program.data_members(data_definition.members).iter().map(
                |member| match member {
                    omega_symbol_resolved_trees::data::DataMember::Field(field) => {
                        symbol_seed(SymbolKind::Field, &field.name, has_sources)
                    }
                    omega_symbol_resolved_trees::data::DataMember::Variant(variant) => {
                        symbol_seed(SymbolKind::Variant, &variant.name, has_sources)
                    }
                },
            )),
    );
}

fn insert_machine_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    machine_symbol: SymbolHandle,
    machine: &omega_symbol_resolved_trees::machine::Machine,
    has_sources: bool,
) {
    let inherited_field_count = inherited_data_field_symbols(program, machine, has_sources).count();
    let machine_children = builder.insert_children(
        machine_symbol,
        inherited_data_field_symbols(program, machine, has_sources)
            .chain(
                program
                    .machine_contained_objects(machine.contains)
                    .iter()
                    .map(|contained_object| {
                        symbol_seed(SymbolKind::Object, &contained_object.name, has_sources)
                    }),
            )
            .chain(
                program
                    .machine_owned_data(machine.owned_data)
                    .iter()
                    .map(|owned_data| {
                        symbol_seed(SymbolKind::Field, &owned_data.name, has_sources)
                    }),
            )
            .chain(
                program
                    .machine_state_handles(machine.states)
                    .iter()
                    .map(|state| program.machine_state(*state))
                    .map(|state| symbol_seed(SymbolKind::State, &state.name, has_sources)),
            ),
    );
    let mut machine_children = SymbolTableBuilder::child_handles(machine_children);

    for _ in 0..inherited_field_count {
        let _ = machine_children.next();
    }
    for _ in program.machine_contained_objects(machine.contains) {
        let _ = machine_children.next();
    }
    for _ in program.machine_owned_data(machine.owned_data) {
        let _ = machine_children.next();
    }
    for state in program.machine_state_handles(machine.states) {
        if let Some(state_symbol) = machine_children.next() {
            let state = program.machine_state(*state);
            insert_state_symbol_children(builder, program, state_symbol, state, has_sources);
        }
    }
}

fn insert_state_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    state_symbol: SymbolHandle,
    state: &omega_symbol_resolved_trees::state::State,
    has_sources: bool,
) {
    builder.insert_children(
        state_symbol,
        program
            .state_parameters(state.parameters)
            .iter()
            .map(|parameter| symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources))
            .chain(local_symbol_seeds(
                program.state_statements(state.statements),
                has_sources,
            )),
    );
}

fn insert_platform_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    platform_symbol: SymbolHandle,
    platform: &omega_symbol_resolved_trees::platform::Platform,
    has_sources: bool,
) {
    let platform_children = builder.insert_children(
        platform_symbol,
        program
            .platform_state_signatures(platform.states)
            .iter()
            .map(|state| symbol_seed(SymbolKind::State, &state.name, has_sources)),
    );

    for (state_symbol, state) in SymbolTableBuilder::child_handles(platform_children)
        .zip(program.platform_state_signatures(platform.states).iter())
    {
        builder.insert_children(
            state_symbol,
            program
                .state_parameters(state.parameters)
                .iter()
                .map(|parameter| symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources)),
        );
    }
}

fn insert_trait_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    trait_symbol: SymbolHandle,
    trait_definition: &omega_symbol_resolved_trees::trait_definition::TraitDefinition,
    has_sources: bool,
) {
    let trait_children = builder.insert_children(
        trait_symbol,
        program
            .trait_machine_signatures(trait_definition.machines)
            .iter()
            .map(|machine| symbol_seed(SymbolKind::State, &machine.name, has_sources)),
    );

    for (machine_symbol, machine) in SymbolTableBuilder::child_handles(trait_children).zip(
        program
            .trait_machine_signatures(trait_definition.machines)
            .iter(),
    ) {
        builder.insert_children(
            machine_symbol,
            program
                .state_parameters(machine.parameters)
                .iter()
                .map(|parameter| symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources)),
        );
    }
}

fn inherited_data_field_symbols<'program>(
    program: &'program SymbolResolvedTrees,
    machine: &'program omega_symbol_resolved_trees::machine::Machine,
    has_sources: bool,
) -> impl Iterator<Item = SymbolSeed<'program>> + 'program {
    program
        .data_definitions
        .iter()
        .find(|data_definition| data_definition.name == machine.name)
        .into_iter()
        .flat_map(|data_definition| program.data_members(data_definition.members).iter())
        .filter_map(move |member| match member {
            omega_symbol_resolved_trees::data::DataMember::Field(field) => {
                Some(symbol_seed(SymbolKind::Field, &field.name, has_sources))
            }
            omega_symbol_resolved_trees::data::DataMember::Variant(_) => None,
        })
}

fn local_symbol_seeds<'program>(
    statements: &'program [omega_symbol_resolved_trees::statement::Statement],
    has_sources: bool,
) -> impl Iterator<Item = SymbolSeed<'program>> + 'program {
    statements
        .iter()
        .filter_map(move |statement| match statement {
            omega_symbol_resolved_trees::statement::Statement::LocalData(local_data) => Some(
                symbol_seed(SymbolKind::Local, &local_data.name, has_sources),
            ),
            _ => None,
        })
}

fn symbol_seed<'name>(
    kind: SymbolKind,
    name: &'name omega_symbol_resolved_trees::name::DiagnosticName,
    has_sources: bool,
) -> SymbolSeed<'name> {
    if has_sources && name.is_source_backed() {
        (kind, SymbolNameRef::Source(name.source_span()))
    } else {
        (kind, SymbolNameRef::Borrowed(name.as_str()))
    }
}

fn assign_top_level_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
    let mut root_children = symbols.child_handles(symbols.root()).into_iter().flatten();

    for _ in 0..builtin_type_symbols().len() {
        let _ = root_children.next();
    }
    for _ in 0..builtin_function_symbols().len() {
        let _ = root_children.next();
    }

    program.invariant_definitions.for_each_mut(|invariant| {
        invariant.symbol = next_child_of_kind(&mut root_children, symbols, SymbolKind::Invariant);
    });

    let data_type_parameters = &mut program.tables.declarations.data_type_parameters;
    let data_members = &mut program.tables.declarations.data_members;
    program
        .roots
        .data_definitions
        .for_each_mut(|data_definition| {
            data_definition.symbol =
                next_child_of_kind(&mut root_children, symbols, SymbolKind::Data);
            let data_symbol = data_definition.symbol;
            let mut data_children = symbols.child_handles(data_symbol).into_iter().flatten();

            for type_parameter in
                data_type_parameters.span_mut_or_empty(data_definition.type_parameters)
            {
                type_parameter.symbol =
                    next_child_of_kind(&mut data_children, symbols, SymbolKind::TypeParameter);
            }

            for member in data_members.span_mut_or_empty(data_definition.members) {
                match member {
                    omega_symbol_resolved_trees::data::DataMember::Field(field) => {
                        field.symbol =
                            next_child_of_kind(&mut data_children, symbols, SymbolKind::Field);
                    }
                    omega_symbol_resolved_trees::data::DataMember::Variant(variant) => {
                        variant.symbol =
                            next_child_of_kind(&mut data_children, symbols, SymbolKind::Variant);
                    }
                }
            }
        });

    let tables = &mut program.tables;
    let declarations = &mut tables.declarations;
    let expression_table = &mut tables.bodies.expressions;
    let data_members = &declarations.data_members;
    let machine_contained_objects = &mut declarations.machine_contained_objects;
    let machine_owned_data = &mut declarations.machine_owned_data;
    let machine_state_handles = &declarations.machine_state_handles;
    let machine_states = &mut declarations.machine_states;
    let state_parameters = &mut declarations.state_parameters;
    let child_type_references = &mut declarations.child_type_references;
    let omega_symbol_resolved_trees::SymbolResolvedRoots {
        data_definitions,
        machines,
        ..
    } = &mut program.roots;
    machines.for_each_mut(|machine| {
        let inherited_field_count =
            inherited_field_count(data_definitions.iter(), data_members, &machine.name);
        machine.symbol = next_child_of_kind(&mut root_children, symbols, SymbolKind::Machine);
        let machine_symbol = machine.symbol;
        let mut machine_children = symbols.child_handles(machine_symbol).into_iter().flatten();

        for _ in 0..inherited_field_count {
            let _ = machine_children.next();
        }

        for contained_object in machine_contained_objects.span_mut_or_empty(machine.contains) {
            contained_object.symbol =
                next_child_of_kind(&mut machine_children, symbols, SymbolKind::Object);
            contained_object.type_symbol = top_level_symbol(
                symbols,
                SymbolKind::Machine,
                contained_object.type_name.as_str(),
            );
        }

        for owned_data in machine_owned_data.span_mut_or_empty(machine.owned_data) {
            owned_data.symbol =
                next_child_of_kind(&mut machine_children, symbols, SymbolKind::Field);
            assign_type_reference_symbol_with_self_type(
                symbols,
                child_type_references,
                machine_symbol,
                &mut owned_data.type_reference,
            );
            if owned_data.initial_value.is_valid() {
                assign_expression_table_symbols(
                    symbols,
                    &MachineScope {
                        symbol: machine_symbol,
                        owned_data: &[],
                        inherited_data_members: None,
                        contains: &[],
                    },
                    &[],
                    SymbolHandle::invalid(),
                    expression_table,
                    child_type_references,
                    owned_data.initial_value,
                );
            }
        }

        for state in machine_state_handles
            .span_or_empty(machine.states)
            .iter()
            .copied()
        {
            let state = machine_states.get_mut(state);
            state.symbol = next_child_of_kind(&mut machine_children, symbols, SymbolKind::State);
            let state_symbol = state.symbol;
            let mut state_children = symbols.child_handles(state_symbol).into_iter().flatten();

            for parameter in state_parameters.span_mut_or_empty(state.parameters) {
                parameter.symbol =
                    next_child_of_kind(&mut state_children, symbols, SymbolKind::Parameter);
                assign_type_reference_symbol_with_self_type(
                    symbols,
                    child_type_references,
                    machine_symbol,
                    &mut parameter.type_reference,
                );
            }

            for statement in declarations
                .state_statements
                .span_mut_or_empty(state.statements)
            {
                if let omega_symbol_resolved_trees::statement::Statement::LocalData(local_data) =
                    statement
                {
                    local_data.symbol =
                        next_child_of_kind(&mut state_children, symbols, SymbolKind::Local);
                }
            }

            if let Some(return_type) = &mut state.return_type {
                assign_type_reference_symbol_with_self_type(
                    symbols,
                    child_type_references,
                    machine_symbol,
                    return_type,
                );
            }
        }
    });

    let declarations = &mut program.tables.declarations;
    let platform_state_signatures = &mut declarations.platform_state_signatures;
    let state_parameters = &mut declarations.state_parameters;
    let child_type_references = &mut declarations.child_type_references;
    program.roots.platforms.for_each_mut(|platform| {
        platform.symbol = next_child_of_kind(&mut root_children, symbols, SymbolKind::Platform);
        let platform_symbol = platform.symbol;
        let mut platform_children = symbols.child_handles(platform_symbol).into_iter().flatten();

        for state in platform_state_signatures.span_mut_or_empty(platform.states) {
            state.symbol = next_child_of_kind(&mut platform_children, symbols, SymbolKind::State);
            let state_symbol = state.symbol;
            let mut state_children = symbols.child_handles(state_symbol).into_iter().flatten();

            for parameter in state_parameters.span_mut_or_empty(state.parameters) {
                parameter.symbol =
                    next_child_of_kind(&mut state_children, symbols, SymbolKind::Parameter);
                assign_type_reference_symbol_with_self_type(
                    symbols,
                    child_type_references,
                    platform_symbol,
                    &mut parameter.type_reference,
                );
            }

            if let Some(return_type) = &mut state.return_type {
                assign_type_reference_symbol_with_self_type(
                    symbols,
                    child_type_references,
                    platform_symbol,
                    return_type,
                );
            }
        }
    });

    let declarations = &mut program.tables.declarations;
    let trait_machine_signatures = &mut declarations.trait_machine_signatures;
    let state_parameters = &mut declarations.state_parameters;
    let child_type_references = &mut declarations.child_type_references;
    program.roots.traits.for_each_mut(|trait_definition| {
        trait_definition.symbol =
            next_child_of_kind(&mut root_children, symbols, SymbolKind::Trait);
        let trait_symbol = trait_definition.symbol;
        let mut trait_children = symbols.child_handles(trait_symbol).into_iter().flatten();

        for machine in trait_machine_signatures.span_mut_or_empty(trait_definition.machines) {
            machine.symbol = next_child_of_kind(&mut trait_children, symbols, SymbolKind::State);
            let machine_symbol = machine.symbol;
            let mut machine_children = symbols.child_handles(machine_symbol).into_iter().flatten();

            for parameter in state_parameters.span_mut_or_empty(machine.parameters) {
                parameter.symbol =
                    next_child_of_kind(&mut machine_children, symbols, SymbolKind::Parameter);
                assign_type_reference_symbol_with_self_type(
                    symbols,
                    child_type_references,
                    trait_symbol,
                    &mut parameter.type_reference,
                );
            }

            if let Some(return_type) = &mut machine.return_type {
                assign_type_reference_symbol_with_self_type(
                    symbols,
                    child_type_references,
                    trait_symbol,
                    return_type,
                );
            }
        }
    });
}

fn next_child_of_kind(
    children: &mut impl Iterator<Item = SymbolHandle>,
    symbols: &SymbolTable,
    kind: SymbolKind,
) -> SymbolHandle {
    let Some(child) = children.next() else {
        return SymbolHandle::invalid();
    };

    if symbols.get(child).kind == kind {
        child
    } else {
        SymbolHandle::invalid()
    }
}

fn inherited_field_count<'data>(
    data_definitions: impl IntoIterator<Item = &'data omega_symbol_resolved_trees::data::DataDefinition>,
    data_members: &Arena<omega_symbol_resolved_trees::data::DataMember>,
    machine_name: &omega_symbol_resolved_trees::name::DiagnosticName,
) -> usize {
    data_definitions
        .into_iter()
        .find(|data_definition| data_definition.name == *machine_name)
        .map(|data_definition| {
            data_members
                .span_or_empty(data_definition.members)
                .iter()
                .filter(|member| {
                    matches!(
                        member,
                        omega_symbol_resolved_trees::data::DataMember::Field(_)
                    )
                })
                .count()
        })
        .unwrap_or(0)
}

fn assign_type_reference_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
    let data_type_parameters = &program.tables.declarations.data_type_parameters;
    let data_members = &mut program.tables.declarations.data_members;
    let child_type_references = &mut program.tables.declarations.child_type_references;
    program
        .roots
        .data_definitions
        .for_each_mut(|data_definition| {
            let type_parameters =
                data_type_parameters.span_or_empty(data_definition.type_parameters);
            for member in data_members.span_mut_or_empty(data_definition.members) {
                if let omega_symbol_resolved_trees::data::DataMember::Field(field) = member {
                    assign_type_reference_symbol_with_locals(
                        symbols,
                        child_type_references,
                        type_parameters,
                        &mut field.type_reference,
                    );
                }
            }
        });
}

fn assign_statement_call_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
    let SymbolResolvedTrees {
        roots:
            omega_symbol_resolved_trees::SymbolResolvedRoots {
                data_definitions,
                machines,
                ..
            },
        tables,
        ..
    } = program;
    let data_members = &tables.declarations.data_members;
    let machine_contained_objects = &tables.declarations.machine_contained_objects;
    let machine_owned_data = &tables.declarations.machine_owned_data;
    let machine_state_handles = &tables.declarations.machine_state_handles;
    let machine_states = &mut tables.declarations.machine_states;
    let state_parameters = &tables.declarations.state_parameters;
    let statement_path_members = &tables.declarations.statement_path_members;
    let expression_table = &mut tables.bodies.expressions;
    let state_statements = &mut tables.declarations.state_statements;
    let child_type_references = &mut tables.declarations.child_type_references;
    machines.for_each_mut(|machine| {
        let machine_symbol = machine.symbol;
        let data_definition = data_definitions
            .iter()
            .find(|data_definition| data_definition.name == machine.name);
        let inherited_data_members = data_definition
            .map(|data_definition| data_members.span_or_empty(data_definition.members));
        let omega_symbol_resolved_trees::machine::MachineStorage {
            contains,
            owned_data,
            states,
        } = &mut machine.storage;
        let machine_scope = MachineScope {
            symbol: machine_symbol,
            contains: machine_contained_objects.span_or_empty(*contains),
            inherited_data_members,
            owned_data: machine_owned_data.span_or_empty(*owned_data),
        };
        for state in machine_state_handles.span_or_empty(*states).iter().copied() {
            let state = machine_states.get_mut(state);
            let state_symbol = state.symbol;
            let parameters = state_parameters.span_or_empty(state.parameters);
            for statement in state_statements.span_mut_or_empty(state.statements) {
                assign_statement_symbols(
                    &machine_scope,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    statement_path_members,
                    statement,
                    symbols,
                );
            }
        }
    });
}

struct MachineScope<'program> {
    symbol: SymbolHandle,
    contains: &'program [omega_symbol_resolved_trees::machine::ContainedObject],
    inherited_data_members: Option<&'program [omega_symbol_resolved_trees::data::DataMember]>,
    owned_data: &'program [omega_symbol_resolved_trees::machine::OwnedData],
}

impl MachineScope<'_> {
    fn field_type_reference(
        &self,
        field_symbol: SymbolHandle,
    ) -> Option<&omega_symbol_resolved_trees::types::TypeReference> {
        if let Some(data_members) = self.inherited_data_members {
            for member in data_members {
                let omega_symbol_resolved_trees::data::DataMember::Field(field) = member else {
                    continue;
                };
                if field.symbol == field_symbol {
                    return Some(&field.type_reference);
                }
            }
        }

        self.owned_data
            .iter()
            .find(|owned_data| owned_data.symbol == field_symbol)
            .map(|owned_data| &owned_data.type_reference)
    }
}

fn assign_statement_symbols(
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    statement_path_members: &Arena<omega_symbol_resolved_trees::name::DiagnosticName>,
    statement: &mut omega_symbol_resolved_trees::statement::Statement,
    symbols: &SymbolTable,
) {
    match statement {
        omega_symbol_resolved_trees::statement::Statement::Assignment(assignment) => {
            assign_statement_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                assignment.target,
            );
            assign_statement_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                assignment.value,
            );
        }
        omega_symbol_resolved_trees::statement::Statement::Call(call) => {
            assign_expression_span_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                call.arguments,
            );
            if !call.receiver.is_empty() {
                let (head_symbol, symbol) = resolve_state_scoped_members(
                    symbols,
                    machine.symbol,
                    state_symbol,
                    statement_path_members.span_or_empty(call.receiver),
                    call.receiver_starts_at_self,
                );
                if symbol.is_valid() {
                    let _ = head_symbol;
                    call.receiver_symbol = symbol;
                }
            }

            call.target_symbol = resolve_call_target_symbol(
                machine,
                parameters,
                !call.receiver.is_empty(),
                call.receiver_symbol,
                &call.target,
                child_type_references,
                symbols,
            );
        }
        omega_symbol_resolved_trees::statement::Statement::Expression(expression) => {
            assign_statement_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                *expression,
            );
        }
        omega_symbol_resolved_trees::statement::Statement::LocalData(local_data) => {
            assign_type_reference_symbol_with_self_type(
                symbols,
                child_type_references,
                machine.symbol,
                &mut local_data.type_reference,
            );
            if local_data.initial_value.is_valid() {
                assign_statement_expression_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    local_data.initial_value,
                );
            }
        }
        omega_symbol_resolved_trees::statement::Statement::Transition(transition) => {
            if let omega_symbol_resolved_trees::statement::TransitionGuard::When(expression) =
                &mut transition.guard
            {
                assign_statement_expression_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    *expression,
                );
            }
            assign_transition_target_symbols(
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                statement_path_members,
                &mut transition.target,
                symbols,
            );
            if let Some(continuation) = &mut transition.continuation {
                assign_transition_target_symbols(
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    statement_path_members,
                    continuation,
                    symbols,
                );
            }
        }
    }
}

fn assign_statement_expression_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
) {
    if !expression.is_valid() {
        return;
    }

    assign_expression_table_symbols(
        symbols,
        machine,
        parameters,
        state_symbol,
        expression_table,
        child_type_references,
        expression,
    );
}

fn assign_expression_span_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    expressions: HandleSpan<omega_symbol_resolved_trees::expression::ExpressionHandle>,
) {
    let count = expressions.count();
    for offset in 0..count {
        let expression = expression_table.expression_handles(expressions)[offset as usize];
        assign_expression_table_symbols(
            symbols,
            machine,
            parameters,
            state_symbol,
            expression_table,
            child_type_references,
            expression,
        );
    }
}

fn assign_expression_table_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
) {
    if !expression.is_valid() {
        return;
    }

    match expression_table.expression(expression).clone() {
        omega_symbol_resolved_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            assign_expression_span_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                values,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Binary(binary) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                binary.left,
            );
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                binary.right,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Boolean(_)
        | omega_symbol_resolved_trees::expression::ExpressionNode::Float(_)
        | omega_symbol_resolved_trees::expression::ExpressionNode::Integer(_)
        | omega_symbol_resolved_trees::expression::ExpressionNode::String(_) => {}
        omega_symbol_resolved_trees::expression::ExpressionNode::Cast(cast) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                cast.value,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                assign_expression_table_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    call.receiver,
                );
            }
            assign_expression_span_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                call.arguments,
            );
            let (head_symbol, symbol) = resolve_expression_table_receiver_path_symbols(
                symbols,
                machine.symbol,
                state_symbol,
                expression_table,
                call.receiver,
            );
            if symbol.is_valid() {
                stamp_receiver_path_symbols_in_table(
                    expression_table,
                    call.receiver,
                    head_symbol,
                    symbol,
                );
            }
            let target_symbol = resolve_expression_table_call_target_symbol(
                machine,
                parameters,
                state_symbol,
                &call,
                expression_table,
                child_type_references,
                symbols,
            );
            if let omega_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
                expression_table.expression_mut(expression)
            {
                call.target_symbol = target_symbol;
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                indexed.collection,
            );
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                indexed.index,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                member.receiver,
            );
            let member_symbol = resolve_expression_table_member_symbol(
                symbols,
                machine.symbol,
                state_symbol,
                expression_table,
                member.receiver,
                &member.member,
            );
            if let (symbol, omega_symbol_resolved_trees::expression::ExpressionNode::Member(member)) =
                (member_symbol, expression_table.expression_mut(expression))
                && symbol.is_valid()
            {
                member.member_symbol = symbol;
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            assign_expression_table_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                inner,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            let (head_symbol, symbol) = resolve_state_scoped_table_path(
                symbols,
                machine.symbol,
                state_symbol,
                expression_table,
                &path,
            );
            if symbol.is_valid() {
                if let omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
                    expression_table.expression_mut(expression)
                {
                    path.head_symbol = head_symbol;
                    path.symbol = symbol;
                }
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
            let count = struct_literal.fields.count();
            for offset in 0..count {
                let field = &expression_table.struct_fields(struct_literal.fields)[offset as usize];
                assign_expression_table_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    field.value,
                );
            }
        }
    }
}

fn stamp_receiver_path_symbols_in_table(
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
    head_symbol: SymbolHandle,
    symbol: SymbolHandle,
) {
    match expression_table.expression(expression).clone() {
        omega_symbol_resolved_trees::expression::ExpressionNode::Name(_) => {
            if let omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
                expression_table.expression_mut(expression)
            {
                path.head_symbol = head_symbol;
                path.symbol = symbol;
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            stamp_receiver_path_head_symbol_in_table(
                expression_table,
                indexed.collection,
                head_symbol,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            stamp_receiver_path_head_symbol_in_table(
                expression_table,
                member.receiver,
                head_symbol,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            stamp_receiver_path_symbols_in_table(expression_table, inner, head_symbol, symbol);
        }
        _ => {}
    }
}

fn stamp_receiver_path_head_symbol_in_table(
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
    head_symbol: SymbolHandle,
) {
    match expression_table.expression(expression).clone() {
        omega_symbol_resolved_trees::expression::ExpressionNode::Name(_) => {
            if let omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
                expression_table.expression_mut(expression)
            {
                path.head_symbol = head_symbol;
                path.symbol = head_symbol;
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            stamp_receiver_path_head_symbol_in_table(
                expression_table,
                indexed.collection,
                head_symbol,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            stamp_receiver_path_head_symbol_in_table(
                expression_table,
                member.receiver,
                head_symbol,
            );
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            stamp_receiver_path_head_symbol_in_table(expression_table, inner, head_symbol);
        }
        _ => {}
    }
}

fn resolve_expression_table_call_target_symbol(
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    call: &omega_symbol_resolved_trees::expression::TableCallExpression,
    expression_table: &omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    symbols: &SymbolTable,
) -> SymbolHandle {
    if call.receiver.is_valid() {
        let receiver_symbol = resolve_expression_table_receiver_symbol(
            symbols,
            machine.symbol,
            state_symbol,
            expression_table,
            call.receiver,
        );
        return resolve_call_target_symbol(
            machine,
            parameters,
            true,
            receiver_symbol,
            &call.target,
            child_type_references,
            symbols,
        );
    }

    resolve_call_target_symbol(
        machine,
        parameters,
        false,
        SymbolHandle::invalid(),
        &call.target,
        child_type_references,
        symbols,
    )
}

fn resolve_expression_table_member_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &omega_symbol_resolved_trees::expression::ExpressionTable,
    receiver: omega_symbol_resolved_trees::expression::ExpressionHandle,
    member: &omega_symbol_resolved_trees::name::DiagnosticName,
) -> SymbolHandle {
    let receiver_symbol = resolve_expression_table_receiver_symbol(
        symbols,
        machine_symbol,
        state_symbol,
        expression_table,
        receiver,
    );
    if !receiver_symbol.is_valid() {
        return SymbolHandle::invalid();
    }
    let member_symbol = child_symbol_by_kinds(
        symbols,
        receiver_symbol,
        &[
            SymbolKind::Field,
            SymbolKind::Object,
            SymbolKind::State,
            SymbolKind::Parameter,
            SymbolKind::Variant,
        ],
        member.as_str(),
    );

    member_symbol
}

fn resolve_expression_table_receiver_path_symbols(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &omega_symbol_resolved_trees::expression::ExpressionTable,
    receiver: omega_symbol_resolved_trees::expression::ExpressionHandle,
) -> (SymbolHandle, SymbolHandle) {
    match expression_table.expression(receiver) {
        omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            resolve_state_scoped_table_path(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                path,
            )
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            let (head_symbol, receiver_symbol) = resolve_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                member.receiver,
            );
            if !receiver_symbol.is_valid() {
                return invalid_symbol_pair();
            }
            let member_symbol = child_symbol_by_kinds(
                symbols,
                receiver_symbol,
                &[
                    SymbolKind::Field,
                    SymbolKind::Object,
                    SymbolKind::State,
                    SymbolKind::Parameter,
                    SymbolKind::Variant,
                ],
                member.member.as_str(),
            );

            if member_symbol.is_valid() {
                (head_symbol, member_symbol)
            } else {
                invalid_symbol_pair()
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            resolve_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                *inner,
            )
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            let omega_symbol_resolved_trees::expression::ExpressionNode::Integer(index) =
                expression_table.expression(indexed.index)
            else {
                return invalid_symbol_pair();
            };

            resolve_indexed_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                indexed.collection,
                *index,
            )
        }
        _ => invalid_symbol_pair(),
    }
}

fn resolve_indexed_expression_table_receiver_path_symbols(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &omega_symbol_resolved_trees::expression::ExpressionTable,
    collection: omega_symbol_resolved_trees::expression::ExpressionHandle,
    index: i64,
) -> (SymbolHandle, SymbolHandle) {
    match expression_table.expression(collection) {
        omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            resolve_state_scoped_table_path_with_indexed_last_member(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                path,
                index,
            )
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            let (head_symbol, receiver_symbol) = resolve_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                member.receiver,
            );
            if !receiver_symbol.is_valid() {
                return invalid_symbol_pair();
            }
            let member_symbol = child_indexed_symbol_by_kinds(
                symbols,
                receiver_symbol,
                &[
                    SymbolKind::Field,
                    SymbolKind::Object,
                    SymbolKind::State,
                    SymbolKind::Parameter,
                    SymbolKind::Variant,
                ],
                member.member.as_str(),
                index,
            );

            if member_symbol.is_valid() {
                (head_symbol, member_symbol)
            } else {
                invalid_symbol_pair()
            }
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            resolve_indexed_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                *inner,
                index,
            )
        }
        _ => invalid_symbol_pair(),
    }
}

fn resolve_expression_table_receiver_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &omega_symbol_resolved_trees::expression::ExpressionTable,
    receiver: omega_symbol_resolved_trees::expression::ExpressionHandle,
) -> SymbolHandle {
    match expression_table.expression(receiver) {
        omega_symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            let (_, symbol) = resolve_state_scoped_table_path(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                path,
            );
            symbol
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            resolve_expression_table_member_symbol(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                member.receiver,
                &member.member,
            )
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Mutable(inner) => {
            resolve_expression_table_receiver_symbol(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                *inner,
            )
        }
        omega_symbol_resolved_trees::expression::ExpressionNode::Indexed(_) => {
            let (_, symbol) = resolve_expression_table_receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                receiver,
            );
            symbol
        }
        _ => SymbolHandle::invalid(),
    }
}

fn resolve_state_scoped_table_path(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &omega_symbol_resolved_trees::expression::ExpressionTable,
    path: &omega_symbol_resolved_trees::expression::TableNamePath,
) -> (SymbolHandle, SymbolHandle) {
    let members = expression_table.name_path_members(path.members);
    resolve_state_scoped_table_members(
        symbols,
        machine_symbol,
        state_symbol,
        members,
        path.is_self_value,
        None,
    )
}

fn resolve_state_scoped_table_path_with_indexed_last_member(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &omega_symbol_resolved_trees::expression::ExpressionTable,
    path: &omega_symbol_resolved_trees::expression::TableNamePath,
    index: i64,
) -> (SymbolHandle, SymbolHandle) {
    let members = expression_table.name_path_members(path.members);
    resolve_state_scoped_table_members(
        symbols,
        machine_symbol,
        state_symbol,
        members,
        path.is_self_value,
        Some(index),
    )
}

fn resolve_state_scoped_table_members(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    members: &[omega_symbol_resolved_trees::name::DiagnosticName],
    starts_at_self: bool,
    indexed_last_member: Option<i64>,
) -> (SymbolHandle, SymbolHandle) {
    if members.is_empty() {
        return invalid_symbol_pair();
    }

    let mut index = 0usize;
    let mut current = SymbolHandle::invalid();
    let head: SymbolHandle;

    if starts_at_self {
        current = machine_symbol;
        index = 1;
    }

    if index >= members.len() {
        return if current.is_valid() {
            (current, current)
        } else {
            invalid_symbol_pair()
        };
    }

    if !current.is_valid() {
        current = if indexed_last_member.is_some() && index + 1 == members.len() {
            let indexed_symbol = resolve_base_indexed_symbol(
                symbols,
                machine_symbol,
                state_symbol,
                members[index].as_str(),
                indexed_last_member.expect("indexed last member should be present"),
            );
            if !indexed_symbol.is_valid() {
                return invalid_symbol_pair();
            }
            indexed_symbol
        } else {
            let base_symbol =
                resolve_base_symbol(symbols, machine_symbol, state_symbol, &members[index]);
            if !base_symbol.is_valid() {
                return invalid_symbol_pair();
            }
            base_symbol
        };
        head = current;
        index += 1;
    } else {
        current = if indexed_last_member.is_some() && index + 1 == members.len() {
            child_indexed_symbol_by_kinds(
                symbols,
                current,
                &[SymbolKind::Field, SymbolKind::Object, SymbolKind::State],
                members[index].as_str(),
                indexed_last_member.expect("indexed last member should be present"),
            )
        } else {
            child_symbol_by_kinds(
                symbols,
                current,
                &[SymbolKind::Field, SymbolKind::Object, SymbolKind::State],
                members[index].as_str(),
            )
        };
        if !current.is_valid() {
            return invalid_symbol_pair();
        }
        head = current;
        index += 1;
    }

    for (offset, member) in members[index..].iter().enumerate() {
        let is_last = index + offset + 1 == members.len();
        current = if indexed_last_member.is_some() && is_last {
            child_indexed_symbol_by_kinds(
                symbols,
                current,
                &[
                    SymbolKind::Field,
                    SymbolKind::Object,
                    SymbolKind::State,
                    SymbolKind::Parameter,
                    SymbolKind::Variant,
                ],
                member.as_str(),
                indexed_last_member.expect("indexed last member should be present"),
            )
        } else {
            child_symbol_by_kinds(
                symbols,
                current,
                &[
                    SymbolKind::Field,
                    SymbolKind::Object,
                    SymbolKind::State,
                    SymbolKind::Parameter,
                    SymbolKind::Variant,
                ],
                member.as_str(),
            )
        };
        if !current.is_valid() {
            return invalid_symbol_pair();
        }
    }

    (head, current)
}

fn resolve_base_indexed_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    member: &str,
    index: i64,
) -> SymbolHandle {
    if state_symbol.is_valid() {
        let parameter_symbol = child_indexed_symbol_by_kinds(
            symbols,
            state_symbol,
            &[SymbolKind::Parameter],
            member,
            index,
        );
        if parameter_symbol.is_valid() {
            return parameter_symbol;
        }
    }

    let machine_child = child_indexed_symbol_by_kinds(
        symbols,
        machine_symbol,
        &[SymbolKind::Field, SymbolKind::Object, SymbolKind::State],
        member,
        index,
    );
    machine_child
}

fn assign_transition_target_symbols(
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    statement_path_members: &Arena<omega_symbol_resolved_trees::name::DiagnosticName>,
    target: &mut omega_symbol_resolved_trees::statement::TransitionTarget,
    symbols: &SymbolTable,
) {
    let omega_symbol_resolved_trees::statement::TransitionTarget::Named(named) = target else {
        if let omega_symbol_resolved_trees::statement::TransitionTarget::Value(expression) = target
        {
            assign_statement_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                *expression,
            );
        }
        return;
    };

    assign_expression_span_symbols(
        symbols,
        machine,
        parameters,
        state_symbol,
        expression_table,
        child_type_references,
        named.arguments,
    );

    let path = statement_path_members.span_or_empty(named.path);
    let target_name = path.last().cloned();
    let (head_symbol, symbol) = resolve_state_scoped_members(
        symbols,
        machine.symbol,
        state_symbol,
        path,
        named.path_starts_at_self,
    );
    if symbol.is_valid() {
        named.head_symbol = head_symbol;
        named.symbol = symbol;
        return;
    }

    let Some(target_name) = target_name else {
        return;
    };

    if path.len() <= 2 {
        let target_symbol = child_symbol_by_kinds(
            symbols,
            machine.symbol,
            &[SymbolKind::State],
            target_name.as_str(),
        );
        if target_symbol.is_valid() {
            named.head_symbol = target_symbol;
            named.symbol = target_symbol;
        }
    }
}

fn resolve_call_target_symbol(
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    has_receiver: bool,
    receiver_symbol: SymbolHandle,
    target: &omega_symbol_resolved_trees::name::DiagnosticName,
    child_type_references: &omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    symbols: &SymbolTable,
) -> SymbolHandle {
    if has_receiver {
        if receiver_symbol.is_valid() {
            if let Some(contained) = machine
                .contains
                .iter()
                .find(|contained| contained.symbol == receiver_symbol)
            {
                return child_symbol_by_kinds(
                    symbols,
                    contained.type_symbol,
                    &[SymbolKind::State],
                    target.as_str(),
                );
            }

            if let Some(field_type_reference) = machine.field_type_reference(receiver_symbol) {
                let symbol = call_target_for_type_reference(
                    symbols,
                    child_type_references,
                    field_type_reference,
                    target.as_str(),
                );
                return symbol;
            }

            let receiver_kind = symbols.get(receiver_symbol).kind;
            if let Some(parameter) = parameters
                .iter()
                .find(|parameter| parameter.symbol == receiver_symbol)
            {
                let direct = call_target_for_type_reference(
                    symbols,
                    child_type_references,
                    &parameter.type_reference,
                    target.as_str(),
                );
                if direct.is_valid() {
                    return direct;
                }
            }
            if matches!(receiver_kind, SymbolKind::BuiltinType) {
                return child_symbol_by_kinds(
                    symbols,
                    receiver_symbol,
                    &[SymbolKind::BuiltinFunction],
                    target.as_str(),
                );
            }
            if matches!(
                receiver_kind,
                SymbolKind::Machine | SymbolKind::Platform | SymbolKind::Trait
            ) {
                return child_symbol_by_kinds(
                    symbols,
                    receiver_symbol,
                    &[SymbolKind::State],
                    target.as_str(),
                );
            }
        }
    }

    let machine_state = child_symbol_by_kinds(
        symbols,
        machine.symbol,
        &[SymbolKind::State],
        target.as_str(),
    );
    if machine_state.is_valid() {
        return machine_state;
    }

    top_level_symbol_by_kinds(symbols, &[SymbolKind::BuiltinFunction], target.as_str())
}

fn resolve_state_scoped_members(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    members: &[omega_symbol_resolved_trees::name::DiagnosticName],
    starts_at_self: bool,
) -> (SymbolHandle, SymbolHandle) {
    if members.is_empty() {
        return invalid_symbol_pair();
    }

    let mut index = 0usize;
    let mut current = SymbolHandle::invalid();
    let head: SymbolHandle;

    if starts_at_self {
        current = machine_symbol;
        index = 1;
    }

    if index >= members.len() {
        return if current.is_valid() {
            (current, current)
        } else {
            invalid_symbol_pair()
        };
    }

    if !current.is_valid() {
        current = resolve_base_symbol(symbols, machine_symbol, state_symbol, &members[index]);
        if !current.is_valid() {
            return invalid_symbol_pair();
        }
        head = current;
        index += 1;
    } else {
        current = child_symbol_by_kinds(
            symbols,
            current,
            &[SymbolKind::Field, SymbolKind::Object, SymbolKind::State],
            members[index].as_str(),
        );
        if !current.is_valid() {
            return invalid_symbol_pair();
        }
        head = current;
        index += 1;
    }

    for member in &members[index..] {
        current = child_symbol_by_kinds(
            symbols,
            current,
            &[
                SymbolKind::Field,
                SymbolKind::Object,
                SymbolKind::State,
                SymbolKind::Parameter,
                SymbolKind::Variant,
            ],
            member.as_str(),
        );
        if !current.is_valid() {
            return invalid_symbol_pair();
        }
    }

    (head, current)
}

fn invalid_symbol_pair() -> (SymbolHandle, SymbolHandle) {
    (SymbolHandle::invalid(), SymbolHandle::invalid())
}

fn resolve_base_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    member: &omega_symbol_resolved_trees::name::DiagnosticName,
) -> SymbolHandle {
    if state_symbol.is_valid() {
        let parameter_symbol = child_symbol_by_kinds(
            symbols,
            state_symbol,
            &[SymbolKind::Parameter],
            member.as_str(),
        );
        if parameter_symbol.is_valid() {
            return parameter_symbol;
        }
    }

    let machine_child = child_symbol_by_kinds(
        symbols,
        machine_symbol,
        &[SymbolKind::Field, SymbolKind::Object, SymbolKind::State],
        member.as_str(),
    );
    if machine_child.is_valid() {
        return machine_child;
    }

    let top_level = top_level_symbol_by_kinds(
        symbols,
        &[
            SymbolKind::BuiltinType,
            SymbolKind::Data,
            SymbolKind::Machine,
            SymbolKind::Platform,
            SymbolKind::Trait,
            SymbolKind::Invariant,
        ],
        member.as_str(),
    );
    top_level
}

fn type_reference_symbol(
    child_type_references: &omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    type_reference: &omega_symbol_resolved_trees::types::TypeReference,
) -> SymbolHandle {
    match type_reference {
        omega_symbol_resolved_trees::types::TypeReference::Reference(reference) => {
            type_reference_symbol(
                child_type_references,
                child_type_references.get(reference.referee),
            )
        }
        omega_symbol_resolved_trees::types::TypeReference::Constrained(constrained) => {
            type_reference_symbol(
                child_type_references,
                child_type_references.get(constrained.base_type),
            )
        }
        omega_symbol_resolved_trees::types::TypeReference::FixedArray(fixed_array) => {
            type_reference_symbol(
                child_type_references,
                child_type_references.get(fixed_array.element_type),
            )
        }
        omega_symbol_resolved_trees::types::TypeReference::Slice(slice) => type_reference_symbol(
            child_type_references,
            child_type_references.get(slice.element_type),
        ),
        omega_symbol_resolved_trees::types::TypeReference::Generic(generic) => generic.base_symbol,
        omega_symbol_resolved_trees::types::TypeReference::Named { symbol, .. } => *symbol,
        omega_symbol_resolved_trees::types::TypeReference::SelfType { symbol } => *symbol,
        omega_symbol_resolved_trees::types::TypeReference::Unit => SymbolHandle::invalid(),
    }
}

fn call_target_for_type_reference(
    symbols: &SymbolTable,
    child_type_references: &omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    type_reference: &omega_symbol_resolved_trees::types::TypeReference,
    target_name: &str,
) -> SymbolHandle {
    child_symbol_by_kinds(
        symbols,
        type_reference_symbol(child_type_references, type_reference),
        &[SymbolKind::State],
        target_name,
    )
}

fn assign_type_reference_symbol_with_self_type(
    symbols: &SymbolTable,
    child_type_references: &mut omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    self_type_symbol: SymbolHandle,
    type_reference: &mut omega_symbol_resolved_trees::types::TypeReference,
) {
    assign_type_reference_symbol_with_context(
        symbols,
        child_type_references,
        &[],
        self_type_symbol,
        type_reference,
    );
}

fn assign_type_reference_symbol_with_locals(
    symbols: &SymbolTable,
    child_type_references: &mut omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    local_type_parameters: &[omega_symbol_resolved_trees::data::TypeParameter],
    type_reference: &mut omega_symbol_resolved_trees::types::TypeReference,
) {
    assign_type_reference_symbol_with_context(
        symbols,
        child_type_references,
        local_type_parameters,
        SymbolHandle::invalid(),
        type_reference,
    );
}

fn assign_type_reference_symbol_with_context(
    symbols: &SymbolTable,
    child_type_references: &mut omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    local_type_parameters: &[omega_symbol_resolved_trees::data::TypeParameter],
    self_type_symbol: SymbolHandle,
    type_reference: &mut omega_symbol_resolved_trees::types::TypeReference,
) {
    match type_reference {
        omega_symbol_resolved_trees::types::TypeReference::Reference(reference) => {
            assign_type_reference_handle_symbol_with_context(
                symbols,
                child_type_references,
                local_type_parameters,
                self_type_symbol,
                reference.referee,
            );
        }
        omega_symbol_resolved_trees::types::TypeReference::Constrained(constrained) => {
            assign_type_reference_handle_symbol_with_context(
                symbols,
                child_type_references,
                local_type_parameters,
                self_type_symbol,
                constrained.base_type,
            );
        }
        omega_symbol_resolved_trees::types::TypeReference::FixedArray(fixed_array) => {
            assign_type_reference_handle_symbol_with_context(
                symbols,
                child_type_references,
                local_type_parameters,
                self_type_symbol,
                fixed_array.element_type,
            );
        }
        omega_symbol_resolved_trees::types::TypeReference::Slice(slice) => {
            assign_type_reference_handle_symbol_with_context(
                symbols,
                child_type_references,
                local_type_parameters,
                self_type_symbol,
                slice.element_type,
            );
        }
        omega_symbol_resolved_trees::types::TypeReference::Generic(generic) => {
            generic.base_symbol =
                resolve_type_symbol(symbols, local_type_parameters, &generic.base_name);

            assign_type_reference_argument_symbols(
                symbols,
                child_type_references,
                local_type_parameters,
                self_type_symbol,
                generic.arguments,
            );
        }
        omega_symbol_resolved_trees::types::TypeReference::Named { symbol, name } => {
            *symbol = resolve_type_symbol(symbols, local_type_parameters, name);
        }
        omega_symbol_resolved_trees::types::TypeReference::SelfType { symbol } => {
            *symbol = self_type_symbol;
        }
        omega_symbol_resolved_trees::types::TypeReference::Unit => {}
    }
}

fn assign_type_reference_handle_symbol_with_context(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<omega_symbol_resolved_trees::types::TypeReference>,
    local_type_parameters: &[omega_symbol_resolved_trees::data::TypeParameter],
    self_type_symbol: SymbolHandle,
    handle: Handle<omega_symbol_resolved_trees::types::TypeReference>,
) {
    let mut type_reference = std::mem::take(child_type_references.get_mut(handle));
    assign_type_reference_symbol_with_context(
        symbols,
        child_type_references,
        local_type_parameters,
        self_type_symbol,
        &mut type_reference,
    );
    *child_type_references.get_mut(handle) = type_reference;
}

fn assign_type_reference_argument_symbols(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<omega_symbol_resolved_trees::types::TypeReference>,
    local_type_parameters: &[omega_symbol_resolved_trees::data::TypeParameter],
    self_type_symbol: SymbolHandle,
    arguments: HandleSpan<omega_symbol_resolved_trees::types::TypeReference>,
) {
    let start = arguments.start();
    let generation = start.generation();

    for offset in 0..arguments.count() {
        let handle = Handle::from_parts(
            start
                .arena_index()
                .checked_add(offset)
                .expect("type reference argument handle overflow"),
            generation,
        );
        let mut argument = std::mem::take(child_type_references.get_mut(handle));
        assign_type_reference_symbol_with_context(
            symbols,
            child_type_references,
            local_type_parameters,
            self_type_symbol,
            &mut argument,
        );
        *child_type_references.get_mut(handle) = argument;
    }
}

fn resolve_type_symbol(
    symbols: &SymbolTable,
    local_type_parameters: &[omega_symbol_resolved_trees::data::TypeParameter],
    name: &omega_symbol_resolved_trees::name::DiagnosticName,
) -> SymbolHandle {
    local_type_parameters
        .iter()
        .find(|parameter| parameter.name.as_str() == name.as_str())
        .map(|parameter| parameter.symbol)
        .unwrap_or_else(|| top_level_type_symbol(symbols, name.as_str()))
}

fn top_level_type_symbol(symbols: &SymbolTable, name: &str) -> SymbolHandle {
    top_level_symbol_by_kinds(
        symbols,
        &[
            SymbolKind::BuiltinType,
            SymbolKind::Data,
            SymbolKind::Machine,
            SymbolKind::Platform,
            SymbolKind::Trait,
            SymbolKind::Invariant,
        ],
        name,
    )
}

fn top_level_symbol(symbols: &SymbolTable, kind: SymbolKind, name: &str) -> SymbolHandle {
    top_level_symbol_by_kinds(symbols, &[kind], name)
}

fn top_level_symbol_by_kinds(
    symbols: &SymbolTable,
    kinds: &[SymbolKind],
    name: &str,
) -> SymbolHandle {
    child_symbol_by_kinds(symbols, symbols.root(), kinds, name)
}

fn child_symbol_by_kinds(
    symbols: &SymbolTable,
    parent: SymbolHandle,
    kinds: &[SymbolKind],
    name: &str,
) -> SymbolHandle {
    child_symbol_by_kinds_matching(symbols, parent, kinds, |symbol_name| symbol_name == name)
}

fn child_indexed_symbol_by_kinds(
    symbols: &SymbolTable,
    parent: SymbolHandle,
    kinds: &[SymbolKind],
    name: &str,
    index: i64,
) -> SymbolHandle {
    child_symbol_by_kinds_matching(symbols, parent, kinds, |symbol_name| {
        symbol_name_matches_indexed_member(symbol_name, name, index)
    })
}

fn child_symbol_by_kinds_matching(
    symbols: &SymbolTable,
    parent: SymbolHandle,
    kinds: &[SymbolKind],
    mut matches_name: impl FnMut(&str) -> bool,
) -> SymbolHandle {
    let Some(children) = symbols.child_handles(parent) else {
        return SymbolHandle::invalid();
    };

    for child in children {
        let symbol = symbols.get(child);
        if matches_name(symbols.name(child)) && kinds.contains(&symbol.kind) {
            return child;
        }
    }

    SymbolHandle::invalid()
}

fn symbol_name_matches_indexed_member(symbol_name: &str, member: &str, index: i64) -> bool {
    let Some(suffix) = symbol_name.strip_prefix(member) else {
        return false;
    };
    let Some(suffix) = suffix.strip_prefix('[') else {
        return false;
    };
    let Some(index_text) = suffix.strip_suffix(']') else {
        return false;
    };

    index_text.parse::<i64>().ok() == Some(index)
}
