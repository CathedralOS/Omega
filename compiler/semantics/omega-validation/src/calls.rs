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
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_call_node(
    program: &TypedTrees,
    call: &TableCall,
    current_machine: &omega_typed_trees::machine::Machine,
    state_name: &str,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let receiver_members = program.statement_table.name_path_members(call.receiver);
    let arguments = program.statement_table.expression_handles(call.arguments);

    // `Schema::encode_wire(...)`: the wire module owns the synthesized
    // encoder call's diagnostics (chapter 20, wire stage 2a).
    if crate::wire::validate_wire_encode_call(
        program,
        call,
        current_machine,
        machine_symbols.state(state_name),
        diagnostics,
    ) {
        return;
    }

    if receiver_members.is_empty()
        || matches!(receiver_members, [receiver] if receiver.as_str() == "self")
    {
        if let Some(state) = machine_symbols.state(&call.target) {
            validate_result_use(
                program,
                call,
                state.name.as_str(),
                state.return_type,
                diagnostics,
            );
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

        let attached_state = current_machine
            .attached_data
            .as_ref()
            .and_then(|attached_data| {
                symbols.attached_machine_state(
                    program,
                    attached_data.as_str(),
                    call.target.as_str(),
                )
            })
            .map(|(_, state)| state);
        // A receiverless call can also target a FREE top-level machine
        // (`machine compute(item: &Item) -> i32`, called as `compute(item)`);
        // its implicit entry state carries the parameters and return type.
        let Some(state) = attached_state
            .or_else(|| free_machine_entry_state(program, symbols, call.target.as_str()))
        else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has no local state `{}`",
                current_machine.name, call.target
            )));
            return;
        };

        // Diagnostics name the call as spelled (`compute`), not the free
        // machine's generated entry-state name (`entry`).
        validate_result_use(
            program,
            call,
            call.target.as_str(),
            state.return_type,
            diagnostics,
        );
        validate_call_arguments_handles(
            program,
            arguments,
            call.target.as_str(),
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

        validate_result_use(
            program,
            call,
            &state_signature.name,
            state_signature.return_type,
            diagnostics,
        );
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
            validate_result_use(program, call, &state.name, state.return_type, diagnostics);
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
        validate_result_use(program, call, &state.name, state.return_type, diagnostics);
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

    // Boundary/trait receivers (e.g. `self.console.exit_process(0)`) resolve to a
    // trait machine signature. Argument validation for trait calls lives elsewhere;
    // strict result use still applies here.
    if let Some(signature) = receiver_type
        .and_then(|type_name| symbols.trait_definition(type_name))
        .and_then(|trait_definition| {
            program
                .trait_machine_signatures(trait_definition)
                .iter()
                .find(|signature| signature.name == call.target)
        })
    {
        validate_result_use(
            program,
            call,
            &signature.name,
            signature.return_type,
            diagnostics,
        );
        return;
    }

    let _ = diagnostics;
}

/// The entry state of the FREE top-level machine named `target` (`machine
/// compute(item: &Item) -> i32 { ... }`), or None. The parser names a free
/// machine's implicit entry state `entry`; explicit entry states matching the
/// call target name win first.
fn free_machine_entry_state<'program>(
    program: &'program TypedTrees,
    symbols: &TopLevelSymbols<'program>,
    target: &str,
) -> Option<&'program omega_typed_trees::state::State> {
    let machine = symbols.machine(target)?;
    if machine.attached_data.is_some() {
        return None;
    }

    let states = program.machine_states(machine);
    states
        .iter()
        .find(|state| state.name.as_str() == target)
        .or_else(|| states.iter().find(|state| state.name.as_str() == "entry"))
        .or_else(|| states.first())
}

/// FROZEN DECISION 9 -- STRICT RESULT USE: a statement-position call whose callee
/// returns a non-unit value must not silently drop that value. Intentional
/// discards are spelled `_ = call();` (which sets `discards_result`). "Non-unit"
/// means the resolved callee declares a return type (`-> T`) that is not `()`.
fn validate_result_use(
    program: &TypedTrees,
    call: &TableCall,
    target_name: &str,
    return_type: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if call.discards_result || !return_type.is_valid() {
        return;
    }

    if matches!(
        program.type_reference_table.type_reference(return_type),
        TypeReferenceNode::Unit
    ) {
        return;
    }

    diagnostics.push(Diagnostic::error(format!(
        "call to `{target_name}` discards its non-unit `{}` result; consume the value or discard it explicitly with `_ = {target_name}(...);`",
        program.display_type_reference_with_constraints(return_type)
    )));
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
