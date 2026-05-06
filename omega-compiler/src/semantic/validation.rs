use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::ir::data::{DataMember, DataShapeKind};
use crate::ir::expression::Expression;
use crate::ir::signature::{StateParameter, StateSignature};
use crate::ir::statement::{Statement, TransitionTarget};
use crate::ir::types::{PrimitiveType, TypeReference};
use crate::semantic::symbols::{MachineSymbols, ProgramSymbols};

pub fn validate_program(program: &Program) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let symbols = ProgramSymbols::build(program, &mut diagnostics);

    validate_callable_state_signatures(program, &symbols, &mut diagnostics);
    validate_data_field_types(program, &symbols, &mut diagnostics);
    validate_entry_point(program, &mut diagnostics);

    for machine in &program.machines {
        let machine_symbols = MachineSymbols::build(machine, &mut diagnostics);

        validate_contained_types(machine, &symbols, &mut diagnostics);
        validate_owned_data(machine, &symbols, &mut diagnostics);

        for state in &machine.states {
            let reserved_names = machine_symbols
                .member_names()
                .chain(
                    state
                        .parameters
                        .iter()
                        .map(|parameter| parameter.name.as_str()),
                )
                .collect::<Vec<_>>();
            validate_local_data_names(
                &state.statements,
                format!("machine `{}` state `{}`", machine.name, state.name),
                &reserved_names,
                &mut diagnostics,
            );

            for statement in &state.statements {
                validate_state_statement(
                    machine,
                    &state.name,
                    &machine_symbols,
                    &symbols,
                    statement,
                    &mut diagnostics,
                );
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn validate_local_data_names(
    statements: &[Statement],
    owner: String,
    reserved_names: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut local_names = Vec::new();

    for statement in statements {
        let Statement::LocalData(local_data) = statement else {
            continue;
        };

        if reserved_names.contains(&local_data.name.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} local data `{}` conflicts with an existing name",
                local_data.name
            )));
            continue;
        }

        if local_names.contains(&local_data.name.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} has duplicate local data `{}`",
                local_data.name
            )));
        }

        local_names.push(local_data.name.as_str());
    }
}

fn validate_state_statement(
    machine: &crate::ir::machine::Machine,
    state_name: &str,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &ProgramSymbols<'_>,
    statement: &Statement,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        Statement::Assignment(_) => {}
        Statement::Call(call) => {
            validate_call(call, machine, machine_symbols, symbols, diagnostics)
        }
        Statement::LocalData(local_data) => validate_type_reference(
            &local_data.type_reference,
            symbols,
            diagnostics,
            format!(
                "machine `{}` state `{state_name}` local data `{}`",
                machine.name, local_data.name
            ),
        ),
        Statement::Transition(transition) => {
            validate_transition_target(&transition.target, machine_symbols, diagnostics);

            if let Some(continuation) = &transition.continuation {
                validate_transition_target(continuation, machine_symbols, diagnostics);
            }
        }
    }
}

fn validate_callable_state_signatures(
    program: &Program,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in &program.machines {
        validate_state_signature_types(
            machine.states.iter().map(|state| StateSignature {
                name: state.name.clone(),
                parameters: state.parameters.clone(),
            }),
            symbols,
            diagnostics,
            format!("machine `{}`", machine.name),
        );
    }

    for platform in &program.platforms {
        validate_platform_state_names(platform, diagnostics);
        validate_state_signature_types(
            platform.states.iter().cloned(),
            symbols,
            diagnostics,
            format!("platform `{}`", platform.name),
        );
    }
}

fn validate_state_signature_types(
    signatures: impl Iterator<Item = StateSignature>,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: String,
) {
    for signature in signatures {
        validate_state_parameter_names(&signature, &owner, diagnostics);

        for parameter in &signature.parameters {
            validate_type_reference(
                &parameter.type_reference,
                symbols,
                diagnostics,
                format!(
                    "{owner} state `{}` parameter `{}`",
                    signature.name, parameter.name
                ),
            );
        }
    }
}

fn validate_platform_state_names(
    platform: &crate::ir::platform::Platform,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut state_names = Vec::new();

    for state in &platform.states {
        if state_names.contains(&state.name.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "platform `{}` has duplicate state `{}`",
                platform.name, state.name
            )));
        }

        state_names.push(state.name.as_str());
    }
}

fn validate_state_parameter_names(
    state: &StateSignature,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut parameter_names = Vec::new();

    for parameter in &state.parameters {
        if parameter_names.contains(&parameter.name.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} state `{}` has duplicate parameter `{}`",
                state.name, parameter.name
            )));
        }

        parameter_names.push(parameter.name.as_str());
    }
}

fn validate_data_field_types(
    program: &Program,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for data_definition in &program.data_definitions {
        validate_data_member_names(data_definition, diagnostics);
        validate_data_shape(data_definition, diagnostics);

        for member in &data_definition.members {
            let DataMember::Field(field) = member else {
                continue;
            };

            validate_type_reference(
                &field.type_reference,
                symbols,
                diagnostics,
                format!("data `{}` field `{}`", data_definition.name, field.name),
            );
        }
    }
}

fn validate_data_shape(
    data_definition: &crate::ir::data::DataDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match data_definition.shape_kind() {
        DataShapeKind::Empty => diagnostics.push(Diagnostic::error(format!(
            "data `{}` must declare at least one field or variant",
            data_definition.name
        ))),
        DataShapeKind::Mixed => diagnostics.push(Diagnostic::error(format!(
            "data `{}` mixes fields and variants; split record data from enum-like data",
            data_definition.name
        ))),
        DataShapeKind::Enum | DataShapeKind::Record => {}
    }
}

fn validate_data_member_names(
    data_definition: &crate::ir::data::DataDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut member_names = Vec::new();

    for member in &data_definition.members {
        let member_name = match member {
            DataMember::Field(field) => field.name.as_str(),
            DataMember::Variant(variant) => variant.name.as_str(),
        };

        if member_names.contains(&member_name) {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` has duplicate member `{member_name}`",
                data_definition.name
            )));
        }

        member_names.push(member_name);
    }
}

fn validate_type_reference(
    type_reference: &TypeReference,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: String,
) {
    match type_reference {
        TypeReference::FixedArray { element_type, .. } => {
            validate_type_reference(element_type, symbols, diagnostics, owner);
        }
        TypeReference::Named(name) => {
            if PrimitiveType::from_name(name).is_some() {
                return;
            }

            if !symbols.has_data_definition(name) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown data type `{name}`"
                )));
            }
        }
    }
}

fn validate_entry_point(program: &Program, diagnostics: &mut Vec<Diagnostic>) {
    let Some(main_machine) = program
        .machines
        .iter()
        .find(|machine| machine.name == "main")
    else {
        diagnostics.push(Diagnostic::error("missing machine main"));
        return;
    };

    if !main_machine
        .states
        .iter()
        .any(|state| state.name == "entry")
    {
        diagnostics.push(Diagnostic::error("machine main is missing state entry"));
    }
}

fn validate_contained_types(
    machine: &crate::ir::machine::Machine,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for contained_object in &machine.contains {
        if !symbols.is_callable_receiver_type(&contained_object.type_name) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` contains `{}` with unknown type `{}`",
                machine.name, contained_object.name, contained_object.type_name
            )));
        }
    }
}

fn validate_owned_data(
    machine: &crate::ir::machine::Machine,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for owned_data in &machine.owned_data {
        validate_type_reference(
            &owned_data.type_reference,
            symbols,
            diagnostics,
            format!(
                "machine `{}` owned data `{}`",
                machine.name, owned_data.name
            ),
        );

        if let Some(initial_value) = &owned_data.initial_value {
            validate_initial_value(
                &owned_data.type_reference,
                initial_value,
                diagnostics,
                format!(
                    "machine `{}` owned data `{}`",
                    machine.name, owned_data.name
                ),
            );
        }
    }
}

fn validate_initial_value(
    type_reference: &TypeReference,
    initial_value: &Expression,
    diagnostics: &mut Vec<Diagnostic>,
    owner: String,
) {
    if !argument_matches_type(initial_value, type_reference) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} initializer expects `{}`, got `{}`",
            type_reference.display_name(),
            expression_type_name(initial_value)
        )));
    }
}

fn validate_call(
    call: &crate::ir::statement::Call,
    current_machine: &crate::ir::machine::Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(receiver) = call.receiver.as_deref() else {
        let Some(state) = machine_symbols.state(&call.target) else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has no local state `{}`",
                current_machine.name, call.target
            )));
            return;
        };

        validate_call_arguments(call, state.name.as_str(), &state.parameters, diagnostics);
        return;
    };

    let receiver_type = machine_symbols.contained_type(receiver);

    if let Some(platform) = receiver_type.and_then(|type_name| symbols.platform(type_name)) {
        let Some(state_signature) = platform
            .states
            .iter()
            .find(|state| state.name == call.target)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "platform `{}` has no state `{}`",
                platform.name, call.target
            )));
            return;
        };

        validate_call_arguments(
            call,
            &state_signature.name,
            &state_signature.parameters,
            diagnostics,
        );
        return;
    }

    if let Some(machine) = receiver_type
        .and_then(|type_name| symbols.machine(type_name))
        .or_else(|| symbols.machine(receiver))
    {
        if let Some(state) = machine
            .states
            .iter()
            .find(|state| state.name == call.target)
        {
            validate_call_arguments(call, &state.name, &state.parameters, diagnostics);
            return;
        };

        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` has no state `{}`",
            machine.name, call.target
        )));
        return;
    }

    diagnostics.push(Diagnostic::error(format!(
        "unknown call receiver `{receiver}`"
    )));
}

fn validate_call_arguments(
    call: &crate::ir::statement::Call,
    target_name: &str,
    parameters: &[StateParameter],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if call.arguments.len() != parameters.len() {
        diagnostics.push(Diagnostic::error(format!(
            "state `{}` expects {} argument(s), got {}",
            target_name,
            parameters.len(),
            call.arguments.len()
        )));
        return;
    }

    for (argument, parameter) in call.arguments.iter().zip(parameters.iter()) {
        if parameter.is_mutable && !matches!(argument, Expression::Mutable(_)) {
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for state `{}` must be passed with `mut`",
                parameter.name, target_name
            )));
            continue;
        }

        if !parameter.is_mutable && matches!(argument, Expression::Mutable(_)) {
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for state `{}` is not mutable",
                parameter.name, target_name
            )));
            continue;
        }

        let expected_type = parameter.type_reference.display_name();

        if !argument_matches_type(argument, &parameter.type_reference) {
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for state `{}` expects `{}`, got `{}`",
                parameter.name,
                target_name,
                expected_type,
                expression_type_name(argument)
            )));
        }
    }
}

fn argument_matches_type(argument: &Expression, type_reference: &TypeReference) -> bool {
    if let Expression::Mutable(inner_expression) = argument {
        return argument_matches_type(inner_expression, type_reference);
    }

    match type_reference {
        TypeReference::FixedArray { .. } => matches!(
            argument,
            Expression::ArrayLiteral(_) | Expression::Indexed(_) | Expression::Name(_)
        ),
        TypeReference::Named(type_name) => {
            if let Some(primitive_type) = PrimitiveType::from_name(type_name) {
                return matches!(argument, Expression::String(_))
                    && primitive_type == PrimitiveType::String
                    || matches!(argument, Expression::Integer(_))
                        && primitive_type.accepts_integer_literal()
                    || matches!(
                        argument,
                        Expression::Binary(_)
                            | Expression::Indexed(_)
                            | Expression::Name(_)
                            | Expression::StructLiteral(_)
                    );
            }

            matches!(
                argument,
                Expression::Binary(_)
                    | Expression::Indexed(_)
                    | Expression::Name(_)
                    | Expression::StructLiteral(_)
            )
        }
    }
}

fn expression_type_name(argument: &Expression) -> &'static str {
    match argument {
        Expression::ArrayLiteral(_) => "array literal",
        Expression::Binary(_) => "binary expression",
        Expression::Indexed(_) => "indexed value",
        Expression::Integer(_) => "integer literal",
        Expression::Mutable(inner_expression) => expression_type_name(inner_expression),
        Expression::Name(_) => "named value",
        Expression::StructLiteral(_) => "struct literal",
        Expression::String(_) => "String",
    }
}

fn validate_transition_target(
    target: &TransitionTarget,
    machine_symbols: &MachineSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let TransitionTarget::Named(path) = target else {
        return;
    };

    if path.len() == 1 {
        if !machine_symbols.has_state(path[0].as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "unknown state transition target `{}`",
                path[0]
            )));
        }

        return;
    }

    if machine_symbols.contained_type(path[0].as_str()).is_none() {
        diagnostics.push(Diagnostic::error(format!(
            "unknown nested transition receiver `{}`",
            path[0]
        )));
    }
}
