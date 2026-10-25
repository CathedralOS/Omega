//! A completed observation owns the normal death of its anonymous receiver,
//! unless its actual result retains that home. This selects a completion only;
//! the occurrence producer still disposes the exact RemainingCustody complement
//! with ordinary claim receipts, before disposing its backing dependencies.
use super::{SourceCompletion, StatementOccurrence, StatementStep};
use crate::checks::multiplicity::temporary_results::{self, TemporaryOwner};
use checked_trees::{CheckFacts, FlowPermissionEvaluation};
use symbols::SymbolHandle;

/// Commit source death after the value's own consuming moves, before changing
/// routes. A produced value and its receiving transfer are distinct occurrences.
#[allow(clippy::too_many_arguments)]
pub(super) fn complete_at(
    program: &dyn validation::CheckingInput,
    facts: &mut CheckFacts,
    completions: &[SourceCompletion],
    occurrences: &[StatementOccurrence],
    moves: &[crate::flow::DiscoveredMoveEvent],
    temporary_owners: &[TemporaryOwner],
    occurrence: usize,
    site: crate::checks::multiplicity::selected_evaluation::place_bindings::SelectionSite,
    places: &mut [crate::checks::multiplicity::linear_obligations::LinearPlace],
    remaining: &mut crate::checks::multiplicity::cleanup_dispositions::RemainingCustody,
    permission_events: &mut Vec<checked_trees::FlowPermissionEventFact>,
    cleanup_dispositions: &mut checked_trees::ContextualCleanupFacts,
) -> Result<(), diagnostics::Diagnostic> {
    use checked_trees::FlowPermissionEventFact;
    use language_semantics::{
        Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
        PermissionEventSource, PermissionProvenance,
    };
    let machine_symbol = site.machine;
    let state_symbol = site.state;
    let evaluation = site.evaluation;
    let statement_source = PermissionEventSource::Statement {
        statement_index: site.statement,
    };
    for completion in completions
        .iter()
        .filter(|completion| completion.occurrence == occurrence)
    {
        let expression = completion.expression;
        let root = completion.root;
        if !remaining.is_present(root, &[])
            || !places
                .iter()
                .any(|place| place.root == root && place.ever_established)
        {
            continue;
        }
        let completion_start = permission_events.len();
        crate::checks::multiplicity::linear_validation::affine_cleanup::append_owned_region_cleanup_permission_events(
            program,
            completion.type_reference,
            root,
            &[],
            places,
            remaining,
            FlowPermissionEventFact {
                expression,
                evaluation,
                machine_symbol,
                state_symbol,
                source: statement_source,
                kind: PermissionEventKind::AffineDrop,
                multiplicity: Multiplicity::Affine,
                access: PermissionAccess::Owned,
                claim_identity: PermissionClaimIdentity::Unknown,
                provenance: PermissionProvenance::Unknown,
                root,
                segments: Default::default(),
                obligation_live: false,
            },
            &mut facts.flow.ownership.segments,
            permission_events,
        );
        if permission_events.len() == completion_start {
            return Err(diagnostics::Diagnostic::error(
                "observed temporary source has no authorized owned completion",
            )
            .with_source_span(program.expression_table.source_span(expression)));
        }
        let disposition_start = cleanup_dispositions.dispositions.len();
        remaining.observe_ordinary(
            program,
            &permission_events[completion_start..],
            &facts.flow.ownership.segments,
            cleanup_dispositions,
            checked_trees::FlowClaimJoinExit::default(),
            &[],
            false,
        );
        let mut disposition_ordinal = 0;
        cleanup_dispositions
            .dispositions
            .for_each_mut(|_, disposition| {
                if disposition_ordinal >= disposition_start {
                    disposition.completion = expression;
                    disposition.completion_constraints = completion.constraints;
                }
                disposition_ordinal += 1;
            });
        for place in places.iter_mut().filter(|place| place.root == root) {
            place.live = false;
        }
        remaining.retire_storage(
            root,
            machine_symbol,
            state_symbol,
            statement_source,
            cleanup_dispositions,
        );
    }

    let expression = match occurrences[occurrence].step {
        StatementStep::Complete(expression) => expression,
        StatementStep::Move { event, .. } => moves[event].expression,
        _ => return Ok(()),
    };
    if completion_cut(occurrences, moves, expression) != Some(occurrence) {
        return Ok(());
    }
    let completion_start = permission_events.len();
    let retired = temporary_results::append_continuation_cleanup(
        program,
        facts,
        machine_symbol,
        state_symbol,
        statement_source,
        expression,
        evaluation,
        temporary_owners,
        places,
        remaining,
        permission_events,
        cleanup_dispositions,
    )?;
    remaining.observe_ordinary(
        program,
        &permission_events[completion_start..],
        &facts.flow.ownership.segments,
        cleanup_dispositions,
        checked_trees::FlowClaimJoinExit::default(),
        &[],
        false,
    );
    for root in retired {
        remaining.retire_storage(
            root,
            machine_symbol,
            state_symbol,
            statement_source,
            cleanup_dispositions,
        );
    }
    Ok(())
}

pub(super) fn append(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    evaluation: FlowPermissionEvaluation,
    moves: &[crate::flow::DiscoveredMoveEvent],
    occurrences: &[StatementOccurrence],
    temporary_owners: &[TemporaryOwner],
    completions: &mut Vec<SourceCompletion>,
) {
    let Some(flow_state) = facts.flow.control.state(machine, state) else {
        return;
    };
    let Some(statement) = facts.flow.state_statement(flow_state, statement_index) else {
        return;
    };
    let observations = facts
        .flow
        .control
        .value_observations
        .span_or_empty(statement.value_observations);
    for observation in observations {
        if observation.evaluation != evaluation
            || !facts.flow.control.operands.is_valid(observation.receiver)
        {
            continue;
        }
        let receiver = facts.flow.control.operands.get(observation.receiver);
        let Some(place) = validation::canonical_place_from_expression_in_state(
            program,
            state,
            statement_index,
            receiver.expression,
        ) else {
            continue;
        };
        let Some(source) = place.root.as_expression() else {
            continue;
        };
        let mut ends = occurrences.iter().enumerate().filter(|(_, occurrence)| {
            matches!(occurrence.step, StatementStep::Complete(completed) if completed == observation.expression)
        });
        let Some((end, _)) = ends.next() else {
            continue;
        };
        if ends.next().is_some() {
            continue;
        }
        // A nested field read is not the last consumer while an enclosing
        // observation still holds this receiver. Select its latest actual
        // completion even if the enclosing result lacks retention evidence;
        // missing evidence must keep custody, never authorize an earlier death.
        if observations.iter().any(|later| {
            later.evaluation == evaluation
                && facts.flow.control.operands.is_valid(later.receiver)
                && validation::canonical_place_from_expression_in_state(
                    program, state, statement_index,
                    facts.flow.control.operands.get(later.receiver).expression,
                ).is_some_and(|later_place| later_place.root == place.root)
                && occurrences[end + 1..].iter().any(|occurrence| {
                    matches!(occurrence.step, StatementStep::Complete(completed) if completed == later.expression)
                })
        }) { continue; }
        completions.retain(|candidate| candidate.root != place.root);
        if temporary_owners
            .iter()
            .any(|owner| owner.expression == source)
            || !facts.flow.control.operands.is_valid(observation.result)
        {
            continue;
        }
        let result = facts.flow.control.operands.get(observation.result);
        let Some(retained) =
            crate::flow::captured_lifetime_roots(program, &facts.flow.control, result)
        else {
            continue;
        };
        if retained.contains(&place.root) {
            continue;
        }
        let mut producers = occurrences[..end].iter().enumerate().filter(|(_, occurrence)| {
            matches!(occurrence.step, StatementStep::Complete(completed) if completed == source)
        });
        let Some((producer_ordinal, producer)) = producers.next() else {
            continue;
        };
        if producers.next().is_some() {
            continue;
        }
        let Some(type_reference) = validation::canonical_place_type_reference(
            program,
            state,
            statement_index,
            &validation::CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            },
        ) else {
            continue;
        };
        if validation::reference_shell(program, type_reference).is_some()
            || validation::reference_result_custody::binding_multiplicity(program, type_reference)
                == language_semantics::Multiplicity::Unrestricted
        {
            continue;
        }
        // The joined frontier has already forgotten custody absent on one
        // alternative. End a dying receiver at each actual result completion,
        // after its transfers and before that meet. No predecessor storage or
        // syntax reconstruction may stand in for a retained result capture.
        let Some(alternatives) = crate::flow::completed_values::terminal_alternatives(
            &facts.flow.control,
            observation.result,
        ) else {
            continue;
        };
        let mut selected = Vec::new();
        for alternative in alternatives {
            let capture = facts.flow.control.operands.get(alternative);
            if capture.state_symbol != state || capture.statement_index != statement_index {
                selected.clear();
                break;
            }
            let Some(end) = completion_cut(occurrences, moves, capture.expression) else {
                selected.clear();
                break;
            };
            let completion = &occurrences[end];
            if producer_ordinal >= end || !completion.route.starts_with(&producer.route) {
                selected.clear();
                break;
            }
            let uses_source = |occurrence: &StatementOccurrence| {
                matches!(&occurrence.step, StatementStep::Observe { place: later_place, .. } if later_place.root == place.root)
                    || matches!(occurrence.step, StatementStep::Move { event, .. }
                        if moves.get(event).is_some_and(|event| event.root == place.root))
            };
            // The exact completed result above retains no part of this home.
            // Holding that result for an enclosing call therefore cannot keep
            // its supplying descriptor storage borrowed. Other observations
            // and moves still retain their own holds, including later uses.
            if occurrences.iter().enumerate().any(|(ordinal, later)| {
                crate::checks::ownership_occurrences::routes_compatible(
                    &later.route,
                    &completion.route,
                ) && uses_source(later)
                    && (ordinal > end
                        || later.held_until > end
                            && !matches!(later.step, StatementStep::Observe { expression, .. }
                            if expression == observation.expression))
            }) {
                selected.clear();
                break;
            }
            let unique_observation = observations
                .iter()
                .filter(|candidate| {
                    candidate.expression == observation.expression
                        && candidate.evaluation == evaluation
                })
                .count()
                == 1;
            selected.push(SourceCompletion {
                occurrence: end,
                expression: capture.expression,
                root: place.root,
                type_reference,
                // Preserve the observer epoch for its direct completion;
                // distinct selected alternatives own their pre-join epoch.
                constraints: unique_observation.then_some(
                    if capture.expression == observation.expression {
                        observation.constraints
                    } else {
                        capture.constraints
                    },
                ),
            });
        }
        completions.extend(selected);
    }
}

/// A value completes before its consuming moves in the shared occurrence
/// schedule. Saved reads have no new producer completion, but their actual
/// transfers still supply this cut. Never borrow a later, unrelated use.
fn completion_cut(
    occurrences: &[StatementOccurrence],
    moves: &[crate::flow::DiscoveredMoveEvent],
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<usize> {
    let own_move = |occurrence: &StatementOccurrence| {
        matches!(occurrence.step, StatementStep::Move { event, ordered: true, .. }
            if moves.get(event).is_some_and(|event| event.expression == expression))
    };
    let mut completed = occurrences.iter().enumerate().filter(|(_, occurrence)| {
        matches!(occurrence.step, StatementStep::Complete(value) if value == expression)
    });
    let first = completed.next().map(|(ordinal, _)| ordinal);
    if completed.next().is_some() {
        return None;
    }
    let first = first.or_else(|| occurrences.iter().position(own_move))?;
    let last = occurrences.iter().rposition(own_move).unwrap_or(first);
    if last < first
        || occurrences[first..=last]
            .iter()
            .skip(1)
            .any(|occurrence| !own_move(occurrence) || occurrence.route != occurrences[first].route)
    {
        return None;
    }
    Some(last)
}
