//! Receiverless and `self` calls: the machine parameter, specialized entry,
//! attached machine or free machine entry a bare call targets.

use super::{
    CallScope, free_machine_entry_state, machine_state_by_symbol,
    type_arguments::validate_resolved_call_arguments, validate_machine_call_type_parameter_bounds,
    validate_result_use,
};
use diagnostics::Diagnostic;
use typed_trees::expression::ExpressionHandle;

/// A receiverless (or `self`) call targets, in order: a machine parameter
/// signature, a specialized concrete entry, or an attached or free machine's
/// entry state; each validates the call's result use, arguments and
/// type-parameter bounds.
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
        symbols,
        writable_roots,
        value_environment,
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
        super::validate_machine_parameter_arguments(
            program,
            current_machine,
            current_state,
            value_environment,
            arguments,
            signature,
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
        validate_resolved_call_arguments(
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
            false,
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

    // A receiverless call resolved to this machine's entry is recursive
    // (`walk(n - 1);` inside `machine walk`). Result
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
        validate_resolved_call_arguments(
            program,
            current_machine,
            current_state,
            value_environment,
            arguments,
            call.target.as_str(),
            current_machine,
            state,
            &call.machine_arguments,
            writable_roots,
            false,
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
        // Internal states are transition destinations, never callables
        // (state_contracts.md: "Ordinary calls invoke machines, not internal
        // states"). Name that directly: the old resolver accepted this form,
        // so authors migrating code need the transition spelling, not a
        // generic unresolved-name error.
        if program
            .machine_states(current_machine)
            .iter()
            .skip(1)
            .any(|state| state.name.as_str() == call.target.as_str())
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` calls its internal state `{}` as an ordinary call; ordinary \
                 calls invoke machines, not states: enter the state with \
                 `transition {{ _ -> {}(..) }}`, or make it a machine if its result is used \
                 as a value",
                current_machine.name, call.target, call.target
            )));
            return;
        }
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` cannot resolve ordinary call `{}` to a machine entry or requirement",
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
    validate_resolved_call_arguments(
        program,
        current_machine,
        current_state,
        value_environment,
        arguments,
        call.target.as_str(),
        callee_machine,
        state,
        &call.machine_arguments,
        writable_roots,
        false,
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
