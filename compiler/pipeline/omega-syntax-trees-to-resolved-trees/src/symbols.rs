use omega_core::symbols::{
    SymbolDefinition, SymbolHandle, SymbolKind, SymbolTable, builtin_type_symbol_definitions,
};
use omega_resolved_trees::Program;

pub(crate) fn assign_symbols(program: &mut Program) {
    program.symbols = build_symbol_table(program);
    let symbols = program.symbols.clone();
    assign_top_level_symbols(program, &symbols);
    assign_type_reference_symbols(program, &symbols);
    assign_statement_call_symbols(program, &symbols);
}

fn build_symbol_table(program: &Program) -> SymbolTable {
    let mut children = builtin_type_symbol_definitions().to_vec();

    children.extend(
        program
            .invariant_definitions
            .iter()
            .map(|invariant| SymbolDefinition::named(SymbolKind::Invariant, invariant.name.as_str())),
    );
    children.extend(program.data_definitions.iter().map(data_symbol_definition));
    children.extend(program.machines.iter().map(|machine| machine_symbol_definition(program, machine)));
    children.extend(program.platforms.iter().map(platform_symbol_definition));

    SymbolTable::from_definition(SymbolDefinition::with_children(SymbolKind::Root, "root", children))
}

fn data_symbol_definition<'program>(
    data_definition: &'program omega_resolved_trees::data::DataDefinition,
) -> SymbolDefinition<'program> {
    SymbolDefinition::with_children(
        SymbolKind::Data,
        data_definition.name.as_str(),
        data_definition
            .type_parameters
            .iter()
            .map(|parameter| {
                SymbolDefinition::named(SymbolKind::TypeParameter, parameter.name.as_str())
            })
            .chain(data_definition.members.iter().map(|member| match member {
                omega_resolved_trees::data::DataMember::Field(field) => {
                    SymbolDefinition::named(SymbolKind::Field, field.name.as_str())
                }
                omega_resolved_trees::data::DataMember::Variant(variant) => {
                    SymbolDefinition::named(SymbolKind::Variant, variant.name.as_str())
                }
            })),
    )
}

fn machine_symbol_definition<'program>(
    program: &'program Program,
    machine: &'program omega_resolved_trees::machine::Machine,
) -> SymbolDefinition<'program> {
    let inherited_data_members = program
        .data_definitions
        .iter()
        .find(|data_definition| data_definition.name == machine.name)
        .into_iter()
        .flat_map(|data_definition| data_definition.members.iter())
        .filter_map(|member| match member {
            omega_resolved_trees::data::DataMember::Field(field) => {
                Some(SymbolDefinition::named(SymbolKind::Field, field.name.as_str()))
            }
            omega_resolved_trees::data::DataMember::Variant(_) => None,
        });
    let contained_objects = machine
        .contains
        .iter()
        .map(|contained_object| SymbolDefinition::named(SymbolKind::Object, contained_object.name.as_str()));
    let owned_data = machine
        .owned_data
        .iter()
        .map(|owned_data| SymbolDefinition::named(SymbolKind::Field, owned_data.name.as_str()));
    let states = machine.states.iter().map(state_symbol_definition);

    SymbolDefinition::with_children(
        SymbolKind::Machine,
        machine.name.as_str(),
        inherited_data_members
            .chain(contained_objects)
            .chain(owned_data)
            .chain(states),
    )
}

fn state_symbol_definition<'program>(
    state: &'program omega_resolved_trees::state::State,
) -> SymbolDefinition<'program> {
    SymbolDefinition::with_children(
        SymbolKind::State,
        state.name.as_str(),
        state
            .parameters
            .iter()
            .map(|parameter| SymbolDefinition::named(SymbolKind::Parameter, parameter.name.as_str()))
            .chain(local_symbol_definitions(&state.statements)),
    )
}

fn platform_symbol_definition<'program>(
    platform: &'program omega_resolved_trees::platform::Platform,
) -> SymbolDefinition<'program> {
    SymbolDefinition::with_children(
        SymbolKind::Platform,
        platform.name.as_str(),
        platform.states.iter().map(|state| {
            SymbolDefinition::with_children(
                SymbolKind::State,
                state.name.as_str(),
                state
                    .parameters
                    .iter()
                    .map(|parameter| {
                        SymbolDefinition::named(SymbolKind::Parameter, parameter.name.as_str())
                    }),
            )
        }),
    )
}

fn local_symbol_definitions<'program>(
    statements: &'program [omega_resolved_trees::statement::Statement],
) -> impl Iterator<Item = SymbolDefinition<'program>> + 'program {
    statements.iter().filter_map(|statement| match statement {
        omega_resolved_trees::statement::Statement::LocalData(local_data) => Some(
            SymbolDefinition::named(SymbolKind::Local, local_data.name.as_str()),
        ),
        _ => None,
    })
}

fn assign_top_level_symbols(program: &mut Program, symbols: &SymbolTable) {
    for invariant in &mut program.invariant_definitions {
        invariant.symbol = top_level_symbol(symbols, SymbolKind::Invariant, invariant.name.as_str());
    }

    for data_definition in &mut program.data_definitions {
        data_definition.symbol = top_level_symbol(symbols, SymbolKind::Data, data_definition.name.as_str());

        for type_parameter in &mut data_definition.type_parameters {
            type_parameter.symbol =
                child_symbol_by_kinds(symbols, data_definition.symbol, &[SymbolKind::TypeParameter], type_parameter.name.as_str());
        }

        for member in &mut data_definition.members {
            match member {
                omega_resolved_trees::data::DataMember::Field(field) => {
                    field.symbol = child_symbol(symbols, data_definition.symbol, field.name.as_str());
                }
                omega_resolved_trees::data::DataMember::Variant(variant) => {
                    variant.symbol =
                        child_symbol(symbols, data_definition.symbol, variant.name.as_str());
                }
            }
        }
    }

    for machine in &mut program.machines {
        machine.symbol = top_level_symbol(symbols, SymbolKind::Machine, machine.name.as_str());

        for contained_object in &mut machine.contains {
            contained_object.symbol = child_symbol(symbols, machine.symbol, contained_object.name.as_str());
            contained_object.type_symbol =
                top_level_symbol(symbols, SymbolKind::Machine, contained_object.type_name.as_str());
        }

        for owned_data in &mut machine.owned_data {
            owned_data.symbol = child_symbol(symbols, machine.symbol, owned_data.name.as_str());
            assign_type_reference_symbol(symbols, &mut owned_data.type_reference);
        }

        for state in &mut machine.states {
            state.symbol = child_symbol(symbols, machine.symbol, state.name.as_str());

            for parameter in &mut state.parameters {
                parameter.symbol = child_symbol(symbols, state.symbol, parameter.name.as_str());
                assign_type_reference_symbol(symbols, &mut parameter.type_reference);
            }

            for statement in &mut state.statements {
                if let omega_resolved_trees::statement::Statement::LocalData(local_data) = statement {
                    local_data.symbol = child_symbol(symbols, state.symbol, local_data.name.as_str());
                }
            }

            if let Some(return_type) = &mut state.return_type {
                assign_type_reference_symbol(symbols, return_type);
            }
        }
    }

    for platform in &mut program.platforms {
        platform.symbol = top_level_symbol(symbols, SymbolKind::Platform, platform.name.as_str());

        for state in &mut platform.states {
            state.symbol = child_symbol(symbols, platform.symbol, state.name.as_str());

            for parameter in &mut state.parameters {
                parameter.symbol = child_symbol(symbols, state.symbol, parameter.name.as_str());
                assign_type_reference_symbol(symbols, &mut parameter.type_reference);
            }

            if let Some(return_type) = &mut state.return_type {
                assign_type_reference_symbol(symbols, return_type);
            }
        }
    }
}

fn assign_type_reference_symbols(
    program: &mut Program,
    symbols: &SymbolTable,
) {
    for data_definition in &mut program.data_definitions {
        let type_parameter_bindings = data_type_parameter_bindings(data_definition);
        for member in &mut data_definition.members {
            if let omega_resolved_trees::data::DataMember::Field(field) = member {
                assign_type_reference_symbol_with_locals(
                    symbols,
                    &type_parameter_bindings,
                    &mut field.type_reference,
                );
            }
        }
    }
}

fn assign_statement_call_symbols(program: &mut Program, symbols: &SymbolTable) {
    let data_definitions = program.data_definitions.clone();
    for machine in &mut program.machines {
        let machine_scope = MachineScope {
            symbol: machine.symbol,
            contains: machine.contains.clone(),
            fields: machine_field_bindings(&data_definitions, machine),
        };
        for state in &mut machine.states {
            let parameter_bindings = state_parameter_bindings(state);
            for statement in &mut state.statements {
                assign_statement_symbols(
                    &machine_scope,
                    &parameter_bindings,
                    state.symbol,
                    statement,
                    symbols,
                );
            }
        }
    }

}

#[derive(Clone)]
struct MachineScope {
    symbol: SymbolHandle,
    contains: Vec<omega_resolved_trees::machine::ContainedObject>,
    fields: Vec<FieldBinding>,
}

#[derive(Clone)]
struct FieldBinding {
    symbol: SymbolHandle,
    name: omega_resolved_trees::name::ProgramName,
    type_name: omega_resolved_trees::name::ProgramName,
    type_symbol: SymbolHandle,
}

#[derive(Clone)]
struct ParameterBinding {
    symbol: SymbolHandle,
    name: omega_resolved_trees::name::ProgramName,
    type_name: omega_resolved_trees::name::ProgramName,
    type_symbol: SymbolHandle,
}

#[derive(Clone)]
struct TypeParameterBinding {
    symbol: SymbolHandle,
    name: omega_resolved_trees::name::ProgramName,
}

fn assign_statement_symbols(
    machine: &MachineScope,
    parameters: &[ParameterBinding],
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

            call.target_symbol = resolve_call_target_symbol(machine, parameters, call, symbols);
        }
        omega_resolved_trees::statement::Statement::Expression(expression) => {
            assign_expression_symbols(symbols, machine, parameters, state_symbol, expression);
        }
        omega_resolved_trees::statement::Statement::LocalData(local_data) => {
            assign_type_reference_symbol(symbols, &mut local_data.type_reference);
            if let Some(initial_value) = &mut local_data.initial_value {
                assign_expression_symbols(symbols, machine, parameters, state_symbol, initial_value);
            }
        }
        omega_resolved_trees::statement::Statement::Transition(transition) => {
            if let omega_resolved_trees::statement::TransitionGuard::When(expression) =
                &mut transition.guard
            {
                assign_expression_symbols(symbols, machine, parameters, state_symbol, expression);
            }
            assign_transition_target_symbols(machine, state_symbol, &mut transition.target, symbols);
            if let Some(continuation) = &mut transition.continuation {
                assign_transition_target_symbols(machine, state_symbol, continuation, symbols);
            }
        }
    }
}

fn assign_expression_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope,
    parameters: &[ParameterBinding],
    state_symbol: SymbolHandle,
    expression: &mut omega_resolved_trees::expression::Expression,
) {
    match expression {
        omega_resolved_trees::expression::Expression::ArrayLiteral(values) => {
            for value in values {
                assign_expression_symbols(symbols, machine, parameters, state_symbol, value);
            }
        }
        omega_resolved_trees::expression::Expression::Binary(binary) => {
            assign_expression_symbols(symbols, machine, parameters, state_symbol, &mut binary.left);
            assign_expression_symbols(symbols, machine, parameters, state_symbol, &mut binary.right);
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
                        *receiver = Box::new(
                            expression_from_path(receiver_path.with_symbols(head_symbol, symbol))
                        );
                    }
                }
            }
            call.target_symbol = resolve_expression_call_target_symbol(machine, parameters, call, symbols);
        }
        omega_resolved_trees::expression::Expression::Indexed(indexed) => {
            assign_expression_symbols(symbols, machine, parameters, state_symbol, &mut indexed.collection);
            assign_expression_symbols(symbols, machine, parameters, state_symbol, &mut indexed.index);
        }
        omega_resolved_trees::expression::Expression::Member(member) => {
            assign_expression_symbols(symbols, machine, parameters, state_symbol, &mut member.receiver);
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
                assign_expression_symbols(symbols, machine, parameters, state_symbol, &mut field.value);
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
                receiver: expression,
                member_symbol: SymbolHandle::invalid(),
                member,
            },
        ));
    }
    expression
}

fn resolve_expression_call_target_symbol(
    machine: &MachineScope,
    parameters: &[ParameterBinding],
    call: &omega_resolved_trees::expression::CallExpression,
    symbols: &SymbolTable,
) -> SymbolHandle {
    if let Some(receiver) = &call.receiver
        && let Some(receiver_path) = expression_name_path(receiver)
    {
        let statement_call = omega_resolved_trees::statement::Call {
            receiver_symbol: receiver_path.symbol(),
            target_symbol: call.target_symbol,
            receiver: Some(receiver_path),
            target: call.target.clone(),
            arguments: Vec::new(),
        };
        return resolve_call_target_symbol(machine, parameters, &statement_call, symbols);
    }

    let statement_call = omega_resolved_trees::statement::Call {
        receiver_symbol: SymbolHandle::invalid(),
        target_symbol: call.target_symbol,
        receiver: None,
        target: call.target.clone(),
        arguments: Vec::new(),
    };
    resolve_call_target_symbol(machine, parameters, &statement_call, symbols)
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
            let omega_resolved_trees::expression::Expression::Integer(index) = &indexed.index else {
                return None;
            };
            let mut path = expression_name_path(&indexed.collection)?;
            let last_segment = path.last_mut()?;
            *last_segment =
                omega_resolved_trees::name::ProgramName::generated(format!("{last_segment}[{index}]"));
            Some(path)
        }
        omega_resolved_trees::expression::Expression::Mutable(inner) => expression_name_path(inner),
        _ => None,
    }
}

fn assign_transition_target_symbols(
    machine: &MachineScope,
    state_symbol: SymbolHandle,
    target: &mut omega_resolved_trees::statement::TransitionTarget,
    symbols: &SymbolTable,
) {
    let omega_resolved_trees::statement::TransitionTarget::Named { path, .. } = target else {
        return;
    };

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
        let target_symbol =
            child_symbol_by_kinds(symbols, machine.symbol, &[SymbolKind::State], target_name.as_str());
        if target_symbol.is_valid() {
            *path = path.clone().with_symbols(target_symbol, target_symbol);
        }
    }
}

fn resolve_call_target_symbol(
    machine: &MachineScope,
    parameters: &[ParameterBinding],
    call: &omega_resolved_trees::statement::Call,
    symbols: &SymbolTable,
) -> SymbolHandle {
    if let Some(receiver) = &call.receiver {
        if call.receiver_symbol.is_valid() {
            if let Some(contained) = machine
                .contains
                .iter()
                .find(|contained| contained.symbol == call.receiver_symbol)
            {
                return child_symbol_by_kinds(
                    symbols,
                    contained.type_symbol,
                    &[SymbolKind::State],
                    call.target.as_str(),
                );
            }

            if let Some(field) = machine
                .fields
                .iter()
                .find(|field| field.symbol == call.receiver_symbol)
            {
                let symbol = child_symbol_by_kinds(
                    symbols,
                    field.type_symbol,
                    &[SymbolKind::State],
                    call.target.as_str(),
                );
                if symbol.is_valid() {
                    return symbol;
                }
                let callable_type = top_level_symbol_by_kinds(
                    symbols,
                    &[SymbolKind::Machine, SymbolKind::Platform],
                    field.type_name.as_str(),
                );
                if callable_type.is_valid() {
                    return child_symbol_by_kinds(
                        symbols,
                        callable_type,
                        &[SymbolKind::State],
                        call.target.as_str(),
                    );
                }
                return symbol;
            }

            let receiver_kind = symbols.get(call.receiver_symbol).kind;
            if let Some(parameter) = parameters
                .iter()
                .find(|parameter| parameter.symbol == call.receiver_symbol)
            {
                let direct = child_symbol_by_kinds(
                    symbols,
                    parameter.type_symbol,
                    &[SymbolKind::State],
                    call.target.as_str(),
                );
                if direct.is_valid() {
                    return direct;
                }
                let callable_type = top_level_symbol_by_kinds(
                    symbols,
                    &[SymbolKind::Machine, SymbolKind::Platform],
                    parameter.type_name.as_str(),
                );
                if callable_type.is_valid() {
                    return child_symbol_by_kinds(
                        symbols,
                        callable_type,
                        &[SymbolKind::State],
                        call.target.as_str(),
                    );
                }
            }
            if matches!(receiver_kind, SymbolKind::Machine | SymbolKind::Platform) {
                return child_symbol_by_kinds(
                    symbols,
                    call.receiver_symbol,
                    &[SymbolKind::State],
                    call.target.as_str(),
                );
            }
        }

        if let Some(receiver_name) = receiver.last() {
            if let Some(contained) = machine
                .contains
                .iter()
                .find(|contained| contained.name == *receiver_name)
            {
                return child_symbol_by_kinds(
                    symbols,
                    contained.type_symbol,
                    &[SymbolKind::State],
                    call.target.as_str(),
                );
            }

            if let Some(field) = machine
                .fields
                .iter()
                .find(|field| field.name == *receiver_name)
            {
                let direct = child_symbol_by_kinds(
                    symbols,
                    field.type_symbol,
                    &[SymbolKind::State],
                    call.target.as_str(),
                );
                if direct.is_valid() {
                    return direct;
                }
                let callable_type = top_level_symbol_by_kinds(
                    symbols,
                    &[SymbolKind::Machine, SymbolKind::Platform],
                    field.type_name.as_str(),
                );
                if callable_type.is_valid() {
                    return child_symbol_by_kinds(
                        symbols,
                        callable_type,
                        &[SymbolKind::State],
                        call.target.as_str(),
                    );
                }
                return direct;
            }

            if let Some(parameter) = parameters
                .iter()
                .find(|parameter| parameter.name == *receiver_name)
            {
                let direct = child_symbol_by_kinds(
                    symbols,
                    parameter.type_symbol,
                    &[SymbolKind::State],
                    call.target.as_str(),
                );
                if direct.is_valid() {
                    return direct;
                }
                let callable_type = top_level_symbol_by_kinds(
                    symbols,
                    &[SymbolKind::Machine, SymbolKind::Platform],
                    parameter.type_name.as_str(),
                );
                if callable_type.is_valid() {
                    return child_symbol_by_kinds(
                        symbols,
                        callable_type,
                        &[SymbolKind::State],
                        call.target.as_str(),
                    );
                }
                return direct;
            }

            let top_level_receiver = top_level_symbol_by_kinds(
                symbols,
                &[SymbolKind::Machine, SymbolKind::Platform],
                receiver_name.as_str(),
            );
            if top_level_receiver.is_valid() {
                return child_symbol_by_kinds(
                    symbols,
                    top_level_receiver,
                    &[SymbolKind::State],
                    call.target.as_str(),
                );
            }
        }
    }

    child_symbol_by_kinds(
        symbols,
        machine.symbol,
        &[SymbolKind::State],
        call.target.as_str(),
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

    if members.first().is_some_and(|member| member.as_str() == "self") {
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
    member: &omega_resolved_trees::name::ProgramName,
) -> Option<SymbolHandle> {
    if state_symbol.is_valid() {
        let parameter_symbol =
            child_symbol_by_kinds(symbols, state_symbol, &[SymbolKind::Parameter], member.as_str());
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

fn machine_field_bindings(
    data_definitions: &[omega_resolved_trees::data::DataDefinition],
    machine: &omega_resolved_trees::machine::Machine,
) -> Vec<FieldBinding> {
    let mut fields = Vec::new();

    if let Some(data_definition) = data_definitions
        .iter()
        .find(|data_definition| data_definition.name == machine.name)
    {
        for member in &data_definition.members {
            let omega_resolved_trees::data::DataMember::Field(field) = member else {
                continue;
            };

            fields.push(FieldBinding {
                symbol: field.symbol,
                name: field.name.clone(),
                type_name: type_reference_name(&field.type_reference),
                type_symbol: type_reference_symbol(&field.type_reference),
            });
        }
    }

    for owned_data in &machine.owned_data {
        fields.push(FieldBinding {
            symbol: owned_data.symbol,
            name: owned_data.name.clone(),
            type_name: type_reference_name(&owned_data.type_reference),
            type_symbol: type_reference_symbol(&owned_data.type_reference),
        });
    }

    fields
}

fn state_parameter_bindings(
    state: &omega_resolved_trees::state::State,
) -> Vec<ParameterBinding> {
    state
        .parameters
        .iter()
        .map(|parameter| ParameterBinding {
            symbol: parameter.symbol,
            name: parameter.name.clone(),
            type_name: type_reference_name(&parameter.type_reference),
            type_symbol: type_reference_symbol(&parameter.type_reference),
        })
        .collect()
}

fn data_type_parameter_bindings(
    data_definition: &omega_resolved_trees::data::DataDefinition,
) -> Vec<TypeParameterBinding> {
    data_definition
        .type_parameters
        .iter()
        .map(|parameter| TypeParameterBinding {
            symbol: parameter.symbol,
            name: parameter.name.clone(),
        })
        .collect()
}

fn type_reference_symbol(
    type_reference: &omega_resolved_trees::types::TypeReference,
) -> SymbolHandle {
    match type_reference {
        omega_resolved_trees::types::TypeReference::Reference { referee, .. } => {
            type_reference_symbol(referee)
        }
        omega_resolved_trees::types::TypeReference::Constrained { base_type, .. } => {
            type_reference_symbol(base_type)
        }
        omega_resolved_trees::types::TypeReference::FixedArray { element_type, .. } => {
            type_reference_symbol(element_type)
        }
        omega_resolved_trees::types::TypeReference::Slice { element_type } => {
            type_reference_symbol(element_type)
        }
        omega_resolved_trees::types::TypeReference::Generic { base_symbol, .. } => *base_symbol,
        omega_resolved_trees::types::TypeReference::Named { symbol, .. } => *symbol,
        omega_resolved_trees::types::TypeReference::Unit => SymbolHandle::invalid(),
    }
}

fn type_reference_name(
    type_reference: &omega_resolved_trees::types::TypeReference,
) -> omega_resolved_trees::name::ProgramName {
    match type_reference {
        omega_resolved_trees::types::TypeReference::Reference { referee, .. } => {
            type_reference_name(referee)
        }
        omega_resolved_trees::types::TypeReference::Constrained { base_type, .. } => {
            type_reference_name(base_type)
        }
        omega_resolved_trees::types::TypeReference::FixedArray { element_type, .. } => {
            type_reference_name(element_type)
        }
        omega_resolved_trees::types::TypeReference::Slice { element_type } => {
            type_reference_name(element_type)
        }
        omega_resolved_trees::types::TypeReference::Generic { base_name, .. } => base_name.clone(),
        omega_resolved_trees::types::TypeReference::Named { name, .. } => name.clone(),
        omega_resolved_trees::types::TypeReference::Unit => {
            omega_resolved_trees::name::ProgramName::default()
        }
    }
}

fn assign_type_reference_symbol(
    symbols: &SymbolTable,
    type_reference: &mut omega_resolved_trees::types::TypeReference,
) {
    assign_type_reference_symbol_with_locals(symbols, &[], type_reference);
}

fn assign_type_reference_symbol_with_locals(
    symbols: &SymbolTable,
    local_type_parameters: &[TypeParameterBinding],
    type_reference: &mut omega_resolved_trees::types::TypeReference,
) {
    match type_reference {
        omega_resolved_trees::types::TypeReference::Reference { referee, .. } => {
            assign_type_reference_symbol_with_locals(symbols, local_type_parameters, referee);
        }
        omega_resolved_trees::types::TypeReference::Constrained { base_type, .. } => {
            assign_type_reference_symbol_with_locals(symbols, local_type_parameters, base_type);
        }
        omega_resolved_trees::types::TypeReference::FixedArray { element_type, .. } => {
            assign_type_reference_symbol_with_locals(symbols, local_type_parameters, element_type);
        }
        omega_resolved_trees::types::TypeReference::Slice { element_type } => {
            assign_type_reference_symbol_with_locals(symbols, local_type_parameters, element_type);
        }
        omega_resolved_trees::types::TypeReference::Generic {
            base_symbol,
            base_name,
            arguments,
        } => {
            *base_symbol = top_level_type_symbol(symbols, base_name.as_str());

            for argument in arguments {
                assign_type_reference_symbol_with_locals(symbols, local_type_parameters, argument);
            }
        }
        omega_resolved_trees::types::TypeReference::Named { symbol, name } => {
            *symbol = local_type_parameters
                .iter()
                .find(|parameter| parameter.name.as_str() == name.as_str())
                .map(|parameter| parameter.symbol)
                .unwrap_or_else(|| top_level_type_symbol(symbols, name.as_str()));
        }
        omega_resolved_trees::types::TypeReference::Unit => {}
    }
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

fn child_symbol(symbols: &SymbolTable, parent: SymbolHandle, name: &str) -> SymbolHandle {
    child_symbol_by_kinds(
        symbols,
        parent,
        &[
            SymbolKind::Field,
            SymbolKind::Variant,
            SymbolKind::State,
            SymbolKind::Parameter,
            SymbolKind::TypeParameter,
            SymbolKind::Local,
            SymbolKind::Object,
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
