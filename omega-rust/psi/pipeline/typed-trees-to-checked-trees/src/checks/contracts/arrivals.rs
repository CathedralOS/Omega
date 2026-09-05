use checked_trees::FlowStateFact;
use diagnostics::Diagnostic;
use facts::{FactOrigin, ProgramPoint};
use typed_trees::statement::{StatementNode, TransitionTargetHandle, TransitionTargetNode};

use super::prover::semantic_contexts_prove_contract_fact;
use crate::labels::semantic_fact_requirement_label;

/// `self` is a real back-edge even though it has no ordinary call arguments.
/// Re-check the declaring state's arrival contract after all preceding
/// mutations; the entry assumption cannot justify itself once invalidated.
pub(super) fn check_self_transition_arrival_requires(
    program: &typed_trees::TypedTrees,
    facts: &checked_trees::CheckFacts,
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
    let is_entry = program
        .machine_states(machine)
        .first()
        .is_some_and(|entry| entry.symbol == state.symbol);
    let mut requirements = facts
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
            ) || matches!(
                fact.origin,
                FactOrigin::MachineContract { machine_symbol }
                    if is_entry && machine_symbol == machine.symbol
            ) || matches!(fact.origin, FactOrigin::StateParameterDomain { .. })
        })
        .cloned()
        .collect::<Vec<_>>();
    requirements.extend(
        facts.semantic.contexts_at_point(ProgramPoint::Machine { machine_symbol: machine.symbol })
            .flat_map(|context| context.facts())
            .filter(|fact| matches!(fact.origin, FactOrigin::MachineFieldDomain { machine_symbol } if machine_symbol == machine.symbol))
            .cloned(),
    );
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
        for target in [transition.target, transition.continuation] {
            if !target.is_valid() || !target_is_self(program, target) {
                continue;
            }
            let point = ProgramPoint::TransitionArm {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                statement_index,
                transition_target: target,
            };
            let entry_contexts = facts
                .semantic
                .contexts
                .iter()
                .filter_map(|(handle, context)| (context.point == point).then_some(handle))
                .collect::<Vec<_>>();
            for requirement in &requirements {
                if semantic_contexts_prove_contract_fact(
                    program,
                    &facts.semantic,
                    &entry_contexts,
                    requirement,
                ) {
                    continue;
                }
                let label = semantic_fact_requirement_label(program, &facts.semantic, requirement);
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove state arrival contract on self-transition in {} state `{}` at statement {}: {}",
                    machine.name, state.name, statement_index, label
                )));
            }
        }
    }
}

fn target_is_self(program: &typed_trees::TypedTrees, target: TransitionTargetHandle) -> bool {
    matches!(
        program.statement_table.transition_target(target),
        TransitionTargetNode::SelfTarget
    )
}
