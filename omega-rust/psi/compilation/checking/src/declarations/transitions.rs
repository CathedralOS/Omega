use crate::machine_calls::calls::validate_call_arguments;
use crate::proof_contracts::arithmetic_domains::ValueEnvironment;
use crate::value_custody::locals::WritableRoots;
use diagnostics::Diagnostic;
use symbols::JoinedPath;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{TransitionTargetHandle, TransitionTargetNode};

mod evaluation;
mod terminal_results;
pub(crate) use evaluation::TransitionValueEnvironments;

/// Resolve a named transfer's exact declaration, retaining its owning machine.
/// Machine declarations select entry; internal control symbols select their exact body.
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
    writable_roots: &WritableRoots<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
) {
    let target_node = program.statement_table.transition_target(target);
    if let TransitionTargetNode::Value(expression) = target_node {
        // Whole terminal invocations use the ordinary expression call route.
        // Retain their result-agreement diagnostic after normalization; a
        // conversion or enclosing value expression keeps its own type owner.
        if let Some(call) = program.expression_table.call_node(*expression)
            && let Some((owner, state)) =
                resolved_transition_target_state(program, crate::call_target_symbol(program, call))
            && owner.symbol != current_machine.symbol
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
        return;
    }
    let TransitionTargetNode::Named {
        path,
        arguments,
        source_span,
        ..
    } = target_node
    else {
        return;
    };

    // A resolved terminal machine call has the ordinary call contract even
    // inside a data-qualified declaration. Lexical resolution already selects
    // visible nested/sibling targets; qualification does not create a receiver
    // or a separate syntax mode. Preserve argument and completion checks below.
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

    // Source invocations and compiler-owned control edges retain exact
    // selected identities. Generated body coordinates have no authored names.
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
            formation_obligations,
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
            formation_obligations,
        );
        return;
    }

    // Authored names resolve once, before checking. A missing declaration
    // cannot acquire an internal control destination from a matching spelling.
    let members = program.statement_table.name_path_members(path.members);
    diagnostics.push(
        Diagnostic::error(format!(
            "terminal call `{}` does not select a declared machine entry or callable requirement",
            members.joined_path(),
        ))
        .with_source_span(*source_span),
    );
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
    formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
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
        formation_obligations,
    );
}
