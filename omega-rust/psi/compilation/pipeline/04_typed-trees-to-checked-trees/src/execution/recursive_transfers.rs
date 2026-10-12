//! Constant-stack eligibility of the execution plans this compile just built.
//!
//! Source tail position and ranking do not prove activation reuse. Each common
//! scalar/Unit plan owns its emitted states, including fused foreign states.
//! Retained calls projected into that plan's exact successors stay within its
//! activation. The remaining ordinary calls must form an acyclic graph of
//! published callable roots. This publication floor consumes produced facts
//! and plans, without replaying source admission or inventing instance identity.

use arena::Handle;
use checked_trees::{
    CheckFacts, CheckedComposedUnitControlTerminatorPlan, CheckedScalarBranchDestination,
    CheckedScalarStateTerminator, CheckedStructuralControlSuccessorPlan, FlowCallDispatchRole,
    FlowCallFact, FlowCallSite, FlowStateFact,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::statement::{StatementNode, TransitionTargetHandle};

struct ExecutionEdge {
    source: usize,
    destination: usize,
    state: SymbolHandle,
    call: Handle<FlowCallFact>,
}

#[derive(Clone, Copy)]
enum StateControl<'plan> {
    Composed(&'plan CheckedComposedUnitControlTerminatorPlan),
    Scalar(&'plan CheckedScalarStateTerminator),
    Returning,
}

pub(super) fn require_constant_stack_cycles(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let plans = &facts.flow.terminal_unit_effects;
    let mut machines = Vec::new();
    for machine in plans
        .machines
        .iter()
        .map(|plan| plan.machine)
        .chain(plans.composed_machines.iter().map(|plan| plan.machine))
    {
        if !machines.contains(&machine) {
            machines.push(machine);
        }
    }
    let mut edges = Vec::new();
    // Source-family declarations still receive ordinary source admission.
    // Only published callable roots own activation storage; a composed root
    // must include every state it actually emits, not only its source owner.
    for plan in &plans.machines {
        let control = plan
            .scalar_control
            .as_ref()
            .map(|control| StateControl::Scalar(&control.terminator))
            .unwrap_or(StateControl::Returning);
        retain_state_calls(
            program,
            facts,
            &machines,
            plan.machine,
            plan.state,
            control,
            &mut edges,
        );
    }
    for plan in &plans.composed_machines {
        for state in &plan.states {
            retain_state_calls(
                program,
                facts,
                &machines,
                plan.machine,
                state.state,
                StateControl::Composed(&state.terminator),
                &mut edges,
            );
        }
    }
    let mut adjacency = vec![Vec::new(); machines.len()];
    for edge in &edges {
        adjacency[edge.source].push(edge.destination);
    }
    for targets in &mut adjacency {
        targets.sort_unstable();
        targets.dedup();
    }
    let components = crate::checks::termination::strongly_connected_components(&adjacency);
    let mut diagnostics = Vec::new();
    for edge in edges {
        let recurrent = components.iter().any(|component| {
            component.contains(&edge.source)
                && component.contains(&edge.destination)
                && (component.len() > 1 || edge.source == edge.destination)
        });
        if recurrent {
            let call = facts.flow.control.calls.get(edge.call);
            diagnostics.push(Diagnostic::error(format!(
                "runtime recursive call has no checked constant-stack transfer: {} -> {} at state {} statement {} call {}",
                program.symbols.display_path(machines[edge.source], "::"),
                program.symbols.display_path(machines[edge.destination], "::"),
                program.symbols.display_path(edge.state, "::"),
                call.statement_index, call.call_ordinal,
            )));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

#[allow(clippy::too_many_arguments)]
fn retain_state_calls(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machines: &[SymbolHandle],
    activation: SymbolHandle,
    state_symbol: SymbolHandle,
    control: StateControl<'_>,
    edges: &mut Vec<ExecutionEdge>,
) {
    let Some(source) = machines.iter().position(|symbol| *symbol == activation) else {
        return;
    };
    for (_, state) in facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, state)| state.state_symbol == state_symbol)
    {
        for call_handle in state.calls.handles() {
            let call = facts.flow.control.calls.get(call_handle);
            if facts.flow.control.is_retired(state.state_symbol, call)
                || call.authored_dispatch_role == FlowCallDispatchRole::SavedSubjectRead
            {
                continue;
            }
            if let Some(is_continuation) = transfer_arm(program, facts, state, call_handle, call)
                && successor_matches(facts, control, call, is_continuation)
            {
                continue;
            }
            let Some(callee) = program
                .machine_holding_state(call.target_symbol)
                .or_else(|| program.machine_by_symbol(call.target_symbol))
            else {
                continue;
            };
            // Ordinary calls invoke the callee's published callable root,
            // even when its source state also appears in this fused plan.
            // A state-owner name alone cannot create a callable entry.
            let Some(entry) = facts
                .flow
                .terminal_unit_effects
                .callable_entry_for_machine(callee.symbol)
            else {
                continue;
            };
            let Some(destination) = machines.iter().position(|symbol| *symbol == entry.machine)
            else {
                continue;
            };
            edges.push(ExecutionEdge {
                source,
                destination,
                state: state.state_symbol,
                call: call_handle,
            });
        }
    }
}

fn transfer_arm(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    flow_state: &FlowStateFact,
    call_handle: Handle<FlowCallFact>,
    call: &FlowCallFact,
) -> Option<bool> {
    let machine = program.machine_by_symbol(flow_state.machine_symbol)?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == flow_state.state_symbol)?;
    let target = if call.authored_site == FlowCallSite::NamedTransition {
        validation::transition_call_target(
            program,
            machine,
            state,
            call.statement_index,
            call.call_ordinal,
        )?
    } else {
        let mut transfers = facts
            .flow
            .control
            .tail_transfers
            .iter()
            .map(|(_, transfer)| transfer)
            .filter(|transfer| {
                transfer.machine_symbol == flow_state.machine_symbol
                    && transfer.state_symbol == flow_state.state_symbol
                    && transfer.statement_index == call.statement_index
                    && transfer.call == call_handle
                    && transfer.target_state == call.target_symbol
            });
        let transfer = transfers.next()?;
        if transfers.next().is_some() {
            return None;
        }
        transfer.transition_target
    };
    selected_arm(program, state, call, target)
}

fn selected_arm(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    call: &FlowCallFact,
    target: TransitionTargetHandle,
) -> Option<bool> {
    let statement = program
        .statement_table
        .statement_at(state, call.statement_index)?;
    if !target.is_valid() {
        return matches!(statement, StatementNode::Expression(expression)
            if *expression == call.authored_expression)
        .then_some(false);
    }
    let transition = statement.as_transition()?;
    match (
        transition.target == target,
        transition.continuation == target,
    ) {
        (true, false) => Some(false),
        (false, true) => Some(true),
        // A shared/absent handle does not identify one selected source arm.
        _ => None,
    }
}

fn successor_matches(
    facts: &CheckFacts,
    control: StateControl<'_>,
    call: &FlowCallFact,
    is_continuation: bool,
) -> bool {
    let matches = |successor: &CheckedStructuralControlSuccessorPlan| {
        successor.statement_ordinal as usize == call.statement_index
            && successor.target_state == call.target_symbol
            && successor.is_continuation == is_continuation
    };
    let destination_matches = |destination: &CheckedScalarBranchDestination| matches!(destination, CheckedScalarBranchDestination::Jump(successor) if matches(successor));
    let guarded_matches = |arms, fallback: &Option<CheckedScalarBranchDestination>| {
        facts
            .flow
            .guarded_exits
            .exits
            .span_or_empty(arms)
            .iter()
            .any(|arm| destination_matches(&arm.destination))
            || fallback.as_ref().is_some_and(destination_matches)
    };
    match control {
        StateControl::Composed(CheckedComposedUnitControlTerminatorPlan::Guarded {
            arms,
            fallback,
            ..
        }) => guarded_matches(*arms, fallback),
        StateControl::Composed(terminator) => terminator.successors().into_iter().any(matches),
        StateControl::Scalar(CheckedScalarStateTerminator::Guarded { arms, fallback }) => {
            guarded_matches(*arms, fallback)
        }
        StateControl::Scalar(CheckedScalarStateTerminator::Jump(successor)) => matches(successor),
        StateControl::Scalar(CheckedScalarStateTerminator::Conditional {
            when_true,
            when_false,
            ..
        }) => destination_matches(when_true) || destination_matches(when_false),
        StateControl::Returning | StateControl::Scalar(_) => false,
    }
}
