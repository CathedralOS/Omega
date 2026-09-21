//! Receiverless and `self` calls: the machine parameter, specialized entry,
//! local state, attached machine or free machine a bare call targets.

use super::{
    CallScope, free_machine_entry_state, machine_state_by_symbol, validate_call_arguments_handles,
    validate_machine_call_type_parameter_bounds, validate_result_use,
};
use diagnostics::Diagnostic;
use typed_trees::expression::ExpressionHandle;

/// A receiverless (or `self`) call targets, in order: a machine parameter
/// signature, a specialized concrete entry of another machine, a local
/// state, or an attached or free machine's entry state; each validates the
/// call's result use, arguments and type-parameter bounds.
pub(super) fn validate_receiverless_call(
    scope: &CallScope<'_>,
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
        value_env,
        ..
    } = *scope;
    if let Some(signature) =
        program.machine_parameter_signature_in(current_machine, call.target_symbol)
    {
        validate_result_use(
            program,
            call,
            signature.name.as_str(),
            signature.return_type,
            diagnostics,
        );
        validate_call_arguments_handles(
            program,
            current_machine,
            current_state,
            value_env,
            arguments,
            signature.name.as_str(),
            program.state_signature_parameters(signature),
            None,
            writable_roots,
            diagnostics,
        );
        return;
    }

    // MP4 specializes `F(args)` to the selected concrete ENTRY symbol.
    // It remains receiverless because the whole callable parameter list
    // (including any explicit data argument) is already present.
    if let Some((callee_machine, state)) = machine_state_by_symbol(program, call.target_symbol)
        && callee_machine.symbol != current_machine.symbol
    {
        validate_result_use(
            program,
            call,
            state.name.as_str(),
            state.return_type,
            diagnostics,
        );
        validate_call_arguments_handles(
            program,
            current_machine,
            current_state,
            value_env,
            arguments,
            state.name.as_str(),
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
            current_machine,
            current_state,
            value_env,
            arguments,
            call.target.as_str(),
            program.state_parameters(state),
            Some(state),
            writable_roots,
            diagnostics,
        );
        validate_machine_call_type_parameter_bounds(
            program,
            symbols,
            current_machine,
            state,
            call.target.as_str(),
            arguments,
            current_machine,
            current_state,
            false,
            diagnostics,
        );
        return;
    }

    // A receiverless call resolved to a state of THIS machine while no local
    // state owns the spelled name is a recursive machine call onto the
    // machine's own entry (`walk(n - 1);` inside `machine walk`). Result
    // overload rebinding already chose the exact sibling; falling through to
    // the name lookups below would re-choose the first-declared overload.
    if let Some((_, state)) = machine_state_by_symbol(program, call.target_symbol)
        .filter(|(callee_machine, _)| callee_machine.symbol == current_machine.symbol)
    {
        validate_result_use(
            program,
            call,
            call.target.as_str(),
            state.return_type,
            diagnostics,
        );
        validate_call_arguments_handles(
            program,
            current_machine,
            current_state,
            value_env,
            arguments,
            call.target.as_str(),
            program.state_parameters(state),
            Some(state),
            writable_roots,
            diagnostics,
        );
        validate_machine_call_type_parameter_bounds(
            program,
            symbols,
            current_machine,
            state,
            call.target.as_str(),
            arguments,
            current_machine,
            current_state,
            false,
            diagnostics,
        );
        return;
    }

    let attached_state = current_machine
        .attached_data
        .as_ref()
        .and_then(|attached_data| {
            symbols.attached_machine_state(program, attached_data.as_str(), call.target.as_str())
        });
    // A receiverless call can also target a FREE top-level machine
    // (`machine compute(item: &Item) -> i32`, called as `compute(item)`);
    // its implicit entry state carries the parameters and return type.
    let Some((callee_machine, state)) =
        attached_state.or_else(|| free_machine_entry_state(program, symbols, call.target.as_str()))
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
        current_machine,
        current_state,
        value_env,
        arguments,
        call.target.as_str(),
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
        call.target.as_str(),
        arguments,
        current_machine,
        current_state,
        false,
        diagnostics,
    );
}
