use std::collections::HashSet;

use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::ir::command::CommandSignature;
use crate::ir::data::DataMember;
use crate::ir::expression::Expression;
use crate::ir::statement::{Statement, TransitionTarget};
use crate::ir::types::{PrimitiveType, TypeReference};
use crate::semantic::symbols::{MachineSymbols, ProgramSymbols};

pub fn validate_program(program: &Program) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let symbols = ProgramSymbols::build(program, &mut diagnostics);

    validate_top_level_command_signatures(program, &symbols, &mut diagnostics);
    validate_data_field_types(program, &symbols, &mut diagnostics);
    validate_entry_point(program, &mut diagnostics);

    for machine in &program.machines {
        let machine_symbols = MachineSymbols::build(machine, &mut diagnostics);

        validate_contained_types(machine, &symbols, &mut diagnostics);
        validate_owned_data(machine, &symbols, &mut diagnostics);

        for state in &machine.states {
            for statement in &state.statements {
                match statement {
                    Statement::Assignment(_) => {}
                    Statement::CommandCall(command_call) => validate_command_call(
                        command_call,
                        machine,
                        &machine_symbols,
                        &symbols,
                        &mut diagnostics,
                    ),
                    Statement::LocalData(local_data) => validate_type_reference(
                        &local_data.type_reference,
                        &symbols,
                        &mut diagnostics,
                        format!(
                            "machine `{}` local data `{}`",
                            machine.name, local_data.name
                        ),
                    ),
                    Statement::Transition(transition) => {
                        validate_transition_target(
                            &transition.target,
                            &machine_symbols,
                            &mut diagnostics,
                        );

                        if let Some(continuation) = &transition.continuation {
                            validate_transition_target(
                                continuation,
                                &machine_symbols,
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

fn validate_top_level_command_signatures(
    program: &Program,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in &program.machines {
        validate_command_signature_types(
            machine.commands.iter().map(|command| &command.signature),
            symbols,
            diagnostics,
            format!("machine `{}`", machine.name),
        );
    }
}

fn validate_command_signature_types<'a>(
    signatures: impl Iterator<Item = &'a CommandSignature>,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: String,
) {
    for command in signatures {
        for parameter in &command.parameters {
            validate_type_reference(
                &parameter.type_reference,
                symbols,
                diagnostics,
                format!(
                    "{owner} command `{}` parameter `{}`",
                    command.name, parameter.name
                ),
            );
        }
    }
}

fn validate_data_field_types(
    program: &Program,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for data_definition in &program.data_definitions {
        validate_data_member_names(data_definition, diagnostics);

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

    for platform in &program.platforms {
        for command in &platform.commands {
            for parameter in &command.parameters {
                validate_type_reference(
                    &parameter.type_reference,
                    symbols,
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

fn validate_data_member_names(
    data_definition: &crate::ir::data::DataDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut member_names = HashSet::new();

    for member in &data_definition.members {
        let member_name = match member {
            DataMember::Field(field) => field.name.as_str(),
            DataMember::Variant(variant) => variant.name.as_str(),
        };

        if !member_names.insert(member_name) {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` has duplicate member `{member_name}`",
                data_definition.name
            )));
        }
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

    if !main_machine.states.iter().any(|state| state.name == "Main") {
        diagnostics.push(Diagnostic::error("machine main is missing state Main"));
    }
}

fn validate_contained_types(
    machine: &crate::ir::machine::Machine,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for contained_object in &machine.contains {
        if !symbols.is_command_receiver_type(&contained_object.type_name) {
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

fn validate_command_call(
    command_call: &crate::ir::statement::CommandCall,
    current_machine: &crate::ir::machine::Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &ProgramSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(receiver) = command_call.receiver.as_deref() else {
        let Some(command_definition) = machine_symbols.command(&command_call.command) else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has no local command `{}`",
                current_machine.name, command_call.command
            )));
            return;
        };

        validate_command_arguments(command_call, &command_definition.signature, diagnostics);
        return;
    };

    let Some(receiver_type) = machine_symbols.contained_type(receiver) else {
        diagnostics.push(Diagnostic::error(format!(
            "unknown command receiver `{}`",
            receiver
        )));
        return;
    };

    if let Some(platform) = symbols.platform(receiver_type) {
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

    if let Some(machine) = symbols.machine(receiver_type) {
        let Some(command_signature) = machine
            .commands
            .iter()
            .find(|command| command.signature.name == command_call.command)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has no command `{}`",
                machine.name, command_call.command
            )));
            return;
        };

        validate_command_arguments(command_call, &command_signature.signature, diagnostics);
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
