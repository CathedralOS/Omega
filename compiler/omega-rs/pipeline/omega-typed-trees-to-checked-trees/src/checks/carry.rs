use omega_core::diagnostics::Diagnostic;
use omega_core::semantics::CarrySuspension;

/// Reject a call that may suspend while a suspension-forbidden lexical value
/// remains live in the caller activation. This is deliberately a local check:
/// CPU/thread/address demands join provider runtime behavior at admission,
/// while suspension joins the value policy with the callee's inferred effect
/// reach here.
pub(super) fn check_suspension_carry(
    program: &omega_typed_trees::TypedTrees,
    facts: &omega_checked_trees::CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let suspend = omega_effects::EffectSet::from_name("Suspend")
        .expect("Suspend is a canonical operational effect");
    let mut diagnostics = Vec::new();

    for state_borrows in facts.borrow.states.iter().map(|(_, state)| state) {
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == state_borrows.machine_symbol)
        else {
            continue;
        };
        let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_borrows.state_symbol)
        else {
            continue;
        };
        let Some(machine_effects) = facts
            .effects
            .machines()
            .iter()
            .find(|effects| effects.symbol == machine.symbol)
        else {
            continue;
        };
        let Some(state_effects) = facts
            .effects
            .states
            .span_or_empty(machine_effects.states)
            .iter()
            .find(|effects| effects.symbol == state.symbol)
        else {
            continue;
        };

        for call in facts.borrow.calls.span_or_empty(state_borrows.calls) {
            let Some(call_effects) = facts
                .effects
                .calls
                .span_or_empty(state_effects.calls)
                .iter()
                .find(|effects| {
                    effects.statement_index == call.statement_index
                        && effects.call_ordinal == call.call_ordinal
                })
            else {
                continue;
            };
            if !call_effects.direct.intersects(suspend)
                && !call_effects.transitive.intersects(suspend)
            {
                continue;
            }

            append_live_parameter_diagnostics(program, machine, state, call, &mut diagnostics);
            append_live_local_diagnostics(program, machine, state, call, &mut diagnostics);
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn append_live_parameter_diagnostics(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    call: &omega_checked_trees::BorrowCallFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for parameter in program.state_parameters(state) {
        if parameter.is_self
            || !crate::borrow::place_is_used_after_statement(
                program,
                state.statement_nodes,
                call.statement_index,
                parameter.symbol,
                parameter.name.as_str(),
            )
        {
            continue;
        }
        append_if_suspension_forbidden(
            program,
            machine,
            parameter.type_reference,
            parameter.name.as_str(),
            call,
            diagnostics,
        );
    }
}

fn append_live_local_diagnostics(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    call: &omega_checked_trees::BorrowCallFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (definition_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        if definition_index >= call.statement_index {
            break;
        }
        let omega_typed_trees::statement::StatementNode::LocalData(local) = statement else {
            continue;
        };
        if !crate::borrow::place_is_used_after_statement(
            program,
            state.statement_nodes,
            call.statement_index,
            local.symbol,
            local.name.as_str(),
        ) {
            continue;
        }
        append_if_suspension_forbidden(
            program,
            machine,
            local.type_reference,
            local.name.as_str(),
            call,
            diagnostics,
        );
    }
}

fn append_if_suspension_forbidden(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
    value_name: &str,
    call: &omega_checked_trees::BorrowCallFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let policy = omega_validation::effective_type_carry_policy(
        program,
        program.machine_type_parameters(machine),
        type_reference,
    );
    if policy.suspension == CarrySuspension::Allowed {
        return;
    }

    let target_name = crate::labels::symbol_name(program, call.target_symbol);
    diagnostics.push(Diagnostic::error(format!(
        "call to `{target_name}` may reach `Suspend` while `{value_name}` remains live, but its effective policy is `{policy}`; consume the value before the call or use a suspension-safe carrier"
    )));
}
