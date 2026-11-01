//! Expand the complete storage frontier for one selected control decision.
//! A destination name continues one of its exact arrivals; it is not a new
//! lineage root. Substitute every affected field together and retain the edge
//! so later result or storage expansion cannot combine mutually exclusive arms.

use super::{ExitFrontier, JoinedOccurrence, ReturnFrontierDiagnostics, expand_joined_frontier};
use crate::checks::multiplicity::linear_obligations::{
    CheckedClaimOutcomeMap, CheckedClaimOutcomeSource,
};
use crate::checks::multiplicity::owned_selection::terminal_arm_route;
use crate::checks::ownership_occurrences::{EvaluationBranch, routes_compatible};
use checked_trees::{
    FlowClaimJoinExit, FlowClaimJoinExitKind, FlowPermissionEventFact, FlowSelectionPlaceBinding,
};
use symbols::SymbolHandle;

pub(super) fn selection_routes_agree(
    program: &dyn validation::CheckingInput,
    exits: &[FlowClaimJoinExit],
    state: SymbolHandle,
    statement: usize,
    route: &[EvaluationBranch],
) -> Option<bool> {
    for exit in exits
        .iter()
        .filter(|exit| exit.state_symbol == state && exit.statement_index == statement)
    {
        let previous = match exit.kind {
            FlowClaimJoinExitKind::SelectionArm { expression, arm } => {
                terminal_arm_route(program, expression, arm)?
            }
            FlowClaimJoinExitKind::SelectionDecision {
                expression,
                decision,
                selected,
                ..
            } => {
                vec![EvaluationBranch::new(expression, decision, selected)]
            }
            _ => continue,
        };
        if !routes_compatible(route, &previous) {
            return Some(false);
        }
    }
    Some(true)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn expand_storage_frontier(
    program: &dyn validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    state: &typed_trees::state::State,
    events: &[FlowPermissionEventFact],
    maps: &[CheckedClaimOutcomeMap],
    active: &mut Vec<SymbolHandle>,
    visited: &[JoinedOccurrence],
    frontier: ExitFrontier,
    binding: &FlowSelectionPlaceBinding,
    diagnostics: &mut ReturnFrontierDiagnostics,
) -> Option<Vec<ExitFrontier>> {
    let ownership = &facts.flow.ownership;
    let occurrence = JoinedOccurrence::Storage {
        state: binding.state,
        statement: binding.statement_ordinal,
        evaluation: binding.evaluation,
        expression: binding.expression,
        decision: binding.decision,
    };
    if state.symbol != binding.state
        || visited.contains(&occurrence)
        || !crate::checks::multiplicity::selected_evaluation::place_bindings::claim_origin(
            ownership,
            binding.destination.claim_identity,
            binding.destination.provenance,
        )
    {
        return None;
    }
    let rows = ownership
        .selection_place_bindings
        .iter()
        .filter_map(|(_, row)| {
            (row.machine == binding.machine
                && row.state == binding.state
                && row.statement_ordinal == binding.statement_ordinal
                && row.evaluation == binding.evaluation
                && row.expression == binding.expression
                && row.decision == binding.decision)
                .then_some(row)
        })
        .collect::<Vec<_>>();
    let mut visited = visited.to_vec();
    visited.push(occurrence);
    let mut expanded = Vec::new();
    for selected in [false, true] {
        let route = [EvaluationBranch::new(
            binding.expression,
            binding.decision,
            selected,
        )];
        if !selection_routes_agree(
            program,
            &frontier.exits,
            binding.state,
            binding.statement_ordinal,
            &route,
        )? {
            continue;
        }
        let mut next = frontier.clone();
        next.exits.push(FlowClaimJoinExit {
            state_symbol: binding.state,
            statement_index: binding.statement_ordinal,
            kind: FlowClaimJoinExitKind::SelectionDecision {
                evaluation: binding.evaluation,
                expression: binding.expression,
                decision: binding.decision,
                selected,
            },
        });
        for entry in &mut next.entries {
            let CheckedClaimOutcomeSource::Established {
                claim_identity,
                provenance,
            } = entry.source
            else {
                continue;
            };
            let mut candidates = rows.iter().filter(|row| {
                row.selected != row.remaining
                    && row.destination.claim_identity == claim_identity
                    && row.destination.provenance == provenance
            });
            let Some(row) = candidates.next() else {
                continue;
            };
            if candidates.next().is_some() {
                return None;
            }
            let arrival = if selected {
                row.selected
            } else {
                row.remaining
            };
            entry.source = super::super::claim_outcome_source_for_identity(
                program,
                state,
                arrival.claim_identity,
                arrival.provenance,
                &ownership.segments,
                events,
            );
        }
        expanded.extend(expand_joined_frontier(
            program,
            facts,
            state,
            events,
            maps,
            active,
            &visited,
            next,
            diagnostics,
        )?);
    }
    (!expanded.is_empty()).then_some(expanded)
}
