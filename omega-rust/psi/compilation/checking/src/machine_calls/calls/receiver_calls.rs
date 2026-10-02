//! Calls on a named receiver: the requirement, machine state, attached
//! state or boundary trait signature the receiver's type resolves to.

use super::{
    CallScope, boundary_trait_call_signature, call_gates, declared_receiver_type_reference,
    generic_requirement, machine_state_by_symbol, validate_call_arguments_handles,
    validate_generic_bound_argument_types, validate_machine_call_type_parameter_bounds,
    validate_resolved_target_type_parameter_bounds, validate_result_use,
};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateSignature;
use typed_trees::statement::TableCall;

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
    // A namespace supplies no runtime receiver. Rejoin its resolved target
    // before the legacy place/name ladder, which can see a different local or
    // declaration sharing the namespace's leaf spelling. Operator declarations
    // keep their separate signature validator; absence of an ordinary selected
    // target is a rejection, never permission to search by name again.
    if call_gates::receiver_is_declaration_namespace(scope, receiver_members) {
        if !validate_resolved_receiver_call(scope, arguments, true, diagnostics)
            && !call_gates::receiver_is_operator_namespace(scope, receiver_members, arguments.len())
        {
            diagnostics.push(Diagnostic::error(format!(
                "qualified call `{}` has no resolved callable declaration",
                scope.call.target,
            )));
        }
        return;
    }
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
            .first()
            .filter(|state| state.name == call.target)
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

    validate_resolved_receiver_call(scope, arguments, false, diagnostics);
}

/// The exact target owns the call's signature. Namespace and runtime-place
/// calls share these checks, but only a namespace supplies every parameter as
/// an explicit argument while legacy instance calls bind their receiver.
fn validate_resolved_receiver_call(
    scope: &CallScope<'_>,
    arguments: &[ExpressionHandle],
    namespace: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let CallScope {
        program,
        call,
        current_machine,
        current_state,
        symbols,
        writable_roots,
        value_environment,
        ..
    } = *scope;
    // Resolved-target rung (see the function doc): every receiver kind the
    // name ladder misses still resolved `call.target_symbol`. An exact
    // state target validates against its own machine and state; a boundary
    // parameter target rejoins its selected trait signature through the
    // write-frame owner's exact receiver/symbol join, which here also admits
    // a requirement generic only over `machine` binders (a callback
    // registrar). Return whether a declaration was found so namespace
    // callers reject missing identity.
    if let Some((callee_machine, state)) = machine_state_by_symbol(program, call.target_symbol) {
        if namespace
            && callee_machine.attached_data_symbol.is_valid()
            && call.receiver_symbol != callee_machine.attached_data_symbol
        {
            diagnostics.push(Diagnostic::error(format!(
                "selected attached call `{}` does not belong to the receiver's exact data type",
                call.target,
            )));
            return true;
        }
        // A receiver that names a declaration rather than a runtime place
        // (`Receipt::ack(value)`, `Domain::content(&v)`) supplies the callee's
        // `self` parameter as an explicit argument, exactly as the lowering's
        // `explicit_self` rule does. A place receiver -- parameter, local,
        // field, or nested member path -- binds `self` through the receiver
        // itself, so `self` stays out of the argument correspondence.
        let self_is_argument = namespace;
        validate_result_use(
            program,
            call,
            state.name.as_str(),
            state.return_type,
            diagnostics,
        );
        super::type_arguments::validate_resolved_call_arguments(
            program,
            current_machine,
            current_state,
            value_environment,
            arguments,
            state.name.as_str(),
            callee_machine,
            state,
            &call.machine_arguments,
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
        return true;
    }
    if let Some(signature) = boundary_trait_call_signature(program, current_machine, call) {
        validate_result_use(
            program,
            call,
            &signature.name,
            signature.return_type,
            diagnostics,
        );
        // Runtime formals only: native callback destinations live outside
        // `parameters` and are joined to their selections below.
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
        validate_native_callback_selections(program, call, signature, diagnostics);
        return true;
    }
    false
}

/// A `native callback name from Binder` destination has no source runtime
/// argument (private_callbacks.md, "Authored selection"): the registrar call
/// omits it, and its callee is the call's static machine argument for that
/// binder. Each destination must therefore name one `machine` binder of the
/// selected signature, and the call must supply that binder's selection at
/// its machine ordinal. MP2b separately admits the selection against the
/// binder's `where machine ... satisfies` bound; this check only refuses a
/// destination with no selection to bind, which later stages would otherwise
/// have to reject without a source diagnostic.
fn validate_native_callback_selections(
    program: &TypedTrees,
    call: &TableCall,
    signature: &StateSignature,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if signature.native_callback_parameters.is_empty() {
        return;
    }
    let machine_binders = program
        .state_signature_type_parameters(signature)
        .iter()
        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Machine { .. }))
        .collect::<Vec<_>>();
    let selections = crate::machine_calls::machine_parameters::machine_selection_arguments(
        program,
        &call.machine_arguments,
    )
    .collect::<Vec<_>>();
    for callback in &signature.native_callback_parameters {
        let mut binders = machine_binders
            .iter()
            .enumerate()
            .filter(|(_, binder)| binder.name == callback.binder);
        let (Some((ordinal, _)), None) = (binders.next(), binders.next()) else {
            diagnostics.push(Diagnostic::error(format!(
                "native callback `{}` of `{}` names `{}`, which is not exactly one `machine` binder of the signature",
                callback.name, signature.name, callback.binder,
            )));
            continue;
        };
        if selections.get(ordinal).is_none() {
            diagnostics.push(Diagnostic::error(format!(
                "call `{}` supplies no static machine argument for native callback `{}` from `{}`",
                call.target, callback.name, callback.binder,
            )));
        }
    }
}
