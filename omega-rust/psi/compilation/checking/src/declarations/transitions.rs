use crate::declarations::symbols::{MachineSymbols, TopLevelSymbols};
use crate::machine_calls::calls::validate_call_arguments;
use crate::proof_contracts::arithmetic_domains::ValueEnvironment;
use crate::value_custody::locals::WritableRoots;
use diagnostics::Diagnostic;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{TransitionTargetHandle, TransitionTargetNode};

mod evaluation;
mod terminal_results;
pub(crate) use evaluation::TransitionValueEnvironments;

/// Resolve a named transfer's exact declaration, retaining its owning machine.
/// Machine declarations select entry; state declarations select that state.
pub(crate) fn resolved_transition_target_state(
    program: &dyn crate::CheckingInput,
    target: symbols::SymbolHandle,
) -> Option<(&Machine, &State)> {
    if !target.is_valid() {
        return None;
    }
    // A machine symbol selects that machine's entry state outright; a state
    // symbol's declaration parent is its machine, so the table answers the
    // owner directly instead of scanning every machine's states.
    if let Some(machine) = program.machine_by_symbol(target) {
        return program
            .machine_states(machine)
            .first()
            .map(|state| (machine, state));
    }
    let machine_symbol = program.symbols.get(target).parent;
    let machine = program.machine_by_symbol(machine_symbol)?;
    program
        .state_in_machine(machine, target)
        .map(|state| (machine, state))
}

/// Named state transfers remain control flow; named machine and requirement
/// targets contribute ordinary call contracts, including static binder bounds.
pub(crate) fn named_transition_call_symbol(
    program: &dyn crate::CheckingInput,
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
                .trait_machine_signature_by_symbol(definition, target)
                .is_some()
        })
    {
        target
    } else {
        symbols::SymbolHandle::invalid()
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_transition_target_node(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    current_state: Option<&State>,
    value_environment: &ValueEnvironment,
    argument_environments: &[ValueEnvironment],
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

    // A named transition on an attached machine is a jump within the current
    // machine: its bare one-member target must resolve to the machine's own
    // entry or one of its states. A one-member target resolving to ANOTHER
    // machine -- the machine declaration or one of its states -- spells a
    // foreign call without the value-call parentheses (`-> (callee(..))`),
    // so reject it rather than repairing it through local-state lookup.
    // Free machines keep the call-edge reading used by measured call
    // components (`outer -> inner(..)`), and receiver-qualified targets
    // (`self.m(..)` or a callable field's `field.state(..)`) keep their
    // nested-receiver admission below.
    if current_machine.attached_data.is_some() {
        let members = program.statement_table.name_path_members(path.members);
        if members.len() == 1
            && (program.machines().iter().any(|candidate| {
                candidate.symbol == path.symbol && candidate.symbol != current_machine.symbol
            }) || program
                .machine_holding_state(path.symbol)
                .is_some_and(|holder| holder.symbol != current_machine.symbol))
        {
            diagnostics.push(Diagnostic::error(format!(
                "unsupported transition target `{}`",
                members[0].as_str()
            )));
            return;
        }
    }

    let arguments = program.statement_table.expression_handles(*arguments);
    crate::proof_contracts::contract_entailment::validate_const_range_call_in_environment(
        program,
        current_machine,
        current_state,
        path.symbol,
        &[],
        arguments,
        Some(value_environment),
        diagnostics,
    );

    // Entry transitions name the machine declaration, whereas named states
    // name their state declaration. Resolve both exact identities before the
    // compatibility spelling paths: a generated `entry` state is not found
    // by looking up the machine's source name in the local state table.
    if let Some((owner, state)) = resolved_transition_target_state(program, path.symbol) {
        // Another machine in target position is a terminal call whose
        // normal result completes this state; a state of this machine,
        // including its re-entered entry, is a jump.
        if owner.symbol != current_machine.symbol
            && let Some(current_state) = current_state
        {
            terminal_results::validate(
                program,
                current_machine,
                current_state,
                owner,
                state,
                diagnostics,
            );
        }
        validate_transition_arguments_handles(
            program,
            current_machine,
            current_state,
            value_environment,
            argument_environments,
            arguments,
            state.name.as_str(),
            program.state_parameters(state),
            writable_roots,
            diagnostics,
        );
        return;
    }

    if let Some(signature) = program.machine_parameter_signature_in(current_machine, path.symbol) {
        validate_call_arguments(
            program,
            current_machine,
            current_state,
            value_environment,
            arguments,
            signature.name.as_str(),
            signature.name.as_str(),
            program.state_signature_parameters(signature),
            writable_roots.statements,
            false,
            argument_environments,
            |argument, required| {
                crate::value_custody::expression_types::argument_matches_carrier(
                    program,
                    current_machine.symbol,
                    argument,
                    required,
                    &[],
                )
            },
            diagnostics,
        );
        return;
    }

    let resolved_symbol = path.symbol;
    let path = program.statement_table.name_path_members(path.members);

    let state_member = if path.len() == 1 {
        Some(0)
    } else if path.len() == 2 && path[0].is_self_receiver() {
        Some(1)
    } else {
        None
    };
    if let Some(member_index) = state_member {
        let Some(state) = machine_symbols.state(path[member_index].as_str()) else {
            // Resolution found no state, machine, parameter or requirement
            // under this spelling. Lowering would otherwise meet an
            // unresolved authored call after checking succeeded.
            if !resolved_symbol.is_valid() {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` transitions to `{}`, which names no state of this machine \
                     and no machine entry or requirement",
                    current_machine.name, path[member_index]
                )));
            }
            return;
        };

        validate_transition_arguments_handles(
            program,
            current_machine,
            current_state,
            value_environment,
            argument_environments,
            arguments,
            state.name.as_str(),
            program.state_parameters(state),
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
            value_environment,
            argument_environments,
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
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    current_state: Option<&State>,
    value_environment: &ValueEnvironment,
    argument_environments: &[ValueEnvironment],
    arguments: &[ExpressionHandle],
    target_name: &str,
    parameters: &[StateParameter],
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_call_arguments(
        program,
        current_machine,
        current_state,
        value_environment,
        arguments,
        target_name,
        target_name,
        parameters,
        writable_roots.statements,
        false,
        argument_environments,
        |argument, required| {
            crate::value_custody::expression_types::argument_matches_carrier(
                program,
                current_machine.symbol,
                argument,
                required,
                &[],
            )
        },
        diagnostics,
    );
}
