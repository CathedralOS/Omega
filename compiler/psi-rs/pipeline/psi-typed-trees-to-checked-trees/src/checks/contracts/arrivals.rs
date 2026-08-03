use psi_checked_trees::FlowStateFact;
use psi_diagnostics::Diagnostic;
use psi_facts::{FactOrigin, ProgramPoint};
use psi_typed_trees::statement::{
    StatementNode, TransitionGuardNode, TransitionTargetHandle, TransitionTargetNode,
};

use super::calls::guard_conjunct_matches;
use super::prover::semantic_contexts_prove_contract_fact;
use crate::labels::semantic_fact_requirement_label;

/// `self` is a real back-edge even though it has no ordinary call arguments.
/// Re-check the declaring state's arrival contract after all preceding
/// mutations; the entry assumption cannot justify itself once invalidated.
pub(super) fn check_self_transition_arrival_requires(
    program: &psi_typed_trees::TypedTrees,
    facts: &psi_checked_trees::CheckFacts,
    state_flow: &FlowStateFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == state_flow.machine_symbol)
    else {
        return;
    };
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_flow.state_symbol)
    else {
        return;
    };
    let requirements = facts
        .semantic
        .contexts_at_point(ProgramPoint::State {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
        })
        .flat_map(|context| context.facts())
        .filter(|fact| {
            matches!(
                fact.origin,
                FactOrigin::StateContract {
                    machine_symbol,
                    state_symbol,
                } if machine_symbol == machine.symbol && state_symbol == state.symbol
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    if requirements.is_empty() {
        return;
    }

    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        let StatementNode::Transition(transition) = statement else {
            continue;
        };
        if !target_is_self(program, transition.target)
            && !(transition.continuation.is_valid()
                && target_is_self(program, transition.continuation))
        {
            continue;
        }

        let entry_constraints = facts
            .flow
            .state_statement(state_flow, statement_index)
            .map(|statement| statement.entry_constraints)
            .unwrap_or(state_flow.entry_constraints);
        let entry_contexts = facts
            .flow
            .semantic_constraint_contexts(entry_constraints)
            .collect::<Vec<_>>();

        for requirement in &requirements {
            let label = semantic_fact_requirement_label(program, &facts.semantic, requirement);
            let guard_proves = match transition.guard {
                TransitionGuardNode::When(guard) => guard_conjunct_matches(program, guard, &label),
                TransitionGuardNode::Always => false,
            };
            if semantic_contexts_prove_contract_fact(
                program,
                &facts.semantic,
                &entry_contexts,
                requirement,
            ) || guard_proves
            {
                continue;
            }
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove state arrival contract on self-transition in {} state `{}` at statement {}: {}",
                machine.name, state.name, statement_index, label
            )));
        }
    }
}

fn target_is_self(program: &psi_typed_trees::TypedTrees, target: TransitionTargetHandle) -> bool {
    matches!(
        program.statement_table.transition_target(target),
        TransitionTargetNode::SelfTarget
    )
}
