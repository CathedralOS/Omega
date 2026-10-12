//! Publish preservation only from the original bounds decision and the
//! caller-relative, alias-closed footprints of the actual selected death hooks.
use checked_trees::{CheckFacts, FlowClaimJoinExitKind, ScalarIndexExitPreservation};
use typed_trees::statement::StatementNode;

pub(super) fn record(program: &dyn validation::CheckingInput, facts: &mut CheckFacts) {
    let mut rows = arena::Arena::<ScalarIndexExitPreservation>::default();
    for (_, source_exit) in facts.flow.control.exits.iter() {
        let Some(state) = program.state_by_symbol(source_exit.state_symbol) else {
            continue;
        };
        let Some(statement) = program
            .statement_table
            .statements(state.statement_nodes)
            .get(source_exit.statement_index)
        else {
            continue;
        };
        let kind = match statement {
            StatementNode::Expression(_) if !source_exit.transition_target.is_valid() => {
                FlowClaimJoinExitKind::FinalExpression
            }
            StatementNode::Transition(transition)
                if transition.target == source_exit.transition_target =>
            {
                FlowClaimJoinExitKind::TransitionTarget
            }
            StatementNode::Transition(transition)
                if transition.continuation.is_valid()
                    && transition.continuation == source_exit.transition_target =>
            {
                FlowClaimJoinExitKind::TransitionContinuation
            }
            _ => continue,
        };
        let exit = checked_trees::FlowClaimJoinExit {
            state_symbol: source_exit.state_symbol,
            statement_index: source_exit.statement_index,
            kind,
        };
        for (_, admission) in facts.scalar_index_admissions.iter() {
            if admission.machine != source_exit.machine_symbol
                || admission.state != source_exit.state_symbol
                || admission.statement_index != source_exit.statement_index
                || !admission.dependencies_complete
            {
                continue;
            }
            let siblings = facts.scalar_index_admissions.iter().filter(|(_, row)| {
                row.occurrence == admission.occurrence
                    && row.machine == admission.machine
                    && row.state == admission.state
                    && row.statement_index == admission.statement_index
            });
            let preserved = siblings.into_iter().all(|(_, row)| {
                if !row.dependencies_complete {
                    return false;
                }
                let Some(dependencies) = facts
                    .scalar_index_dependency_places
                    .span(row.extent_dependencies)
                else {
                    return false;
                };
                let mut reads = Vec::with_capacity(dependencies.len());
                for dependency in dependencies {
                    let Some(segments) = facts
                        .scalar_index_dependency_segments
                        .span(dependency.segments)
                    else {
                        return false;
                    };
                    reads.push(validation::CanonicalPlace {
                        root: dependency.root,
                        segments: segments.to_vec(),
                    });
                }
                super::cleanup_preservation::reads_survive(
                    program,
                    facts,
                    admission.machine,
                    exit,
                    &reads,
                )
            });
            if preserved
                && !rows.iter().any(|(_, row)| {
                    row.occurrence == admission.occurrence
                        && row.machine == admission.machine
                        && row.exit == exit
                })
            {
                rows.insert(ScalarIndexExitPreservation {
                    occurrence: admission.occurrence,
                    machine: admission.machine,
                    exit,
                });
            }
        }
    }
    facts.scalar_index_exit_preservations = rows;
}
