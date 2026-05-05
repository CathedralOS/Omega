use std::collections::{HashMap, HashSet};

use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::ir::command::CommandSignature;
use crate::ir::data::DataMember;
use crate::ir::expression::Expression;
use crate::ir::statement::{Statement, TransitionTarget};
use crate::ir::types::TypeReference;

pub fn validate_program(program: &Program) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    validate_top_level_names(program, &mut diagnostics);
    validate_data_field_types(program, &mut diagnostics);
    validate_entry_point(program, &mut diagnostics);

    let platforms = program
        .platforms
        .iter()
        .map(|platform| (platform.name.as_str(), platform))
        .collect::<HashMap<_, _>>();
    let machines = program
        .machines
        .iter()
        .map(|machine| (machine.name.as_str(), machine))
        .collect::<HashMap<_, _>>();

    for machine in &program.machines {
        let contained_types = machine
            .contains
            .iter()
            .map(|contained_object| {
                (
                    contained_object.name.as_str(),
                    contained_object.type_name.as_str(),
                )
            })
            .collect::<HashMap<_, _>>();
        let state_names = machine
            .states
            .iter()
            .map(|state| state.name.as_str())
            .collect::<HashSet<_>>();

        validate_contained_types(machine, program, &mut diagnostics);

        for state in &machine.states {
            for statement in &state.statements {
                match statement {
                    Statement::Assignment(_) => {}
                    Statement::CommandCall(command_call) => validate_command_call(
                        command_call,
                        machine,
                        &contained_types,
                        &machines,
                        &platforms,
                        &mut diagnostics,
                    ),
                    Statement::Transition(transition) => {
                        validate_transition_target(
                            &transition.target,
                            &contained_types,
                            &state_names,
                            &mut diagnostics,
                        );

                        if let Some(continuation) = &transition.continuation {
                            validate_transition_target(
                                continuation,
                                &contained_types,
                                &state_names,
                                &mut diagnostics,
                            );
                        }
                    }
                }
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn validate_top_level_names(program: &Program, diagnostics: &mut Vec<Diagnostic>) {
    let mut data_names = HashSet::new();
    let mut machine_names = HashSet::new();
    let mut platform_names = HashSet::new();

    for data_definition in &program.data_definitions {
        if !data_names.insert(data_definition.name.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate data `{}`",
                data_definition.name
            )));
        }
    }

    for machine in &program.machines {
        if !machine_names.insert(machine.name.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate machine `{}`",
                machine.name
            )));
        }

        if data_names.contains(machine.name.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "`{}` is declared as both data and a machine",
                machine.name
            )));
        }
    }

    for platform in &program.platforms {
        if !platform_names.insert(platform.name.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate platform `{}`",
                platform.name
            )));
        }

        if data_names.contains(platform.name.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "`{}` is declared as both data and a platform",
                platform.name
            )));
        }

        if machine_names.contains(platform.name.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "`{}` is declared as both a machine and a platform",
                platform.name
            )));
        }
    }

    for machine in &program.machines {
        validate_command_signature_types(
            machine.commands.iter(),
            program,
            diagnostics,
            format!("machine `{}`", machine.name),
        );
    }
}

fn validate_command_signature_types<'a>(
    signatures: impl Iterator<Item = &'a CommandSignature>,
    program: &Program,
    diagnostics: &mut Vec<Diagnostic>,
    owner: String,
) {
    for command in signatures {
        for parameter in &command.parameters {
            validate_type_reference(
                &parameter.type_reference,
                program,
                diagnostics,
                format!(
                    "{owner} command `{}` parameter `{}`",
                    command.name, parameter.name
                ),
            );
        }
    }
}

fn validate_data_field_types(program: &Program, diagnostics: &mut Vec<Diagnostic>) {
    for data_definition in &program.data_definitions {
        for member in &data_definition.members {
            let DataMember::Field(field) = member else {
                continue;
            };

            validate_type_reference(
                &field.type_reference,
                program,
                diagnostics,
                format!("data `{}` field `{}`", data_definition.name, field.name),
            );
        }
    }

    for platform in &program.platforms {
        for command in &platform.commands {
            for parameter in &command.parameters {
                validate_type_reference(
                    &parameter.type_reference,
                    program,
                    diagnostics,
                    format!(
                        "platform `{}` command `{}` parameter `{}`",
                        platform.name, command.name, parameter.name
                    ),
                );
            }
        }
    }
}

fn validate_type_reference(
    type_reference: &TypeReference,
    program: &Program,
    diagnostics: &mut Vec<Diagnostic>,
    owner: String,
) {
    match type_reference {
        TypeReference::FixedArray { element_type, .. } => {
            validate_type_reference(element_type, program, diagnostics, owner);
        }
        TypeReference::Named(name) => {
            if is_primitive_type(name) {
                return;
            }

            let exists = program
                .data_definitions
                .iter()
                .any(|data_definition| data_definition.name == *name)
                || program.machines.iter().any(|machine| machine.name == *name)
                || program
                    .platforms
                    .iter()
                    .any(|platform| platform.name == *name);

            if !exists {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown type `{name}`"
                )));
            }
        }
    }
}

fn is_primitive_type(name: &str) -> bool {
    matches!(
        name,
        "String" | "bool" | "i32" | "u32" | "u64" | "usize" | "f32" | "f64"
    )
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

    if !main_machine.states.iter().any(|state| state.name == "Main") {
        diagnostics.push(Diagnostic::error("machine main is missing state Main"));
    }
}

fn validate_contained_types(
    machine: &crate::ir::machine::Machine,
    program: &Program,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for contained_object in &machine.contains {
        let type_exists = program
            .machines
            .iter()
            .any(|machine| machine.name == contained_object.type_name)
            || program
                .platforms
                .iter()
                .any(|platform| platform.name == contained_object.type_name);

        if !type_exists {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` contains `{}` with unknown type `{}`",
                machine.name, contained_object.name, contained_object.type_name
            )));
        }
    }
}

fn validate_command_call(
    command_call: &crate::ir::statement::CommandCall,
    current_machine: &crate::ir::machine::Machine,
    contained_types: &HashMap<&str, &str>,
    machines: &HashMap<&str, &crate::ir::machine::Machine>,
    platforms: &HashMap<&str, &crate::ir::platform::Platform>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(receiver) = command_call.receiver.as_deref() else {
        let Some(command_signature) = current_machine
            .commands
            .iter()
            .find(|command| command.name == command_call.command)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has no local command `{}`",
                current_machine.name, command_call.command
            )));
            return;
        };

        validate_command_arguments(command_call, command_signature, diagnostics);
        return;
    };

    let Some(receiver_type) = contained_types.get(receiver) else {
        diagnostics.push(Diagnostic::error(format!(
            "unknown command receiver `{}`",
            receiver
        )));
        return;
    };

    if let Some(platform) = platforms.get(receiver_type) {
        let Some(command_signature) = platform
            .commands
            .iter()
            .find(|command| command.name == command_call.command)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "platform `{}` has no command `{}`",
                platform.name, command_call.command
            )));
            return;
        };

        validate_command_arguments(command_call, command_signature, diagnostics);
        return;
    }

    if let Some(machine) = machines.get(receiver_type) {
        let Some(command_signature) = machine
            .commands
            .iter()
            .find(|command| command.name == command_call.command)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has no command `{}`",
                machine.name, command_call.command
            )));
            return;
        };

        validate_command_arguments(command_call, command_signature, diagnostics);
        return;
    }

    diagnostics.push(Diagnostic::error(format!(
        "`{receiver_type}` is not a known command receiver type"
    )));
}

fn validate_command_arguments(
    command_call: &crate::ir::statement::CommandCall,
    command_signature: &CommandSignature,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if command_call.arguments.len() != command_signature.parameters.len() {
        diagnostics.push(Diagnostic::error(format!(
            "command `{}` expects {} argument(s), got {}",
            command_call.command,
            command_signature.parameters.len(),
            command_call.arguments.len()
        )));
        return;
    }

    for (argument, parameter) in command_call
        .arguments
        .iter()
        .zip(command_signature.parameters.iter())
    {
        if parameter.is_mutable && !matches!(argument, Expression::Mutable(_)) {
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for command `{}` must be passed with `mut`",
                parameter.name, command_call.command
            )));
            continue;
        }

        if !parameter.is_mutable && matches!(argument, Expression::Mutable(_)) {
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for command `{}` is not mutable",
                parameter.name, command_call.command
            )));
            continue;
        }

        let expected_type = parameter.type_reference.display_name();

        if !argument_matches_type(argument, &parameter.type_reference) {
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for command `{}` expects `{}`, got `{}`",
                parameter.name,
                command_call.command,
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
        TypeReference::FixedArray { .. } => matches!(argument, Expression::Name(_)),
        TypeReference::Named(type_name) => {
            matches!(
                (argument, type_name.as_str()),
                (Expression::String(_), "String")
                    | (Expression::Integer(_), "i32")
                    | (Expression::Integer(_), "u32")
                    | (Expression::Integer(_), "u64")
                    | (Expression::Integer(_), "usize")
            ) || matches!(argument, Expression::Name(_))
        }
    }
}

fn expression_type_name(argument: &Expression) -> &'static str {
    match argument {
        Expression::Integer(_) => "integer literal",
        Expression::Mutable(inner_expression) => expression_type_name(inner_expression),
        Expression::Name(_) => "named value",
        Expression::String(_) => "String",
    }
}

fn validate_transition_target(
    target: &TransitionTarget,
    contained_types: &HashMap<&str, &str>,
    state_names: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let TransitionTarget::Named(path) = target else {
        return;
    };

    if path.len() == 1 {
        if !state_names.contains(path[0].as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "unknown state transition target `{}`",
                path[0]
            )));
        }

        return;
    }

    if !contained_types.contains_key(path[0].as_str()) {
        diagnostics.push(Diagnostic::error(format!(
            "unknown nested transition receiver `{}`",
            path[0]
        )));
    }
}
