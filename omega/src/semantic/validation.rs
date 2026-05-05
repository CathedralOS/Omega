use std::collections::{HashMap, HashSet};

use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::ir::statement::{Statement, TransitionTarget};

pub fn validate_program(program: &Program) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    validate_entry_point(program, &mut diagnostics);

    let platforms = program
        .platforms
        .iter()
        .map(|platform| (platform.name.as_str(), platform))
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

        for state in &machine.states {
            for statement in &state.statements {
                match statement {
                    Statement::CommandCall(command_call) => validate_command_call(
                        command_call,
                        &contained_types,
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

fn validate_command_call(
    command_call: &crate::ir::statement::CommandCall,
    contained_types: &HashMap<&str, &str>,
    platforms: &HashMap<&str, &crate::ir::platform::Platform>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(receiver_type) = contained_types.get(command_call.receiver.as_str()) else {
        diagnostics.push(Diagnostic::error(format!(
            "unknown command receiver `{}`",
            command_call.receiver
        )));
        return;
    };

    let Some(platform) = platforms.get(receiver_type) else {
        return;
    };

    if !platform
        .commands
        .iter()
        .any(|command| command.name == command_call.command)
    {
        diagnostics.push(Diagnostic::error(format!(
            "platform `{}` has no command `{}`",
            platform.name, command_call.command
        )));
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
