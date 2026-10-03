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
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(machine) = crate::lookup::machine_by_symbol(program, state_flow.machine_symbol) else {
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
            let current = crate::checks::current_scalar_values::CurrentScalarValues::at(
                program,
                machine,
                state,
                statement_index,
                target,
                call_frames,
            );
            for requirement in &requirements {
                // Dependency rows only retire their associated proposition on
                // writes; they assert no arrival obligation themselves.
                if matches!(
                    requirement.payload,
                    facts::FactPayload::StorageDependency { .. }
                ) {
                    continue;
                }
                if let facts::FactPayload::ContractBooleanExpression { expression, .. } =
                    requirement.payload
                    && current.as_ref().is_some_and(|current| {
                        current
                            .current(expression)
                            .is_some_and(|goal| current.proves(program, machine, &goal))
                    })
                {
                    continue;
                }

                if let facts::FactPayload::AssignedIntegerBounds { bounds } = requirement.payload
                    && let facts::FactPlace::Place(place) = requirement.place
                    && let Some(current) = &current
                    && facts.semantic.places.is_valid(place)
                    && let place = facts.semantic.places.get(place)
                    && place.segments.is_empty()
                    && let facts::PlaceRoot::Symbol(symbol) = place.root
                    && let Some(value) = current.current_symbol(symbol)
                    && facts.semantic.integer_ranges.is_valid(bounds)
                    && let bounds = facts.semantic.integer_ranges.get(bounds)
                    && current.proves_values_relation(
                        program,
                        machine,
                        typed_trees::expression::BinaryOperator::GreaterOrEqual,
                        &value,
                        &validation::ScopedArithmeticValue::Integer(bounds.minimum.clone()),
                    )
                    && current.proves_values_relation(
                        program,
                        machine,
                        typed_trees::expression::BinaryOperator::LessOrEqual,
                        &value,
                        &validation::ScopedArithmeticValue::Integer(bounds.maximum.clone()),
                    )
                {
                    continue;
                }

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
