use omega_core::diagnostics::Diagnostic;
use omega_core::semantics::CarrySuspension;

mod intra_statement;

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

            let call_site = crate::find_call_site(
                program,
                machine.symbol,
                state.symbol,
                call.statement_index,
                call.call_ordinal,
            );

            append_call_carried_argument_diagnostics(
                program,
                call,
                call_site.as_ref(),
                &mut diagnostics,
            );
            append_live_persistent_diagnostics(
                program,
                machine,
                state,
                call,
                call_site.as_ref(),
                &mut diagnostics,
            );
            append_live_parameter_diagnostics(
                program,
                machine,
                state,
                call,
                call_site.as_ref(),
                &mut diagnostics,
            );
            append_live_local_diagnostics(
                program,
                machine,
                state,
                call,
                call_site.as_ref(),
                &mut diagnostics,
            );
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn append_call_carried_argument_diagnostics(
    program: &omega_typed_trees::TypedTrees,
    call: &omega_checked_trees::BorrowCallFact,
    call_site: Option<&crate::CallSite<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(call_site) = call_site else {
        return;
    };
    let Some(parameters) = crate::call_target_parameters(program, call.target_symbol) else {
        return;
    };
    let arguments = crate::call_site_argument_expressions(program, call_site);
    for (parameter, argument) in parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .zip(arguments)
    {
        let display_name = program.expression_table.display_name(*argument);
        append_if_suspension_forbidden_with_type_parameters(
            program,
            crate::call_target_type_parameters(program, call.target_symbol),
            parameter.type_reference,
            &display_name,
            call,
            diagnostics,
        );
    }
}

fn append_live_persistent_diagnostics(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    call: &omega_checked_trees::BorrowCallFact,
    call_site: Option<&crate::CallSite<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(attached_name) = machine.attached_data.as_ref()
        && let Some(attached) = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name == *attached_name)
    {
        for member in program.data_members(attached) {
            match member {
                omega_typed_trees::data::DataMember::Field(field) => {
                    append_persistent_field_if_live(
                        program,
                        machine,
                        state,
                        call,
                        call_site,
                        field.symbol,
                        field.type_reference,
                        field.name.as_str(),
                        diagnostics,
                    );
                }
                omega_typed_trees::data::DataMember::Variant(variant) => {
                    for field in program.data_payload_fields(variant) {
                        append_persistent_field_if_live(
                            program,
                            machine,
                            state,
                            call,
                            call_site,
                            field.symbol,
                            field.type_reference,
                            field.name.as_str(),
                            diagnostics,
                        );
                    }
                }
            }
        }
    }

    for owned in program.machine_owned_data(machine) {
        append_persistent_field_if_live(
            program,
            machine,
            state,
            call,
            call_site,
            owned.symbol,
            owned.type_reference,
            owned.name.as_str(),
            diagnostics,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn append_persistent_field_if_live(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    call: &omega_checked_trees::BorrowCallFact,
    call_site: Option<&crate::CallSite<'_>>,
    field_symbol: omega_core::symbols::SymbolHandle,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
    field_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !persistent_symbol_is_live_after_call(
        program,
        machine,
        state,
        call,
        call_site,
        field_symbol,
        field_name,
    ) {
        return;
    }
    let display_name = format!("self.{field_name}");
    append_if_suspension_forbidden(
        program,
        machine,
        type_reference,
        &display_name,
        call,
        diagnostics,
    );
}

fn persistent_symbol_is_live_after_call(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    call: &omega_checked_trees::BorrowCallFact,
    call_site: Option<&crate::CallSite<'_>>,
    field_symbol: omega_core::symbols::SymbolHandle,
    field_name: &str,
) -> bool {
    if call_site.is_some_and(|call_site| {
        intra_statement::place_is_used_after_call(
            program,
            state,
            call.statement_index,
            call_site,
            field_symbol,
            field_name,
        )
    }) {
        return true;
    }
    if crate::borrow::place_symbol_is_used_after_statement(
        program,
        state.symbol,
        state.statement_nodes,
        call.statement_index,
        field_symbol,
    ) {
        return true;
    }

    let mut pending = Vec::new();
    append_state_successors_after_statement(
        program,
        machine,
        state,
        call.statement_index,
        &mut pending,
    );
    let mut visited = Vec::new();
    while let Some(state_symbol) = pending.pop() {
        if visited.contains(&state_symbol) {
            continue;
        }
        visited.push(state_symbol);
        let Some(reachable) = program
            .machine_states(machine)
            .iter()
            .find(|candidate| candidate.symbol == state_symbol)
        else {
            continue;
        };
        if crate::borrow::place_symbol_is_used_in_state(program, reachable, field_symbol) {
            return true;
        }
        append_all_state_successors(program, machine, reachable, &mut pending);
    }
    false
}

fn append_state_successors_after_statement(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    successors: &mut Vec<omega_core::symbols::SymbolHandle>,
) {
    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .skip(statement_index)
    {
        append_statement_successors(program, machine, state, statement, successors);
    }
}

fn append_all_state_successors(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    successors: &mut Vec<omega_core::symbols::SymbolHandle>,
) {
    for statement in program.statement_table.statements(state.statement_nodes) {
        append_statement_successors(program, machine, state, statement, successors);
    }
}

fn append_statement_successors(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    statement: &omega_typed_trees::statement::StatementNode,
    successors: &mut Vec<omega_core::symbols::SymbolHandle>,
) {
    let omega_typed_trees::statement::StatementNode::Transition(transition) = statement else {
        return;
    };
    append_transition_target_successor(program, machine, state, transition.target, successors);
    if transition.continuation.is_valid() {
        append_transition_target_successor(
            program,
            machine,
            state,
            transition.continuation,
            successors,
        );
    }
}

fn append_transition_target_successor(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    target: omega_typed_trees::statement::TransitionTargetHandle,
    successors: &mut Vec<omega_core::symbols::SymbolHandle>,
) {
    let symbol = match program.statement_table.transition_target(target) {
        omega_typed_trees::statement::TransitionTargetNode::Named { path, .. } => path.symbol,
        omega_typed_trees::statement::TransitionTargetNode::SelfTarget => state.symbol,
        omega_typed_trees::statement::TransitionTargetNode::Value(_)
        | omega_typed_trees::statement::TransitionTargetNode::Terminal => return,
    };
    if symbol.is_valid()
        && program
            .machine_states(machine)
            .iter()
            .any(|candidate| candidate.symbol == symbol)
    {
        successors.push(symbol);
    }
}

fn append_live_parameter_diagnostics(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    call: &omega_checked_trees::BorrowCallFact,
    call_site: Option<&crate::CallSite<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for parameter in program.state_parameters(state) {
        if parameter.is_self
            || (!crate::borrow::place_is_used_after_statement(
                program,
                state.statement_nodes,
                call.statement_index,
                parameter.symbol,
                parameter.name.as_str(),
            ) && !call_site.is_some_and(|call_site| {
                intra_statement::place_is_used_after_call(
                    program,
                    state,
                    call.statement_index,
                    call_site,
                    parameter.symbol,
                    parameter.name.as_str(),
                )
            }))
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
    call_site: Option<&crate::CallSite<'_>>,
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
        ) && !call_site.is_some_and(|call_site| {
            intra_statement::place_is_used_after_call(
                program,
                state,
                call.statement_index,
                call_site,
                local.symbol,
                local.name.as_str(),
            )
        }) {
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
    append_if_suspension_forbidden_with_type_parameters(
        program,
        program.machine_type_parameters(machine),
        type_reference,
        value_name,
        call,
        diagnostics,
    );
}

fn append_if_suspension_forbidden_with_type_parameters(
    program: &omega_typed_trees::TypedTrees,
    type_parameters: &[omega_typed_trees::data::TypeParameter],
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
    value_name: &str,
    call: &omega_checked_trees::BorrowCallFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let policy =
        omega_validation::effective_type_carry_policy(program, type_parameters, type_reference);
    if policy.suspension == CarrySuspension::Allowed {
        return;
    }

    let target_name = crate::labels::symbol_name(program, call.target_symbol);
    let message = format!(
        "call to `{target_name}` may reach `Suspend` while `{value_name}` remains live, but its effective policy is `{policy}`; consume the value before the call or use a suspension-safe carrier"
    );
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message == message)
    {
        return;
    }
    diagnostics.push(Diagnostic::error(message));
}
