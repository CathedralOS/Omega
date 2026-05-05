use crate::ast;
use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::ir::command::{CommandParameter, CommandSignature};
use crate::ir::data::{DataDefinition, DataField, DataMember, DataVariant};
use crate::ir::expression::Expression;
use crate::ir::machine::{ContainedObject, Machine, OwnedData};
use crate::ir::platform::Platform;
use crate::ir::state::State;
use crate::ir::statement::{Assignment, CommandCall, Statement, Transition, TransitionTarget};
use crate::ir::types::TypeReference;

pub fn lower_program(items: &[ast::item::Item]) -> Result<Program, Diagnostic> {
    let mut program = Program::default();

    for item in items {
        match item {
            ast::item::Item::Data(data_definition) => {
                program
                    .data_definitions
                    .push(lower_data_definition(data_definition)?);
            }
            ast::item::Item::Use(_) => {}
            ast::item::Item::Machine(machine) => {
                program.machines.push(lower_machine(machine)?);
            }
            ast::item::Item::Platform(platform) => {
                program.platforms.push(lower_platform(platform)?);
            }
        }
    }

    Ok(program)
}

fn lower_data_definition(
    data_definition: &ast::item::DataDefinition,
) -> Result<DataDefinition, Diagnostic> {
    let members = data_definition
        .members
        .iter()
        .map(lower_data_member)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(DataDefinition {
        name: data_definition.name.clone(),
        members,
    })
}

fn lower_data_member(member: &ast::item::DataMember) -> Result<DataMember, Diagnostic> {
    match member {
        ast::item::DataMember::Field(field) => Ok(DataMember::Field(DataField {
            name: field.name.clone(),
            type_reference: lower_type_reference(&field.type_reference)?,
        })),
        ast::item::DataMember::Variant(variant) => Ok(DataMember::Variant(DataVariant {
            name: variant.name.clone(),
        })),
    }
}

fn lower_machine(machine: &ast::item::Machine) -> Result<Machine, Diagnostic> {
    let contains = machine
        .contains
        .iter()
        .map(|contained_object| ContainedObject {
            name: contained_object.name.clone(),
            type_name: contained_object.type_name.clone(),
        })
        .collect();

    let commands = machine
        .commands
        .iter()
        .map(|command| lower_command_signature(&command.signature))
        .collect::<Result<Vec<_>, _>>()?;

    let owned_data = machine
        .owned_data
        .iter()
        .map(lower_owned_data)
        .collect::<Result<Vec<_>, _>>()?;

    let states = machine
        .states
        .iter()
        .map(lower_state)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Machine {
        name: machine.name.clone(),
        commands,
        contains,
        owned_data,
        states,
    })
}

fn lower_owned_data(owned_data: &ast::item::OwnedData) -> Result<OwnedData, Diagnostic> {
    Ok(OwnedData {
        name: owned_data.name.clone(),
        type_reference: lower_type_reference(&owned_data.type_reference)?,
        initial_value: owned_data
            .initial_value
            .as_ref()
            .map(lower_expression)
            .transpose()?,
    })
}

fn lower_platform(platform: &ast::item::Platform) -> Result<Platform, Diagnostic> {
    let commands = platform
        .commands
        .iter()
        .map(lower_command_signature)
        .collect::<Result<Vec<_>, Diagnostic>>()?;

    Ok(Platform {
        name: platform.name.clone(),
        commands,
    })
}

fn lower_command_signature(
    command: &ast::item::CommandSignature,
) -> Result<CommandSignature, Diagnostic> {
    Ok(CommandSignature {
        name: command.name.clone(),
        parameters: command
            .parameters
            .iter()
            .map(|parameter| {
                Ok(CommandParameter {
                    name: parameter.name.clone(),
                    type_reference: lower_type_reference(&parameter.type_reference)?,
                    is_mutable: parameter.is_mutable,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?,
    })
}

fn lower_type_reference(
    type_reference: &ast::types::TypeReference,
) -> Result<TypeReference, Diagnostic> {
    match type_reference {
        ast::types::TypeReference::FixedArray {
            element_type,
            length,
        } => Ok(TypeReference::FixedArray {
            element_type: Box::new(lower_type_reference(element_type)?),
            length: *length,
        }),
        ast::types::TypeReference::Named(name) => Ok(TypeReference::Named(name.clone())),
    }
}

fn lower_state(state: &ast::item::State) -> Result<State, Diagnostic> {
    let statements = state
        .statements
        .iter()
        .map(lower_statement)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(State {
        name: state.name.clone(),
        statements,
    })
}

fn lower_statement(statement: &ast::statement::Statement) -> Result<Statement, Diagnostic> {
    match statement {
        ast::statement::Statement::Assignment(assignment) => {
            Ok(Statement::Assignment(Assignment {
                target: assignment.target.clone(),
                value: lower_expression(&assignment.value)?,
            }))
        }
        ast::statement::Statement::CommandCall(command_call) => {
            Ok(Statement::CommandCall(CommandCall {
                receiver: command_call.receiver.clone(),
                command: command_call.command.clone(),
                arguments: command_call
                    .arguments
                    .iter()
                    .map(lower_expression)
                    .collect::<Result<Vec<_>, _>>()?,
            }))
        }
        ast::statement::Statement::Transition(transition) => {
            Ok(Statement::Transition(Transition {
                target: lower_transition_target(&transition.target),
                continuation: transition
                    .continuation
                    .as_ref()
                    .map(lower_transition_target),
                condition: transition.condition.clone(),
            }))
        }
    }
}

fn lower_expression(expression: &ast::expression::Expression) -> Result<Expression, Diagnostic> {
    match expression {
        ast::expression::Expression::Integer(value) => Ok(Expression::Integer(*value)),
        ast::expression::Expression::Mutable(inner_expression) => Ok(Expression::Mutable(
            Box::new(lower_expression(inner_expression)?),
        )),
        ast::expression::Expression::Name(path) => Ok(Expression::Name(path.clone())),
        ast::expression::Expression::String(value) => Ok(Expression::String(value.clone())),
    }
}

fn lower_transition_target(target: &ast::statement::TransitionTarget) -> TransitionTarget {
    match target {
        ast::statement::TransitionTarget::Named(path) => TransitionTarget::Named(path.clone()),
        ast::statement::TransitionTarget::SelfTarget => TransitionTarget::SelfTarget,
        ast::statement::TransitionTarget::ReturnToCaller => TransitionTarget::ReturnToCaller,
    }
}
