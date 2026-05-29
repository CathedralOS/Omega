use crate::expression_types::{
    argument_matches_type_reference_handle, expression_type_name_handle,
};
use crate::locals::WritableRoots;
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::signature::StateParameter;
use omega_typed_trees::statement::TableCall;

pub(crate) fn validate_call_node(
    program: &TypedTrees,
    call: &TableCall,
    current_machine: &omega_typed_trees::machine::Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let receiver_members = program.statement_table.name_path_members(call.receiver);
    let arguments = program.statement_table.expression_handles(call.arguments);

    if receiver_members.is_empty()
        || matches!(receiver_members, [receiver] if receiver.as_str() == "self")
    {
        if let Some(state) = machine_symbols.state(&call.target) {
            validate_call_arguments_handles(
                program,
                arguments,
                state.name.as_str(),
                program.state_parameters(state),
                writable_roots,
                diagnostics,
            );
            return;
        }

        let Some((_, state)) = current_machine
            .attached_data
            .as_ref()
            .and_then(|attached_data| {
                symbols.attached_machine_state(
                    program,
                    attached_data.as_str(),
                    call.target.as_str(),
                )
            })
        else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has no local state `{}`",
                current_machine.name, call.target
            )));
            return;
        };

        validate_call_arguments_handles(
            program,
            arguments,
            state.name.as_str(),
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );
        return;
    }

    let receiver = receiver_members
        .last()
        .map(|member| member.as_str())
        .unwrap_or_default();
    let receiver_type = machine_symbols.contained_type(receiver);

    if let Some(platform) = receiver_type.and_then(|type_name| symbols.platform(type_name)) {
        let Some(state_signature) = program
            .platform_state_signatures(platform)
            .iter()
            .find(|state| state.name == call.target)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "platform `{}` has no state `{}`",
                platform.name, call.target
            )));
            return;
        };

        validate_call_arguments_handles(
            program,
            arguments,
            &state_signature.name,
            program.state_signature_parameters(state_signature),
            writable_roots,
            diagnostics,
        );
        return;
    }

    if let Some(machine) = receiver_type
        .and_then(|type_name| symbols.machine(type_name))
        .or_else(|| symbols.machine(receiver))
    {
        if let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.name == call.target)
        {
            validate_call_arguments_handles(
                program,
                arguments,
                &state.name,
                program.state_parameters(state),
                writable_roots,
                diagnostics,
            );
            return;
        };

        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` has no state `{}`",
            machine.name, call.target
        )));
        return;
    }

    if let Some((_, state)) = receiver_type.and_then(|type_name| {
        symbols.attached_machine_state(program, type_name, call.target.as_str())
    }) {
        validate_call_arguments_handles(
            program,
            arguments,
            &state.name,
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );
        return;
    }

    let _ = diagnostics;
}

pub(crate) fn validate_call_arguments_handles(
    program: &TypedTrees,
    arguments: &[ExpressionHandle],
    target_name: &str,
    parameters: &[StateParameter],
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let callable_parameter_count = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();

    if arguments.len() != callable_parameter_count {
        diagnostics.push(Diagnostic::error(format!(
            "state `{}` expects {} argument(s), got {}",
            target_name,
            callable_parameter_count,
            arguments.len()
        )));
        return;
    }

    for (argument, parameter) in arguments
        .iter()
        .zip(parameters.iter().filter(|parameter| !parameter.is_self))
    {
        let is_mutable = matches!(
            program.expression_table.expression(*argument),
            ExpressionNode::Mutable(_)
        );

        if parameter.is_mutable && !is_mutable {
            continue;
        }

        if !parameter.is_mutable && is_mutable {
            continue;
        }

        let expected_type =
            program.display_type_reference_with_constraints(parameter.type_reference);

        if !argument_matches_type_reference_handle(program, *argument, parameter.type_reference) {
            diagnostics.push(Diagnostic::error(format!(
                "argument `{}` for state `{}` expects `{}`, got `{}`",
                parameter.name,
                target_name,
                expected_type,
                expression_type_name_handle(program, *argument)
            )));
        }
    }

    let _ = (writable_roots, diagnostics);
}
