use std::sync::Arc;

use omega_core::arena::Arena;
use omega_core::source::SourceMap;
use omega_core::symbols::{
    SymbolHandle, SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder, builtin_type_symbols,
};
use omega_resolved_trees::SymbolResolvedTrees;

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
                .chain(program.platforms.iter().map(|platform| {
                    symbol_seed(SymbolKind::Platform, &platform.name, has_sources)
                })),
        );
    let mut root_children = SymbolTableBuilder::child_handles(root_children);

    for _ in 0..builtin_type_symbols().len() {
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

    builder.finish()
}

fn insert_data_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    data_symbol: SymbolHandle,
    data_definition: &omega_resolved_trees::data::DataDefinition,
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
                    omega_resolved_trees::data::DataMember::Field(field) => {
                        symbol_seed(SymbolKind::Field, &field.name, has_sources)
                    }
                    omega_resolved_trees::data::DataMember::Variant(variant) => {
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
    machine: &omega_resolved_trees::machine::Machine,
    has_sources: bool,
) {
    let inherited_field_count = inherited_data_field_symbols(program, machine, has_sources).count();
    let machine_children =
        builder.insert_children(
            machine_symbol,
            inherited_data_field_symbols(program, machine, has_sources)
                .chain(machine.contains.iter().map(|contained_object| {
                    symbol_seed(SymbolKind::Object, &contained_object.name, has_sources)
                }))
                .chain(machine.owned_data.iter().map(|owned_data| {
                    symbol_seed(SymbolKind::Field, &owned_data.name, has_sources)
                }))
                .chain(
                    machine
                        .states
                        .iter()
                        .map(|state| symbol_seed(SymbolKind::State, &state.name, has_sources)),
                ),
        );
    let mut machine_children = SymbolTableBuilder::child_handles(machine_children);

    for _ in 0..inherited_field_count {
        let _ = machine_children.next();
    }
    for _ in &machine.contains {
        let _ = machine_children.next();
    }
    for _ in &machine.owned_data {
        let _ = machine_children.next();
    }
    for state in &machine.states {
        if let Some(state_symbol) = machine_children.next() {
            insert_state_symbol_children(builder, program, state_symbol, state, has_sources);
        }
    }
}

fn insert_state_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    state_symbol: SymbolHandle,
    state: &omega_resolved_trees::state::State,
    has_sources: bool,
) {
    builder.insert_children(
        state_symbol,
        program
            .state_parameters(state.parameters)
            .iter()
            .map(|parameter| symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources))
            .chain(local_symbol_seeds(&state.statements, has_sources)),
    );
}

fn insert_platform_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    platform_symbol: SymbolHandle,
    platform: &omega_resolved_trees::platform::Platform,
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

fn inherited_data_field_symbols<'program>(
    program: &'program SymbolResolvedTrees,
    machine: &'program omega_resolved_trees::machine::Machine,
    has_sources: bool,
) -> impl Iterator<Item = SymbolSeed<'program>> + 'program {
    program
        .data_definitions
        .iter()
        .find(|data_definition| data_definition.name == machine.name)
        .into_iter()
        .flat_map(|data_definition| program.data_members(data_definition.members).iter())
        .filter_map(move |member| match member {
            omega_resolved_trees::data::DataMember::Field(field) => {
                Some(symbol_seed(SymbolKind::Field, &field.name, has_sources))
            }
            omega_resolved_trees::data::DataMember::Variant(_) => None,
        })
}

fn local_symbol_seeds<'program>(
    statements: &'program [omega_resolved_trees::statement::Statement],
    has_sources: bool,
) -> impl Iterator<Item = SymbolSeed<'program>> + 'program {
    statements
        .iter()
        .filter_map(move |statement| match statement {
            omega_resolved_trees::statement::Statement::LocalData(local_data) => Some(symbol_seed(
                SymbolKind::Local,
                &local_data.name,
                has_sources,
            )),
            _ => None,
        })
}

fn symbol_seed<'name>(
    kind: SymbolKind,
    name: &'name omega_resolved_trees::name::DiagnosticName,
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
                    omega_resolved_trees::data::DataMember::Field(field) => {
                        field.symbol =
                            next_child_of_kind(&mut data_children, symbols, SymbolKind::Field);
                    }
                    omega_resolved_trees::data::DataMember::Variant(variant) => {
                        variant.symbol =
                            next_child_of_kind(&mut data_children, symbols, SymbolKind::Variant);
                    }
                }
            }
        });

    let data_members = &program.tables.declarations.data_members;
    let state_parameters = &mut program.tables.declarations.state_parameters;
    let omega_resolved_trees::SymbolResolvedRoots {
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

        for contained_object in &mut machine.contains {
            contained_object.symbol =
                next_child_of_kind(&mut machine_children, symbols, SymbolKind::Object);
            contained_object.type_symbol = top_level_symbol(
                symbols,
                SymbolKind::Machine,
                contained_object.type_name.as_str(),
            );
        }

        for owned_data in &mut machine.owned_data {
            owned_data.symbol =
                next_child_of_kind(&mut machine_children, symbols, SymbolKind::Field);
            assign_type_reference_symbol_with_self_type(
                symbols,
                machine_symbol,
                &mut owned_data.type_reference,
            );
        }

        for state in &mut machine.states {
            state.symbol = next_child_of_kind(&mut machine_children, symbols, SymbolKind::State);
            let state_symbol = state.symbol;
            let mut state_children = symbols.child_handles(state_symbol).into_iter().flatten();

            for parameter in state_parameters.span_mut_or_empty(state.parameters) {
                parameter.symbol =
                    next_child_of_kind(&mut state_children, symbols, SymbolKind::Parameter);
                assign_type_reference_symbol_with_self_type(
                    symbols,
                    machine_symbol,
                    &mut parameter.type_reference,
                );
            }

            for statement in &mut state.statements {
                if let omega_resolved_trees::statement::Statement::LocalData(local_data) = statement
                {
                    local_data.symbol =
                        next_child_of_kind(&mut state_children, symbols, SymbolKind::Local);
                }
            }

            if let Some(return_type) = &mut state.return_type {
                assign_type_reference_symbol_with_self_type(symbols, machine_symbol, return_type);
            }
        }
    });

    let platform_state_signatures = &mut program.tables.declarations.platform_state_signatures;
    let state_parameters = &mut program.tables.declarations.state_parameters;
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
                    platform_symbol,
                    &mut parameter.type_reference,
                );
            }

            if let Some(return_type) = &mut state.return_type {
                assign_type_reference_symbol_with_self_type(symbols, platform_symbol, return_type);
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
    data_definitions: impl IntoIterator<Item = &'data omega_resolved_trees::data::DataDefinition>,
    data_members: &Arena<omega_resolved_trees::data::DataMember>,
    machine_name: &omega_resolved_trees::name::DiagnosticName,
) -> usize {
    data_definitions
        .into_iter()
        .find(|data_definition| data_definition.name == *machine_name)
        .map(|data_definition| {
            data_members
                .span_or_empty(data_definition.members)
                .iter()
                .filter(|member| matches!(member, omega_resolved_trees::data::DataMember::Field(_)))
                .count()
        })
        .unwrap_or(0)
}

fn assign_type_reference_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
    let data_type_parameters = &program.tables.declarations.data_type_parameters;
    let data_members = &mut program.tables.declarations.data_members;
    program
        .roots
        .data_definitions
        .for_each_mut(|data_definition| {
            let type_parameters =
                data_type_parameters.span_or_empty(data_definition.type_parameters);
            for member in data_members.span_mut_or_empty(data_definition.members) {
                if let omega_resolved_trees::data::DataMember::Field(field) = member {
                    assign_type_reference_symbol_with_locals(
                        symbols,
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
            omega_resolved_trees::SymbolResolvedRoots {
                data_definitions,
                machines,
                ..
            },
        tables,
        ..
    } = program;
    let data_members = &tables.declarations.data_members;
    let state_parameters = &tables.declarations.state_parameters;
    machines.for_each_mut(|machine| {
        let machine_symbol = machine.symbol;
        let data_definition = data_definitions
            .iter()
            .find(|data_definition| data_definition.name == machine.name);
        let inherited_data_members = data_definition
            .map(|data_definition| data_members.span_or_empty(data_definition.members));
        let omega_resolved_trees::machine::MachineStorage {
            contains,
            owned_data,
            states,
        } = &mut machine.storage;
        let machine_scope = MachineScope {
            symbol: machine_symbol,
            contains: contains.as_slice(),
            inherited_data_members,
            owned_data: owned_data.as_slice(),
        };
        for state in states {
            let state_symbol = state.symbol;
            let parameters = state_parameters.span_or_empty(state.parameters);
            for statement in &mut state.storage.statements {
                assign_statement_symbols(
                    &machine_scope,
                    parameters,
                    state_symbol,
                    statement,
                    symbols,
                );
            }
        }
    });
}

struct MachineScope<'program> {
    symbol: SymbolHandle,
    contains: &'program [omega_resolved_trees::machine::ContainedObject],
    inherited_data_members: Option<&'program [omega_resolved_trees::data::DataMember]>,
    owned_data: &'program [omega_resolved_trees::machine::OwnedData],
}

impl MachineScope<'_> {
    fn field_type_reference(
        &self,
        field_symbol: SymbolHandle,
    ) -> Option<&omega_resolved_trees::types::TypeReference> {
        if let Some(data_members) = self.inherited_data_members {
            for member in data_members {
                let omega_resolved_trees::data::DataMember::Field(field) = member else {
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
    parameters: &[omega_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    statement: &mut omega_resolved_trees::statement::Statement,
    symbols: &SymbolTable,
) {
    match statement {
        omega_resolved_trees::statement::Statement::Assignment(assignment) => {
            assign_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                &mut assignment.target,
            );
            assign_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                &mut assignment.value,
            );
        }
        omega_resolved_trees::statement::Statement::Call(call) => {
            for argument in &mut call.arguments {
                assign_expression_symbols(symbols, machine, parameters, state_symbol, argument);
            }
            if let Some(receiver) = &mut call.receiver {
                if let Some((head_symbol, symbol)) =
                    resolve_state_scoped_path(symbols, machine.symbol, state_symbol, receiver)
                {
                    *receiver = receiver.clone().with_symbols(head_symbol, symbol);
                    call.receiver_symbol = symbol;
                }
            }

            call.target_symbol = resolve_call_target_symbol(
                machine,
                parameters,
                call.receiver.is_some(),
                call.receiver_symbol,
                &call.target,
                symbols,
            );
        }
        omega_resolved_trees::statement::Statement::Expression(expression) => {
            assign_expression_symbols(symbols, machine, parameters, state_symbol, expression);
        }
        omega_resolved_trees::statement::Statement::LocalData(local_data) => {
            assign_type_reference_symbol_with_self_type(
                symbols,
                machine.symbol,
                &mut local_data.type_reference,
            );
            if let Some(initial_value) = &mut local_data.initial_value {
                assign_expression_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    initial_value,
                );
            }
        }
        omega_resolved_trees::statement::Statement::Transition(transition) => {
            if let omega_resolved_trees::statement::TransitionGuard::When(expression) =
                &mut transition.guard
            {
                assign_expression_symbols(symbols, machine, parameters, state_symbol, expression);
            }
            assign_transition_target_symbols(
                machine,
                state_symbol,
                &mut transition.target,
                symbols,
            );
            if let Some(continuation) = &mut transition.continuation {
                assign_transition_target_symbols(machine, state_symbol, continuation, symbols);
            }
        }
    }
}

fn assign_expression_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[omega_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression: &mut omega_resolved_trees::expression::Expression,
) {
    match expression {
        omega_resolved_trees::expression::Expression::ArrayLiteral(array_literal) => {
            for value in &mut array_literal.values {
                assign_expression_symbols(symbols, machine, parameters, state_symbol, value);
            }
        }
        omega_resolved_trees::expression::Expression::Binary(binary) => {
            assign_expression_symbols(symbols, machine, parameters, state_symbol, &mut binary.left);
            assign_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                &mut binary.right,
            );
        }
        omega_resolved_trees::expression::Expression::Boolean(_)
        | omega_resolved_trees::expression::Expression::Float(_)
        | omega_resolved_trees::expression::Expression::Integer(_)
        | omega_resolved_trees::expression::Expression::String(_) => {}
        omega_resolved_trees::expression::Expression::Cast(cast) => {
            assign_expression_symbols(symbols, machine, parameters, state_symbol, &mut cast.value);
        }
        omega_resolved_trees::expression::Expression::Call(call) => {
            if let Some(receiver) = &mut call.receiver {
                assign_expression_symbols(symbols, machine, parameters, state_symbol, receiver);
            }
            for argument in &mut call.arguments {
                assign_expression_symbols(symbols, machine, parameters, state_symbol, argument);
            }
            if let Some(receiver_path) = call
                .receiver
                .as_ref()
                .and_then(|receiver| expression_name_path(receiver))
            {
                if let Some((head_symbol, symbol)) =
                    resolve_state_scoped_path(symbols, machine.symbol, state_symbol, &receiver_path)
                {
                    if let Some(receiver) = &mut call.receiver {
                        *receiver = Box::new(expression_from_path(
                            receiver_path.with_symbols(head_symbol, symbol),
                        ));
                    }
                }
            }
            call.target_symbol =
                resolve_expression_call_target_symbol(machine, parameters, call, symbols);
        }
        omega_resolved_trees::expression::Expression::Indexed(indexed) => {
            assign_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                &mut indexed.collection,
            );
            assign_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                &mut indexed.index,
            );
        }
        omega_resolved_trees::expression::Expression::Member(member) => {
            assign_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                &mut member.receiver,
            );
            if let Some(path) = expression_name_path(
                &omega_resolved_trees::expression::Expression::Member(member.clone()),
            ) && let Some((_, symbol)) =
                resolve_state_scoped_path(symbols, machine.symbol, state_symbol, &path)
            {
                member.member_symbol = symbol;
            }
        }
        omega_resolved_trees::expression::Expression::Mutable(inner) => {
            assign_expression_symbols(symbols, machine, parameters, state_symbol, inner);
        }
        omega_resolved_trees::expression::Expression::Name(path) => {
            if let Some((head_symbol, symbol)) =
                resolve_state_scoped_path(symbols, machine.symbol, state_symbol, path)
            {
                *path = path.clone().with_symbols(head_symbol, symbol);
            }
        }
        omega_resolved_trees::expression::Expression::StructLiteral(struct_literal) => {
            for field in &mut struct_literal.fields {
                assign_expression_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    &mut field.value,
                );
            }
        }
    }
}

fn expression_from_path(
    path: omega_resolved_trees::expression::NamePath,
) -> omega_resolved_trees::expression::Expression {
    let head_symbol = path.head_symbol();
    let mut members = path.members().iter().cloned();
    let Some(first) = members.next() else {
        return omega_resolved_trees::expression::Expression::Name(path);
    };
    let mut expression = omega_resolved_trees::expression::Expression::Name(
        omega_resolved_trees::expression::NamePath::resolved(vec![first], head_symbol, head_symbol),
    );
    for member in members {
        expression = omega_resolved_trees::expression::Expression::Member(Box::new(
            omega_resolved_trees::expression::MemberExpression {
                storage: omega_resolved_trees::expression::MemberExpressionStorage {
                    receiver: expression,
                    member_symbol: SymbolHandle::invalid(),
                    member,
                },
            },
        ));
    }
    expression
}

fn resolve_expression_call_target_symbol(
    machine: &MachineScope<'_>,
    parameters: &[omega_resolved_trees::signature::StateParameter],
    call: &omega_resolved_trees::expression::CallExpression,
    symbols: &SymbolTable,
) -> SymbolHandle {
    if let Some(receiver) = &call.receiver
        && let Some(receiver_path) = expression_name_path(receiver)
    {
        return resolve_call_target_symbol(
            machine,
            parameters,
            true,
            receiver_path.symbol(),
            &call.target,
            symbols,
        );
    }

    resolve_call_target_symbol(
        machine,
        parameters,
        false,
        SymbolHandle::invalid(),
        &call.target,
        symbols,
    )
}

fn expression_name_path(
    expression: &omega_resolved_trees::expression::Expression,
) -> Option<omega_resolved_trees::expression::NamePath> {
    match expression {
        omega_resolved_trees::expression::Expression::Name(path) => Some(path.clone()),
        omega_resolved_trees::expression::Expression::Member(member) => {
            let mut path = expression_name_path(&member.receiver)?;
            path.push(member.member.clone());
            Some(path)
        }
        omega_resolved_trees::expression::Expression::Indexed(indexed) => {
            let omega_resolved_trees::expression::Expression::Integer(index) = &indexed.index
            else {
                return None;
            };
            let mut path = expression_name_path(&indexed.collection)?;
            let last_segment = path.last_mut()?;
            *last_segment = omega_resolved_trees::name::DiagnosticName::generated(format!(
                "{last_segment}[{index}]"
            ));
            Some(path)
        }
        omega_resolved_trees::expression::Expression::Mutable(inner) => expression_name_path(inner),
        _ => None,
    }
}

fn assign_transition_target_symbols(
    machine: &MachineScope<'_>,
    state_symbol: SymbolHandle,
    target: &mut omega_resolved_trees::statement::TransitionTarget,
    symbols: &SymbolTable,
) {
    let omega_resolved_trees::statement::TransitionTarget::Named(named) = target else {
        return;
    };

    let path = &mut named.path;
    let target_name = path.last().cloned();
    if let Some((head_symbol, symbol)) =
        resolve_state_scoped_path(symbols, machine.symbol, state_symbol, path)
    {
        *path = path.clone().with_symbols(head_symbol, symbol);
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
            *path = path.clone().with_symbols(target_symbol, target_symbol);
        }
    }
}

fn resolve_call_target_symbol(
    machine: &MachineScope<'_>,
    parameters: &[omega_resolved_trees::signature::StateParameter],
    has_receiver: bool,
    receiver_symbol: SymbolHandle,
    target: &omega_resolved_trees::name::DiagnosticName,
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
                let symbol =
                    call_target_for_type_reference(symbols, field_type_reference, target.as_str());
                return symbol;
            }

            let receiver_kind = symbols.get(receiver_symbol).kind;
            if let Some(parameter) = parameters
                .iter()
                .find(|parameter| parameter.symbol == receiver_symbol)
            {
                let direct = call_target_for_type_reference(
                    symbols,
                    &parameter.type_reference,
                    target.as_str(),
                );
                if direct.is_valid() {
                    return direct;
                }
            }
            if matches!(receiver_kind, SymbolKind::Machine | SymbolKind::Platform) {
                return child_symbol_by_kinds(
                    symbols,
                    receiver_symbol,
                    &[SymbolKind::State],
                    target.as_str(),
                );
            }
        }
    }

    child_symbol_by_kinds(
        symbols,
        machine.symbol,
        &[SymbolKind::State],
        target.as_str(),
    )
}

fn resolve_state_scoped_path(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    path: &omega_resolved_trees::expression::NamePath,
) -> Option<(SymbolHandle, SymbolHandle)> {
    let members = path.members();
    if members.is_empty() {
        return None;
    }

    let mut index = 0usize;
    let mut current = SymbolHandle::invalid();
    let head: SymbolHandle;

    if members
        .first()
        .is_some_and(|member| member.as_str() == "self")
    {
        current = machine_symbol;
        index = 1;
    }

    if index >= members.len() {
        return current.is_valid().then_some((current, current));
    }

    if !current.is_valid() {
        current = resolve_base_symbol(symbols, machine_symbol, state_symbol, &members[index])?;
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
            return None;
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
            return None;
        }
    }

    Some((head, current))
}

fn resolve_base_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    member: &omega_resolved_trees::name::DiagnosticName,
) -> Option<SymbolHandle> {
    if state_symbol.is_valid() {
        let parameter_symbol = child_symbol_by_kinds(
            symbols,
            state_symbol,
            &[SymbolKind::Parameter],
            member.as_str(),
        );
        if parameter_symbol.is_valid() {
            return Some(parameter_symbol);
        }
    }

    let machine_child = child_symbol_by_kinds(
        symbols,
        machine_symbol,
        &[SymbolKind::Field, SymbolKind::Object, SymbolKind::State],
        member.as_str(),
    );
    if machine_child.is_valid() {
        return Some(machine_child);
    }

    let top_level = top_level_symbol_by_kinds(
        symbols,
        &[
            SymbolKind::BuiltinType,
            SymbolKind::Data,
            SymbolKind::Machine,
            SymbolKind::Platform,
            SymbolKind::Invariant,
        ],
        member.as_str(),
    );
    top_level.is_valid().then_some(top_level)
}

fn type_reference_symbol(
    type_reference: &omega_resolved_trees::types::TypeReference,
) -> SymbolHandle {
    match type_reference {
        omega_resolved_trees::types::TypeReference::Reference(reference) => {
            type_reference_symbol(&reference.referee)
        }
        omega_resolved_trees::types::TypeReference::Constrained(constrained) => {
            type_reference_symbol(&constrained.base_type)
        }
        omega_resolved_trees::types::TypeReference::FixedArray(fixed_array) => {
            type_reference_symbol(&fixed_array.element_type)
        }
        omega_resolved_trees::types::TypeReference::Slice(slice) => {
            type_reference_symbol(&slice.element_type)
        }
        omega_resolved_trees::types::TypeReference::Generic(generic) => generic.base_symbol,
        omega_resolved_trees::types::TypeReference::Named { symbol, .. } => *symbol,
        omega_resolved_trees::types::TypeReference::SelfType { symbol } => *symbol,
        omega_resolved_trees::types::TypeReference::Unit => SymbolHandle::invalid(),
    }
}

fn type_reference_name(
    type_reference: &omega_resolved_trees::types::TypeReference,
) -> Option<&omega_resolved_trees::name::DiagnosticName> {
    match type_reference {
        omega_resolved_trees::types::TypeReference::Reference(reference) => {
            type_reference_name(&reference.referee)
        }
        omega_resolved_trees::types::TypeReference::Constrained(constrained) => {
            type_reference_name(&constrained.base_type)
        }
        omega_resolved_trees::types::TypeReference::FixedArray(fixed_array) => {
            type_reference_name(&fixed_array.element_type)
        }
        omega_resolved_trees::types::TypeReference::Slice(slice) => {
            type_reference_name(&slice.element_type)
        }
        omega_resolved_trees::types::TypeReference::Generic(generic) => Some(&generic.base_name),
        omega_resolved_trees::types::TypeReference::Named { name, .. } => Some(name),
        omega_resolved_trees::types::TypeReference::SelfType { .. }
        | omega_resolved_trees::types::TypeReference::Unit => None,
    }
}

fn call_target_for_type_reference(
    symbols: &SymbolTable,
    type_reference: &omega_resolved_trees::types::TypeReference,
    target_name: &str,
) -> SymbolHandle {
    let direct = child_symbol_by_kinds(
        symbols,
        type_reference_symbol(type_reference),
        &[SymbolKind::State],
        target_name,
    );
    if direct.is_valid() {
        return direct;
    }

    let Some(type_name) = type_reference_name(type_reference) else {
        return direct;
    };
    let callable_type = top_level_symbol_by_kinds(
        symbols,
        &[SymbolKind::Machine, SymbolKind::Platform],
        type_name.as_str(),
    );
    if callable_type.is_valid() {
        return child_symbol_by_kinds(symbols, callable_type, &[SymbolKind::State], target_name);
    }

    direct
}

fn assign_type_reference_symbol_with_self_type(
    symbols: &SymbolTable,
    self_type_symbol: SymbolHandle,
    type_reference: &mut omega_resolved_trees::types::TypeReference,
) {
    assign_type_reference_symbol_with_context(symbols, &[], self_type_symbol, type_reference);
}

fn assign_type_reference_symbol_with_locals(
    symbols: &SymbolTable,
    local_type_parameters: &[omega_resolved_trees::data::TypeParameter],
    type_reference: &mut omega_resolved_trees::types::TypeReference,
) {
    assign_type_reference_symbol_with_context(
        symbols,
        local_type_parameters,
        SymbolHandle::invalid(),
        type_reference,
    );
}

fn assign_type_reference_symbol_with_context(
    symbols: &SymbolTable,
    local_type_parameters: &[omega_resolved_trees::data::TypeParameter],
    self_type_symbol: SymbolHandle,
    type_reference: &mut omega_resolved_trees::types::TypeReference,
) {
    match type_reference {
        omega_resolved_trees::types::TypeReference::Reference(reference) => {
            assign_type_reference_symbol_with_context(
                symbols,
                local_type_parameters,
                self_type_symbol,
                &mut reference.referee,
            );
        }
        omega_resolved_trees::types::TypeReference::Constrained(constrained) => {
            assign_type_reference_symbol_with_context(
                symbols,
                local_type_parameters,
                self_type_symbol,
                &mut constrained.base_type,
            );
        }
        omega_resolved_trees::types::TypeReference::FixedArray(fixed_array) => {
            assign_type_reference_symbol_with_context(
                symbols,
                local_type_parameters,
                self_type_symbol,
                &mut fixed_array.element_type,
            );
        }
        omega_resolved_trees::types::TypeReference::Slice(slice) => {
            assign_type_reference_symbol_with_context(
                symbols,
                local_type_parameters,
                self_type_symbol,
                &mut slice.element_type,
            );
        }
        omega_resolved_trees::types::TypeReference::Generic(generic) => {
            generic.base_symbol =
                resolve_type_symbol(symbols, local_type_parameters, &generic.base_name);

            for argument in &mut generic.arguments {
                assign_type_reference_symbol_with_context(
                    symbols,
                    local_type_parameters,
                    self_type_symbol,
                    argument,
                );
            }
        }
        omega_resolved_trees::types::TypeReference::Named { symbol, name } => {
            *symbol = resolve_type_symbol(symbols, local_type_parameters, name);
        }
        omega_resolved_trees::types::TypeReference::SelfType { symbol } => {
            *symbol = self_type_symbol;
        }
        omega_resolved_trees::types::TypeReference::Unit => {}
    }
}

fn resolve_type_symbol(
    symbols: &SymbolTable,
    local_type_parameters: &[omega_resolved_trees::data::TypeParameter],
    name: &omega_resolved_trees::name::DiagnosticName,
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
    let Some(children) = symbols.child_handles(parent) else {
        return SymbolHandle::invalid();
    };

    for child in children {
        let symbol = symbols.get(child);
        if symbols.name(child) == name && kinds.contains(&symbol.kind) {
            return child;
        }
    }

    SymbolHandle::invalid()
}
