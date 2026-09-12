use crate::arithmetic_domains::ValueEnv;
use crate::calls::validate_call_arguments_handles_with_policy_retention;
use crate::locals::WritableRoots;
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{TransitionTargetHandle, TransitionTargetNode};

mod evaluation;
pub(crate) use evaluation::TransitionValueEnvironments;

/// Resolve a named transfer's exact declaration, retaining its owning machine.
/// Machine declarations select entry; state declarations select that state.
pub(crate) fn resolved_transition_target_state(
    program: &TypedTrees,
    target: symbols::SymbolHandle,
) -> Option<(&Machine, &State)> {
    if !target.is_valid() {
        return None;
    }
    program.machines().iter().find_map(|machine| {
        let states = program.machine_states(machine);
        let state = if machine.symbol == target {
            states.first()
        } else {
            states.iter().find(|state| state.symbol == target)
        }?;
        Some((machine, state))
    })
}

/// Named state transfers remain control flow; named machine and requirement
/// targets contribute ordinary call contracts, including static binder bounds.
pub(crate) fn named_transition_call_symbol(
    program: &TypedTrees,
    caller: symbols::SymbolHandle,
    target: symbols::SymbolHandle,
) -> symbols::SymbolHandle {
    if let Some((owner, state)) = resolved_transition_target_state(program, target) {
        return if owner.symbol != caller || target == owner.symbol {
            state.symbol
        } else {
            symbols::SymbolHandle::invalid()
        };
    }
    if program.machine_parameter_signature(target).is_some()
        || program.traits().iter().any(|definition| {
            program
                .trait_machine_signatures(definition)
                .iter()
                .any(|signature| signature.symbol == target)
        })
    {
        target
    } else {
        symbols::SymbolHandle::invalid()
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_transition_target_node(
    program: &TypedTrees,
    current_machine: &Machine,
    current_state: Option<&State>,
    value_env: &ValueEnv,
    argument_environments: &[ValueEnv],
    target: TransitionTargetHandle,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(target)
    else {
        return;
    };

    let arguments = program.statement_table.expression_handles(*arguments);

    // Entry transitions name the machine declaration, whereas named states
    // name their state declaration. Resolve both exact identities before the
    // compatibility spelling paths: a generated `entry` state is not found
    // by looking up the machine's source name in the local state table.
    if let Some((_, state)) = resolved_transition_target_state(program, path.symbol) {
        validate_transition_arguments_handles(
            program,
            current_machine,
            current_state,
            value_env,
            argument_environments,
            arguments,
            state.name.as_str(),
            program.state_parameters(state),
            state,
            writable_roots,
            diagnostics,
        );
        return;
    }

    if let Some(signature) = program.machine_parameter_signature_in(current_machine, path.symbol) {
        validate_call_arguments_handles_with_policy_retention(
            program,
            current_machine,
            current_state,
            value_env,
            arguments,
            signature.name.as_str(),
            program.state_signature_parameters(signature),
            None,
            writable_roots,
            false,
            argument_environments,
            diagnostics,
        );
        return;
    }

    let path = program.statement_table.name_path_members(path.members);

    if path.len() == 1 {
        let Some(state) = machine_symbols.state(path[0].as_str()) else {
            return;
        };

        validate_transition_arguments_handles(
            program,
            current_machine,
            current_state,
            value_env,
            argument_environments,
            arguments,
            state.name.as_str(),
            program.state_parameters(state),
            state,
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
            argument_environments,
            arguments,
            state.name.as_str(),
            program.state_parameters(state),
            state,
            writable_roots,
            diagnostics,
        );
        return;
    }

    let Some(receiver_type) = machine_symbols.callable_field_type(path[0].as_str()) else {
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
            argument_environments,
            arguments,
            &state.name,
            program.state_parameters(state),
            state,
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
    argument_environments: &[ValueEnv],
    arguments: &[ExpressionHandle],
    target_name: &str,
    parameters: &[StateParameter],
    callee_state: &State,
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_call_arguments_handles_with_policy_retention(
        program,
        current_machine,
        current_state,
        value_env,
        arguments,
        target_name,
        parameters,
        Some(callee_state),
        writable_roots,
        false,
        argument_environments,
        diagnostics,
    );
}
