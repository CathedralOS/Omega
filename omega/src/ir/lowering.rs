use crate::ast;
use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::ir::command::CommandSignature;
use crate::ir::expression::Expression;
use crate::ir::machine::{ContainedObject, Machine};
use crate::ir::platform::Platform;
use crate::ir::state::State;
use crate::ir::statement::{CommandCall, Statement};

pub fn lower_program(items: &[ast::item::Item]) -> Result<Program, Diagnostic> {
    let mut program = Program::default();

    for item in items {
        match item {
            ast::item::Item::Use(_) => {}
            ast::item::Item::Machine(machine) => {
                program.machines.push(lower_machine(machine)?);
            }
            ast::item::Item::Platform(platform) => {
                program.platforms.push(lower_platform(platform));
            }
        }
    }

    Ok(program)
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

    let states = machine
        .states
        .iter()
        .map(lower_state)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Machine {
        name: machine.name.clone(),
        contains,
        states,
    })
}

fn lower_platform(platform: &ast::item::Platform) -> Platform {
    let commands = platform
        .commands
        .iter()
        .map(|command| CommandSignature {
            name: command.name.clone(),
        })
        .collect();

    Platform {
        name: platform.name.clone(),
        commands,
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
        ast::statement::Statement::Transition(_) => Err(Diagnostic::error(
            "explicit transition lowering is not wired into the MVP executable path yet",
        )),
    }
}

fn lower_expression(expression: &ast::expression::Expression) -> Result<Expression, Diagnostic> {
    match expression {
        ast::expression::Expression::Integer(value) => Ok(Expression::Integer(*value)),
        ast::expression::Expression::String(value) => Ok(Expression::String(value.clone())),
    }
}
