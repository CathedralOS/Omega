//! Calls on a named receiver: the requirement, machine state, attached
//! state or boundary trait signature the receiver's type resolves to.

use super::{
    CallScope, boundary_trait_signature, declared_receiver_type_reference, generic_requirement,
    machine_state_by_symbol, validate_call_arguments_handles,
    validate_call_arguments_handles_with_self_argument, validate_generic_bound_argument_types,
    validate_machine_call_type_parameter_bounds, validate_resolved_target_type_parameter_bounds,
    validate_result_use,
};
use diagnostics::Diagnostic;
use typed_trees::expression::ExpressionHandle;

/// A call on a named receiver targets, in order: a dynamic requirement (an
/// error), a generic bound requirement, a state of the receiver's machine,
/// an attached machine's state, or a boundary trait signature; each
/// validates the call's result use and arguments.
///
/// The name-based rungs below can only see a receiver that is a callable
/// FIELD of the current machine (`machine_symbols.callable_field_type`), a
/// bare type/trait/machine spelling, or a declared receiver named by the
/// path's last member. A receiver that is a state parameter, a local
/// binding, a nested value path (`outer.inner.inspect()`), a type name, or
/// a boundary parameter matches none of them -- yet the call's
/// `target_symbol` is still resolved to the exact state or signature the
/// lowering invokes. Falling through admitted such calls with NO result-use,
/// arity, or type-parameter-bound check at all, so a statement-position
/// `carrier.context.increment_counter();` silently dropped its `u64`
/// result. The final rung therefore rejoins the resolved target and applies
/// the same ordinary validation every other rung runs.
pub(super) fn validate_receiver_call(
    scope: &CallScope<'_>,
    receiver_members: &[typed_trees::name::Identifier],
    arguments: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let CallScope {
        program,
        call,
        current_machine,
        current_state,
        machine_symbols,
        symbols,
        writable_roots,
        value_environment,
        ..
    } = *scope;
    let receiver = receiver_members
        .last()
        .map(|member| member.as_str())
        .unwrap_or_default();
    let receiver_type = machine_symbols.callable_field_type(receiver);
    let receiver_type_reference = current_state.and_then(|state| {
        declared_receiver_type_reference(program, current_machine, state, receiver)
    });

    if let Some(error) = receiver_type_reference.and_then(|type_reference| {
        crate::declarations::traits::dynamic_requirement_call_error(
            program,
            type_reference,
            call.target.as_str(),
            call.target_symbol,
        )
    }) {
        diagnostics.push(Diagnostic::error(error));
        return;
    }

    if let Some(type_reference) = receiver_type_reference {
        match crate::declarations::traits::generic_bound_requirement_call(
            program,
            current_machine,
            type_reference,
            call.target.as_str(),
        ) {
            Ok(Some(requirement)) => {
                let signature = requirement.signature;
                validate_result_use(
                    program,
                    call,
                    signature.name.as_str(),
                    signature.return_type,
                    diagnostics,
                );
                validate_generic_bound_argument_types(
                    program,
                    current_machine,
                    current_state,
                    type_reference,
                    arguments,
                    &requirement,
                    diagnostics,
                );
                generic_requirement::validate_requirement_call_arguments(
                    program,
                    current_machine,
                    current_state,
                    value_environment,
                    arguments,
                    program.state_signature_parameters(signature),
                    Some(type_reference),
                    &requirement,
                    writable_roots,
                    diagnostics,
                );
                return;
            }
            Ok(None) => {}
            Err(error) => {
                diagnostics.push(Diagnostic::error(error));
                return;
            }
        }
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
                current_machine,
                current_state,
                value_environment,
                arguments,
                &state.name,
                program.state_parameters(state),
                Some(state),
                writable_roots,
                diagnostics,
            );
            validate_machine_call_type_parameter_bounds(
                program,
                symbols,
                machine,
                state,
                state.name.as_str(),
                arguments,
                current_machine,
                current_state,
                false,
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

    if let Some((callee_machine, state)) = receiver_type.and_then(|type_name| {
        symbols.attached_machine_state(program, type_name, call.target.as_str())
    }) {
        validate_result_use(program, call, &state.name, state.return_type, diagnostics);
        validate_call_arguments_handles(
            program,
            current_machine,
            current_state,
            value_environment,
            arguments,
            &state.name,
            program.state_parameters(state),
            Some(state),
            writable_roots,
            diagnostics,
        );
        validate_machine_call_type_parameter_bounds(
            program,
            symbols,
            callee_machine,
            state,
            state.name.as_str(),
            arguments,
            current_machine,
            current_state,
            false,
            diagnostics,
        );
        return;
    }

    // Boundary/trait receivers (e.g. `self.console.exit_process(0)`) resolve to a
    // trait machine signature. Strict result use plus argument validation apply
    // here -- a boundary is still a typed call, and a cross-class argument
    // (`exit_process(self.bool_field)`) would otherwise reach the host encoder as
    // a raw byte and be read as garbage with no frontend error.
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
        validate_call_arguments_handles(
            program,
            current_machine,
            current_state,
            value_environment,
            arguments,
            &signature.name,
            program.state_signature_parameters(signature),
            None,
            writable_roots,
            diagnostics,
        );
        return;
    }

    // Resolved-target rung (see the function doc): every receiver kind the
    // name ladder misses still resolved `call.target_symbol`. An exact
    // state target validates against its own machine and state; a boundary
    // parameter target rejoins its selected trait signature through the
    // write-frame owner's exact receiver/symbol join. An unresolved target
    // (invalid symbol) keeps the previous silence -- nothing here invents a
    // callee.
    if let Some((callee_machine, state)) = machine_state_by_symbol(program, call.target_symbol) {
        // A receiver that names a declaration rather than a runtime place
        // (`Receipt::ack(value)`, `Domain::content(&v)`) supplies the callee's
        // `self` parameter as an explicit argument, exactly as the lowering's
        // `explicit_self` rule does. A place receiver -- parameter, local,
        // field, or nested member path -- binds `self` through the receiver
        // itself, so `self` stays out of the argument correspondence.
        let self_is_argument = matches!(
            program.symbols.get(call.receiver_symbol).kind,
            symbols::SymbolKind::BuiltinType
                | symbols::SymbolKind::Data
                | symbols::SymbolKind::Domain
                | symbols::SymbolKind::Machine
                | symbols::SymbolKind::Module
                | symbols::SymbolKind::Trait
                | symbols::SymbolKind::ConformanceParameter
        );
        validate_result_use(
            program,
            call,
            state.name.as_str(),
            state.return_type,
            diagnostics,
        );
        validate_call_arguments_handles_with_self_argument(
            program,
            current_machine,
            current_state,
            value_environment,
            arguments,
            state.name.as_str(),
            program.state_parameters(state),
            Some(state),
            writable_roots,
            self_is_argument,
            diagnostics,
        );
        validate_resolved_target_type_parameter_bounds(
            program,
            symbols,
            callee_machine,
            state,
            state.name.as_str(),
            arguments,
            current_machine,
            current_state,
            self_is_argument,
            diagnostics,
        );
        return;
    }
    if let Some(signature) = boundary_trait_signature(program, current_machine, call) {
        validate_result_use(
            program,
            call,
            &signature.name,
            signature.return_type,
            diagnostics,
        );
        validate_call_arguments_handles(
            program,
            current_machine,
            current_state,
            value_environment,
            arguments,
            &signature.name,
            program.state_signature_parameters(signature),
            None,
            writable_roots,
            diagnostics,
        );
    }
}
