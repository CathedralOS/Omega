use crate::validation::declarations::symbols::{MachineSymbols, TopLevelSymbols};
use crate::validation::machine_calls::calls::validate_call_arguments_handles_with_policy_retention;
use crate::validation::proof_contracts::arithmetic_domains::ValueEnvironment;
use crate::validation::value_custody::locals::WritableRoots;
use diagnostics::Diagnostic;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle;
use symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine;
use symbol_resolved_trees_to_typed_trees::typed_trees::signature::StateParameter;
use symbol_resolved_trees_to_typed_trees::typed_trees::state::State;
use symbol_resolved_trees_to_typed_trees::typed_trees::statement::{
    TransitionTargetHandle, TransitionTargetNode,
};

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
    // A machine symbol selects that machine's entry state outright; a state
    // symbol's declaration parent is its machine, so the table answers the
    // owner directly instead of scanning every machine's states.
    if let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == target)
    {
        return program
            .machine_states(machine)
            .first()
            .map(|state| (machine, state));
    }
    let machine_symbol = program.symbols.get(target).parent;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == target)
        .map(|state| (machine, state))
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
    crate::validation::proof_contracts::contract_entailment::validate_const_range_call_in_environment(
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
    if let Some((_, state)) = resolved_transition_target_state(program, path.symbol) {
        validate_transition_arguments_handles(
            program,
            current_machine,
            current_state,
            value_environment,
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
            value_environment,
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
            value_environment,
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

    if path.len() == 2 && path[0].is_self_receiver() {
        let Some(state) = machine_symbols.state(path[1].as_str()) else {
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
            value_environment,
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
    value_environment: &ValueEnvironment,
    argument_environments: &[ValueEnvironment],
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
        value_environment,
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
