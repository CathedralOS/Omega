use crate::arithmetic_domains::{ValueEnv, call_return_type};
use crate::calls::validate_call_arguments_handles;
use crate::locals::WritableRoots;
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::signature::StateParameter;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{TransitionTargetHandle, TransitionTargetNode};

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_transition_target_node(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&State>,
    value_env: &ValueEnv,
    target: TransitionTargetHandle,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let TransitionTargetNode::Named { path, arguments } =
        program.statement_table.transition_target(target)
    else {
        return;
    };

    let path = program.statement_table.name_path_members(path.members);
    let arguments = program.statement_table.expression_handles(*arguments);

    if path.len() == 1 {
        let Some(state) = machine_symbols.state(path[0].as_str()) else {
            return;
        };

        validate_transition_arguments_handles(
            program,
            current_machine,
            current_state,
            value_env,
            arguments,
            state.name.as_str(),
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );

        return;
    }

    if path.len() == 2 && path[0].as_str() == "self" {
        let Some(state) = machine_symbols.state(path[1].as_str()) else {
            return;
        };

        validate_transition_arguments_handles(
            program,
            current_machine,
            current_state,
            value_env,
            arguments,
            state.name.as_str(),
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );
        return;
    }

    let Some(receiver_type) = machine_symbols.contained_type(path[0].as_str()) else {
        return;
    };

    if path.len() == 2 {
        let Some(machine) = symbols.machine(receiver_type) else {
            return;
        };

        let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.name == path[1])
        else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has no state `{}`",
                machine.name, path[1]
            )));
            return;
        };

        validate_transition_arguments_handles(
            program,
            current_machine,
            current_state,
            value_env,
            arguments,
            &state.name,
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_transition_arguments_handles(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&State>,
    value_env: &ValueEnv,
    arguments: &[ExpressionHandle],
    target_name: &str,
    parameters: &[StateParameter],
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    reject_scalar_value_call_arguments(
        program,
        current_machine,
        current_state,
        arguments,
        target_name,
        diagnostics,
    );
    validate_call_arguments_handles(
        program,
        current_machine,
        current_state,
        value_env,
        arguments,
        target_name,
        parameters,
        writable_roots,
        diagnostics,
    );
}

/// A SCALAR/BOOL user value-machine call used DIRECTLY as a transition
/// argument is a known silent miscompile: the backend never materializes the
/// call result into the argument slot, so the target state's parameter reads
/// 0 -- and with two such calls in one transition, a paired argument reads the
/// wrong call's result. Reject the shape with the sound workaround instead of
/// shipping wrong values. Scope (deliberate): only calls `call_return_type`
/// resolves -- a self/sibling-attached value machine with a DECLARED primitive
/// return. Builtin methods (`opt.unwrap()`), string/view-returning machines,
/// and method-on-local calls resolve to None and stay accepted (strings work
/// via the carrier path; the rest is the documented accepted gap).
fn reject_scalar_value_call_arguments(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&State>,
    arguments: &[ExpressionHandle],
    target_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for argument in arguments {
        let ExpressionNode::Call(call) = program.expression_table.expression(*argument) else {
            continue;
        };
        let Some(return_type) = call_return_type(program, current_machine, call) else {
            continue;
        };
        if program.primitive_type_reference(return_type).is_none() {
            continue;
        }
        let state_context = current_state
            .map(|state| format!(" state `{}`", state.name))
            .unwrap_or_default();
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}`{}: transition argument `{}(..)` for `{}` is a value-machine \
             call returning a scalar -- the backend does not materialize a value-call \
             directly into a state argument, so the parameter would silently read 0 \
             (or a paired argument's result). Bind the result to a `let` local first, \
             then pass the local.",
            current_machine.name, state_context, call.target, target_name
        )));
    }
}
