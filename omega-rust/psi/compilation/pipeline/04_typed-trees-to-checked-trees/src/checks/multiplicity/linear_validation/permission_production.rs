//! Permission production follows the shared source occurrence inventory.
//! Operand moves, invocation completion and anonymous result establishment
//! happen before the next occurrence; only named receiving writes wait for
//! the expression join. Grouping calls separately from statement moves would
//! establish a temporary after its consuming branch had already reconverged.
//! The emitted ledger retains exact producer identities for outcome mapping
//! and the storage extent of each write for contextual cleanup.

use crate::checks::multiplicity::cleanup_dispositions::{
    CustodyCut, StorageBindingChange, StorageEstablishment, TransferCustody,
};
use crate::checks::multiplicity::linear_obligations::{
    ClaimIdentityAllocator, LinearPlace, WrittenLinearTarget,
};
use crate::checks::multiplicity::linear_validation::claim_provenance::{
    completed_receiving_values, completed_statement_values, written_linear_targets,
    written_owner_targets,
};
use crate::checks::multiplicity::selected_evaluation::SelectedEvaluation;
use crate::checks::multiplicity::selected_evaluation::place_bindings::{
    SelectionSite, enter_produced,
};
use crate::checks::multiplicity::{projected_affine, projected_replacements, temporary_results};
use crate::checks::ownership_occurrences::{StatementOccurrence, StatementStep};
use crate::flow::FlowOwnershipEventSource;
use checked_trees::{CheckFacts, FlowPermissionEventFact};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;

mod observed_completions;
mod projected_scopes;
use projected_scopes::ProjectedScopes;

pub(crate) struct ProducedStorage {
    pub(crate) binding_changes: Vec<StorageBindingChange>,
    pub(crate) establishments: Vec<StorageEstablishment>,
    pub(crate) transfers: Vec<TransferCustody>,
    /// Prefix already reflected in joined storage custody. Claim events alone
    /// cannot reconstruct whole-carrier moves that contain several claims.
    pub(crate) evaluated_events: usize,
}

/// Expression evaluation has joined, but the receiving statement has not
/// installed its value. Keep the actual transfer snapshots and pending targets
/// across this cut: selection binding consumes evaluated arrivals, never a
/// second walk of the source or the statement's entry custody.
pub(crate) struct EvaluatedStorage {
    binding_changes: Vec<StorageBindingChange>,
    pub(crate) transfers: Vec<TransferCustody>,
    pub(crate) observations: Vec<ObservedCustody>,
    event_start: usize,
    checked_through: usize,
    written_targets: Vec<(WrittenLinearTarget, PermissionEventSource)>,
    calls: Vec<checked_trees::FlowCallFact>,
    projected_scopes: ProjectedScopes,
}

/// Custody at an actual operand read, before its following move or reborrow.
/// Reference bindings have ordinary reborrow usage and need not emit a value
/// Transfer event. Their selected permission still needs the same post-effect
/// arrival cut as an owned aggregate; an absent event is not an absent read.
pub(crate) struct ObservedCustody {
    /// Permission-event prefix at this actual read, before its following move.
    pub(crate) cut: CustodyCut,
    pub(crate) expression: typed_trees::expression::ExpressionHandle,
    pub(crate) place: validation::CanonicalPlace,
    pub(crate) places: Vec<LinearPlace>,
    pub(crate) remaining: super::super::cleanup_dispositions::RemainingCustody,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_statement_permissions(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    moves: &[crate::flow::DiscoveredMoveEvent],
    occurrences: &[StatementOccurrence],
    observed_temporaries: &[(
        typed_trees::expression::ExpressionHandle,
        typed_trees::types::TypeReferenceHandle,
    )],
    state_calls: &[checked_trees::FlowCallFact],
    completion_regions: &[(PermissionEventSource, Vec<validation::CanonicalPlace>)],
    call_writes: &[(
        PermissionEventSource,
        Option<Vec<validation::CanonicalPlace>>,
    )],
    temporary_owners: &[temporary_results::TemporaryOwner],
    statement_index: usize,
    statement: &StatementNode,
    evaluation: checked_trees::FlowPermissionEvaluation,
    capture: Option<&super::super::owned_selection::SelectionProduction>,
    places: &mut [LinearPlace],
    remaining: &mut super::super::cleanup_dispositions::RemainingCustody,
    permission_events: &mut Vec<FlowPermissionEventFact>,
    claim_identities: &mut ClaimIdentityAllocator,
    conditional_carrier_transfers: &mut Vec<usize>,
    selections: &mut super::super::owned_selection::SelectionOccurrences,
    cleanup_dispositions: &mut checked_trees::ContextualCleanupFacts,
    claim_outcomes: &mut super::super::claim_outcomes::ClaimOutcomePreparation,
) -> Result<Option<EvaluatedStorage>, diagnostics::Diagnostic> {
    // Destructure coverage markers are proof-only reads synthesized by the
    // parser; they neither transfer a value nor establish user storage.
    if matches!(statement, StatementNode::LocalData(local) if local.role == language_core::LocalBindingRole::ArmPattern)
    {
        return Ok(None);
    }

    let event_start = permission_events.len();
    let statement_source = PermissionEventSource::Statement { statement_index };
    let (machine, state) = program
        .machine_by_symbol(machine_symbol)
        .zip(program.state_by_symbol(state_symbol))
        .ok_or_else(|| {
            diagnostics::Diagnostic::error("permission evaluation has no owning machine/state")
        })?;
    let mut written_targets = written_linear_targets(
        program,
        state_symbol,
        statement_index,
        statement,
        evaluation,
        places,
    )
    .into_iter()
    .map(|target| (target, statement_source))
    .collect::<Vec<_>>();
    for &(expression, _) in observed_temporaries {
        let target = validation::CanonicalPlace {
            root: facts::PlaceRoot::Expression(expression),
            segments: Vec::new(),
        };
        let captured_source = capture.is_some_and(|capture| {
            capture.evaluation == evaluation && capture.captures_anonymous_source(expression)
        });
        // An observed owner is established by its own completion, which
        // belongs to exactly one evaluation of this statement. Another
        // evaluation (a transition target reusing its guard's subject)
        // observes that same owner; it must not establish it again.
        // A direct stored value normally completes at its named receiver.
        // A captured source that must transfer into that receiver is a
        // distinct custody coordinate despite sharing the value expression.
        if written_targets.iter().any(|(written, _)| {
            written.value == expression && (written.root == target.root || !captured_source)
        })
            || temporary_owners
            .iter()
            .any(|owner| owner.expression == expression)
            || !occurrences.iter().any(|occurrence| {
                matches!(occurrence.step, StatementStep::Complete(completed) if completed == expression)
            })
        {
            continue;
        }
        written_targets.extend(
            written_owner_targets(
                program,
                state_symbol,
                statement_index,
                &target,
                expression,
                places,
            )
            .into_iter()
            .map(|target| (target, statement_source)),
        );
    }
    for owner in temporary_owners.iter().filter(|owner| {
        matches!(owner.consumer,
            PermissionEventSource::Call { statement_index: index, .. }
                | PermissionEventSource::Statement { statement_index: index }
            if index == statement_index)
            && owner.evaluation == evaluation
    }) {
        let target = validation::CanonicalPlace {
            root: facts::PlaceRoot::Expression(owner.expression),
            segments: Vec::new(),
        };
        written_targets.extend(
            written_owner_targets(
                program,
                state_symbol,
                statement_index,
                &target,
                owner.expression,
                places,
            )
            .into_iter()
            .map(|target| (target, owner.producer)),
        );
    }

    for (target, source) in &mut written_targets {
        if let Some(expression) = target.root.as_expression()
            && let Some(call) = state_calls.iter().find(|call| {
                call.statement_index == statement_index && call.authored_expression == expression
            })
        {
            *source = PermissionEventSource::Call {
                statement_index,
                call_ordinal: call.call_ordinal,
                target_symbol: call.target_symbol,
            };
        }
    }
    // The receiving write owns disposal and storage establishment, but the
    // selected completions own resource identity. Retain that write with an
    // explicitly unavailable invariant identity; the complete capture receipt
    // supplies its guarded availability after ordinary storage production.
    if let Some(capture) = capture {
        written_targets.retain(|(target, _)| {
            !target
                .root
                .as_expression()
                .is_some_and(|expression| capture.transported.contains(&expression))
                || (capture.destination.root.is_valid()
                    && capture.receives(target.root, &target.destination_path))
        });
        for (target, _) in &mut written_targets {
            if capture.destination.root.is_valid()
                && capture.receives(target.root, &target.destination_path)
            {
                target.claim_identity = Some(PermissionClaimIdentity::Unknown);
                target.provenance = Some(PermissionProvenance::Unknown);
            }
        }
    }

    // Moves out of initializer/assignment sources happen before the
    // destination becomes established. The old move-only summary also
    // contains a production event *at* the destination; exclude that
    // compatibility event here rather than mistaking creation for use.
    let calls = state_calls
        .iter()
        .filter(|call| {
            program
                .machine_by_symbol(machine_symbol)
                .zip(program.state_by_symbol(state_symbol))
                .is_some_and(|(machine, state)| {
                    call.statement_index == statement_index
                        && crate::flow::call_permission_evaluation(
                            program,
                            machine,
                            state,
                            statement_index,
                            call.call_ordinal,
                        ) == evaluation
                })
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut storage_establishments = Vec::new();
    let mut transfers = Vec::new();
    let mut observations = Vec::new();
    let mut binding_changes = Vec::new();
    let mut source_completions = copy_source_completions(
        program,
        facts,
        machine_symbol,
        state_symbol,
        statement_index,
        evaluation,
        occurrences,
        temporary_owners,
    );
    observed_completions::append(
        program,
        facts,
        machine_symbol,
        state_symbol,
        statement_index,
        evaluation,
        moves,
        occurrences,
        temporary_owners,
        &mut source_completions,
    );
    let mut checked_through = event_start;
    let mut custody = (places.to_vec(), remaining.clone());
    let mut selected = SelectedEvaluation::new();
    let mut projected_scopes = ProjectedScopes::default();
    let completed_values = completed_statement_values(program, statement);
    let selection_site = SelectionSite {
        machine: machine_symbol,
        state: state_symbol,
        statement: statement_index,
        evaluation,
    };
    // Every produced occurrence leaves each departing scope through the same
    // retirement route; the four occurrences (observe, arm-local move,
    // complete, and the statement's own exit) each restated the same
    // eleven-argument staging of it. The macro owns the tuple here, where the
    // occurrences already are, so a site names only the branch route it keeps.
    macro_rules! retire_produced {
        ($route:expr) => {
            enter_produced(
                &mut selected,
                $route,
                &mut custody,
                selection_site,
                &mut facts.flow.ownership,
                claim_identities,
                |branch, current, ownership| {
                    projected_scopes.retire_scope(
                        program,
                        selection_site,
                        Some(branch),
                        &mut current.0,
                        &mut current.1,
                        ownership,
                        permission_events,
                        cleanup_dispositions,
                    )
                },
            )?
        };
    }
    // Calls are completion facts, not a second ordering authority. Grouping
    // them before construction moves delays anonymous custody past joins.
    for (occurrence_ordinal, occurrence) in occurrences.iter().enumerate() {
        match occurrence.step {
            StatementStep::Observe {
                ref place,
                expression,
            } => {
                retire_produced!(&occurrence.route);
                observations.push(ObservedCustody {
                    cut: CustodyCut {
                        occurrence_ordinal,
                        permission_prefix: permission_events.len() - event_start,
                    },
                    expression,
                    place: place.clone(),
                    places: custody
                        .0
                        .iter()
                        .filter(|candidate| candidate.root == place.root)
                        .cloned()
                        .collect(),
                    remaining: custody.1.clone(),
                });
            }
            StatementStep::Move {
                event,
                ordered,
                conditional: _,
            } => {
                let event = &moves[event];
                if event.evaluation != evaluation {
                    continue;
                }
                let path = facts.flow.ownership.segments.span_or_empty(event.segments);
                // Admission covers every source move, including unmatched
                // occurrences that cannot publish ordered permission events
                // below. Borrowed extraction keeps its
                // separate window contract rather than acquiring ownership.
                if !path.is_empty()
                    && !super::super::linear_obligations::move_event_is_production_target(
                        program, state, event, path,
                    )
                    && !projected_affine::is_borrowed_place_transfer(
                        program,
                        machine_symbol,
                        state,
                        event,
                        path,
                    )
                    && let Some(diagnostic) = temporary_results::unselected_claim_diagnostic(
                        program,
                        state_symbol,
                        event,
                        path,
                        temporary_owners,
                    )
                {
                    return Err(diagnostic);
                }
                if !ordered {
                    continue;
                }
                if projected_affine::is_borrowed_place_transfer(
                    program,
                    machine_symbol,
                    state,
                    event,
                    path,
                ) {
                    continue;
                }
                let transfers_custody = projected_affine::transfers_value_custody(
                    program,
                    state_symbol,
                    statement_index,
                    event,
                    path,
                );
                // Arm-local moves follow the same retained decision route as
                // their completions. Skipping them leaves the source live at
                // the join even when every arm transfers it. The route joins
                // custody without publishing an unconditional move or losing
                // the cleanup owed by an arm that did not transfer it.
                retire_produced!(&occurrence.route);
                let (places, remaining) = &mut custody;
                // The occurrence inventory includes copied observations. They
                // still complete their evaluations, but transfer neither the
                // source carrier nor its contained claims or projected scope.
                if !transfers_custody {
                    observed_completions::complete_at(
                        program,
                        facts,
                        &source_completions,
                        occurrences,
                        moves,
                        temporary_owners,
                        occurrence_ordinal,
                        selection_site,
                        places,
                        remaining,
                        permission_events,
                        cleanup_dispositions,
                    )?;
                    continue;
                }
                // A transition's selected exit owns its residual schedule.
                // Ordinary consumers close anonymous scopes before each join;
                // call-held projections retain their exact call continuation.
                if !matches!(statement, StatementNode::Transition(_)) {
                    projected_scopes.observe_move(
                        occurrences,
                        temporary_owners,
                        event,
                        facts.flow.ownership.segments.span_or_empty(event.segments),
                    )?;
                }
                let event_path = facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(event.segments)
                    .to_vec();
                select_static_case_alternative(event.root, &event_path, places);
                if program.state_by_symbol(state_symbol).is_some_and(|state| {
                    super::super::linear_obligations::move_event_is_production_target(
                        program,
                        state,
                        event,
                        &event_path,
                    )
                }) {
                    observed_completions::complete_at(
                        program,
                        facts,
                        &source_completions,
                        occurrences,
                        moves,
                        temporary_owners,
                        occurrence_ordinal,
                        selection_site,
                        places,
                        remaining,
                        permission_events,
                        cleanup_dispositions,
                    )?;
                    continue;
                }
                trace_transfer_cut(program, machine_symbol, event, &event_path, places);
                let matching = places
                    .iter()
                    .enumerate()
                    .filter_map(|(index, place)| {
                        (place.root == event.root && move_selects_claim(&event_path, place))
                            .then_some(index)
                    })
                    .collect::<Vec<_>>();
                // Storage custody moves once at the authored carrier path;
                // nested linear claims retain their separate exact transfers.
                let projected_start = permission_events.len();
                let transfer_snapshot = {
                    let position = transfers.len();
                    let start = projected_start - event_start;
                    transfers.push(TransferCustody {
                        occurrence_ordinal,
                        events: start..start,
                        places: places.clone(),
                        remaining: remaining.clone(),
                    });
                    position
                };
                projected_affine::append_transfer(
                    program,
                    facts,
                    machine_symbol,
                    state_symbol,
                    event,
                    &event_path,
                    places,
                    remaining,
                    permission_events,
                )?;
                transfers[transfer_snapshot].events.end = permission_events.len() - event_start;
                // A projected affine subtree may have no tracked claim leaf.
                // Its captured storage transfer still changes this branch's
                // cleanup complement before any enclosing join.
                remaining.advance(
                    program,
                    &permission_events[projected_start..],
                    &facts.flow.ownership.segments,
                );
                let kind = PermissionEventKind::Transfer;
                for index in matching {
                    let claim_path = places[index].path.clone();
                    if excluded_carrier_payload(&places[index], &event_path) {
                        continue;
                    }
                    let segments = facts.flow.ownership.segments.insert_many(claim_path);
                    let place = &mut places[index];
                    let obligation_live = place.live && place.multiplicity == Multiplicity::Linear;
                    // Bind the destination from the selected live transfer.
                    // Planning happened before this statement's producers ran;
                    // an anonymous source therefore had no identity then.
                    for (target, _) in &mut written_targets {
                        if target.source_place.as_ref().is_some_and(|source| {
                            source.root == place.root && source.segments == place.path
                        }) {
                            target.claim_identity = place.claim_identity;
                            target.provenance = place.provenance;
                            target.obligation_live = obligation_live;
                        }
                    }
                    record_conditional_carrier_transfer(
                        place,
                        &event_path,
                        permission_events.len(),
                        conditional_carrier_transfers,
                    );
                    permission_events.push(FlowPermissionEventFact {
                        expression: event.expression,
                        evaluation: checked_trees::FlowPermissionEvaluation::Statement,
                        machine_symbol,
                        state_symbol,
                        source: permission_source(event.source),
                        kind,
                        multiplicity: place.multiplicity,
                        access: PermissionAccess::Owned,
                        claim_identity: place
                            .claim_identity
                            .unwrap_or(PermissionClaimIdentity::Unknown),
                        provenance: place.provenance.unwrap_or(PermissionProvenance::Unknown),
                        root: event.root,
                        segments,
                        obligation_live,
                    });
                    place.live = false;
                }
                transfers[transfer_snapshot].events.end = permission_events.len() - event_start;
                remaining.observe_selected_transfer(
                    program,
                    state_symbol,
                    statement_index,
                    event.root,
                    event.expression,
                    &event_path,
                );
                remaining.transfer_storage(event.root, &event_path);
                observed_completions::complete_at(
                    program,
                    facts,
                    &source_completions,
                    occurrences,
                    moves,
                    temporary_owners,
                    occurrence_ordinal,
                    selection_site,
                    places,
                    remaining,
                    permission_events,
                    cleanup_dispositions,
                )?;
            }

            StatementStep::Complete(expression) => {
                retire_produced!(&occurrence.route);
                let (places, remaining) = &mut custody;
                let composition_start = permission_events.len();
                if !capture.is_some_and(|capture| capture.transported.contains(&expression)) {
                    temporary_results::append_owned_transfer(
                        facts,
                        machine_symbol,
                        state_symbol,
                        statement_source,
                        evaluation,
                        expression,
                        temporary_owners,
                        &mut written_targets,
                        places,
                        remaining,
                        permission_events,
                        conditional_carrier_transfers,
                    )?;
                }
                remaining.observe_ordinary(
                    program,
                    &permission_events[composition_start..],
                    &facts.flow.ownership.segments,
                    cleanup_dispositions,
                    checked_trees::FlowClaimJoinExit::default(),
                    &[],
                    false,
                );
                // Invalid handles are the standalone statement/named-target
                // boundary. The exact evaluation filter above selects that
                // one invocation; expression operators keep valid handles.
                for call in calls
                    .iter()
                    .filter(|call| call.authored_expression == expression)
                {
                    let source = PermissionEventSource::Call {
                        statement_index,
                        call_ordinal: call.call_ordinal,
                        target_symbol: call.target_symbol,
                    };
                    let completion_start = permission_events.len();
                    binding_changes.push(StorageBindingChange {
                        cut: CustodyCut {
                            occurrence_ordinal,
                            permission_prefix: completion_start - event_start,
                        },
                        source,
                        evaluation,
                        route: occurrence.route.clone(),
                        writes: call_writes
                            .iter()
                            .find(|(candidate, _)| *candidate == source)
                            .and_then(|(_, writes)| writes.clone()),
                    });
                    remaining.invalidate_call_observations(
                        program,
                        call_writes
                            .iter()
                            .find(|(candidate, _)| *candidate == source)
                            .and_then(|(_, writes)| writes.as_deref()),
                    );
                    temporary_results::append_owned_transfer(
                        facts,
                        machine_symbol,
                        state_symbol,
                        source,
                        evaluation,
                        expression,
                        temporary_owners,
                        &mut written_targets,
                        places,
                        remaining,
                        permission_events,
                        conditional_carrier_transfers,
                    )?;
                    let retired_owners = temporary_results::append_continuation_cleanup(
                        program,
                        facts,
                        machine_symbol,
                        state_symbol,
                        source,
                        typed_trees::expression::ExpressionHandle::invalid(),
                        evaluation,
                        temporary_owners,
                        places,
                        remaining,
                        permission_events,
                        cleanup_dispositions,
                    )?;
                    if let Some((_, regions)) = completion_regions
                        .iter()
                        .find(|(candidate, _)| *candidate == source)
                    {
                        super::loan_completions::restore(
                            program,
                            facts,
                            machine_symbol,
                            state_symbol,
                            source,
                            regions,
                            places,
                            claim_identities,
                            permission_events,
                        )?;
                    }
                    // Completion disposes anonymous owners as well as moving
                    // operands. Publish those contextual deaths before their
                    // storage retires; advancing custody alone loses the hook
                    // and residual action roster needed by execution.
                    remaining.observe_ordinary(
                        program,
                        &permission_events[completion_start..],
                        &facts.flow.ownership.segments,
                        cleanup_dispositions,
                        checked_trees::FlowClaimJoinExit::default(),
                        &[],
                        false,
                    );
                    for root in retired_owners {
                        remaining.retire_storage(
                            root,
                            machine_symbol,
                            state_symbol,
                            source,
                            cleanup_dispositions,
                        );
                    }
                }
                // A completed anonymous owner must exist before the next
                // operand or invocation uses it, including inside a selection.
                // Outgoing values normally wait for the receiving cut. A
                // captured source is a distinct custody role even when it
                // shares that expression: establish it here before transfer.
                let captured_source = capture.is_some_and(|capture| {
                    capture.evaluation == evaluation
                        && capture.captures_anonymous_source(expression)
                });
                let (completed, pending): (Vec<_>, Vec<_>) =
                    written_targets.into_iter().partition(|(target, _)| {
                        target.root == facts::PlaceRoot::Expression(expression)
                            && (captured_source
                                || !completed_values.iter().any(|(receiving, source)| {
                                    *receiving == expression && *source == evaluation
                                }))
                    });
                written_targets = pending;
                let establishment_start = permission_events.len();
                let first_storage = storage_establishments.len();
                establish_targets(
                    program,
                    facts,
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    completed,
                    places,
                    remaining,
                    permission_events,
                    claim_identities,
                    selections,
                    event_start,
                    &mut checked_through,
                    &mut storage_establishments,
                    claim_outcomes,
                )?;
                let establishments = storage_establishments[first_storage..]
                    .iter()
                    .map(|establishment| StorageEstablishment {
                        event_ordinal: establishment.event_ordinal + event_start
                            - establishment_start,
                        destination: establishment.destination.clone(),
                    })
                    .collect::<Vec<_>>();
                if let Some(capture) = capture {
                    for owner in capture.owners.iter().filter(|owner| {
                        owner.expression == expression
                            && owner.source.root == facts::PlaceRoot::Expression(expression)
                    }) {
                        let transfer_snapshot = transfers.len();
                        let transfer_start = permission_events.len() - event_start;
                        transfers.push(TransferCustody {
                            occurrence_ordinal,
                            events: transfer_start..transfer_start,
                            places: places.to_vec(),
                            remaining: remaining.clone(),
                        });
                        for place in places.iter_mut().filter(|place| {
                            place.root == owner.source.root
                                && place.path.starts_with(&owner.source.segments)
                        }) {
                            if !place.ever_established {
                                return Err(diagnostics::Diagnostic::error(format!(
                                    "selected completion has no established source custody in machine `{}`, state `{}`, statement {}",
                                    machine.name.as_str(),
                                    state.name.as_str(),
                                    statement_index + 1,
                                ))
                                .with_source_span(program.expression_table.source_span(expression)));
                            }
                            if excluded_carrier_payload(place, &owner.source.segments) {
                                continue;
                            }
                            permission_events.push(FlowPermissionEventFact {
                                expression,
                                evaluation,
                                machine_symbol,
                                state_symbol,
                                source: statement_source,
                                kind: PermissionEventKind::Transfer,
                                multiplicity: place.multiplicity,
                                access: PermissionAccess::Owned,
                                claim_identity: place
                                    .claim_identity
                                    .unwrap_or(PermissionClaimIdentity::Unknown),
                                provenance: place
                                    .provenance
                                    .unwrap_or(PermissionProvenance::Unknown),
                                root: place.root,
                                segments: facts
                                    .flow
                                    .ownership
                                    .segments
                                    .insert_many(place.path.iter().copied()),
                                obligation_live: place.live
                                    && place.multiplicity == Multiplicity::Linear,
                            });
                            place.live = false;
                        }
                        transfers[transfer_snapshot].events.end =
                            permission_events.len() - event_start;
                        remaining.transfer_storage(owner.source.root, &owner.source.segments);
                    }
                }
                remaining.observe_ordinary(
                    program,
                    &permission_events[establishment_start..],
                    &facts.flow.ownership.segments,
                    cleanup_dispositions,
                    checked_trees::FlowClaimJoinExit::default(),
                    &establishments,
                    false,
                );
                crate::checks::content::observe_call_completion(
                    program,
                    facts,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    expression,
                    evaluation,
                    permission_events,
                );
                if !completed_receiving_values(
                    program,
                    facts,
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    statement,
                )
                .iter()
                .any(|(receiving, source)| *receiving == expression && *source == evaluation)
                {
                    complete_body_custody(
                        program,
                        facts,
                        machine,
                        state,
                        statement_index,
                        expression,
                        evaluation,
                        places,
                        remaining,
                        permission_events,
                        event_start,
                        cleanup_dispositions,
                        crate::checks::multiplicity::CompletedClaimLocation::Expression,
                    );
                }
                observed_completions::complete_at(
                    program,
                    facts,
                    &source_completions,
                    occurrences,
                    moves,
                    temporary_owners,
                    occurrence_ordinal,
                    selection_site,
                    places,
                    remaining,
                    permission_events,
                    cleanup_dispositions,
                )?;
            }
        }
    }
    retire_produced!(&[]);
    places.clone_from_slice(&custody.0);
    *remaining = custody.1;
    let stored_transfer_start = permission_events.len();
    temporary_results::append_stored_transfer(
        facts,
        machine_symbol,
        state_symbol,
        statement_index,
        evaluation,
        places,
        remaining,
        permission_events,
        conditional_carrier_transfers,
    )?;
    remaining.observe_ordinary(
        program,
        &permission_events[stored_transfer_start..],
        &facts.flow.ownership.segments,
        cleanup_dispositions,
        checked_trees::FlowClaimJoinExit::default(),
        &[],
        false,
    );
    for event in &mut permission_events[event_start..] {
        event.evaluation = evaluation;
    }
    Ok(Some(EvaluatedStorage {
        binding_changes,
        transfers,
        observations,
        event_start,
        checked_through,
        written_targets,
        calls,
        projected_scopes,
    }))
}

/// Finish the receiving write only after all evaluated source arrivals have
/// been retained. The selected-value binder may supply its exact receiving
/// claims at this seam; ordinary destinations use the same pending targets.
#[allow(clippy::too_many_arguments)]
pub(crate) fn complete_statement_permissions(
    program: &dyn validation::CheckingInput,
    facts: &mut CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    evaluation: checked_trees::FlowPermissionEvaluation,
    evaluated: EvaluatedStorage,
    places: &mut [LinearPlace],
    remaining: &mut super::super::cleanup_dispositions::RemainingCustody,
    permission_events: &mut Vec<FlowPermissionEventFact>,
    claim_identities: &mut ClaimIdentityAllocator,
    conditional_carrier_transfers: &mut Vec<usize>,
    selections: &mut super::super::owned_selection::SelectionOccurrences,
    cleanup_dispositions: &mut checked_trees::ContextualCleanupFacts,
    claim_outcomes: &mut super::super::claim_outcomes::ClaimOutcomePreparation,
) -> Result<ProducedStorage, diagnostics::Diagnostic> {
    let EvaluatedStorage {
        binding_changes,
        transfers,
        observations: _,
        event_start,
        mut checked_through,
        written_targets,
        calls,
        mut projected_scopes,
    } = evaluated;
    let statement_source = PermissionEventSource::Statement { statement_index };
    // Branch-born owners already closed at their individual route exits.
    // Only root-scope owners wait for this normal statement completion.
    projected_scopes.retire_scope(
        program,
        SelectionSite {
            machine: machine_symbol,
            state: state_symbol,
            statement: statement_index,
            evaluation,
        },
        None,
        places,
        remaining,
        &mut facts.flow.ownership,
        permission_events,
        cleanup_dispositions,
    )?;
    let evaluated_events = permission_events.len() - event_start;
    // Calls have already joined their branch-local custody. Replaying their
    // flattened event roster here would import transfers from sibling arms.
    let replacement_custody = remaining.clone();
    let mut storage_establishments: Vec<StorageEstablishment> = Vec::new();
    for mut establishment in projected_replacements::append_replacement(
        program,
        facts,
        machine_symbol,
        state_symbol,
        statement_index,
        statement,
        places,
        &replacement_custody,
        permission_events,
        cleanup_dispositions,
    )? {
        // The storage receipt names this write's first establishing event.
        // Authenticate the RHS and old cleanup before ending only the covered
        // selected occurrence; the new event itself remains under the guard.
        selections.check_events(
            program,
            &permission_events[checked_through..establishment.event_ordinal],
            &facts.flow.ownership.segments,
        )?;
        selections.establish(
            establishment.destination.root,
            &establishment.destination.segments,
        );
        checked_through = establishment.event_ordinal;
        // The producer returns its exact index in the shared event vector;
        // both cleanup consumers replay this statement's event slice.
        establishment.event_ordinal -= event_start;
        storage_establishments.push(establishment);
    }

    establish_targets(
        program,
        facts,
        machine_symbol,
        state_symbol,
        statement_index,
        written_targets,
        places,
        &replacement_custody,
        permission_events,
        claim_identities,
        selections,
        event_start,
        &mut checked_through,
        &mut storage_establishments,
        claim_outcomes,
    )?;
    // Nested anonymous owners complete in evaluation; named and returned
    // owners complete at this receiving cut. Retry their call correspondence
    // before body custody and return transfer, now that the frontier exists.
    for call in &calls {
        crate::checks::content::observe_call_completion(
            program,
            facts,
            machine_symbol,
            state_symbol,
            statement_index,
            call.authored_expression,
            evaluation,
            permission_events,
        );
    }
    for (expression, source) in completed_receiving_values(
        program,
        facts,
        machine_symbol,
        state_symbol,
        statement_index,
        statement,
    ) {
        if source != evaluation {
            continue;
        }
        let (machine, state) = program
            .machine_by_symbol(machine_symbol)
            .zip(program.state_by_symbol(state_symbol))
            .ok_or_else(|| {
                diagnostics::Diagnostic::error("receiving completion has no owning machine/state")
            })?;
        complete_body_custody(
            program,
            facts,
            machine,
            state,
            statement_index,
            expression,
            evaluation,
            places,
            remaining,
            permission_events,
            event_start,
            cleanup_dispositions,
            crate::checks::multiplicity::CompletedClaimLocation::ReceivingStorage,
        );
    }
    // Returning a completed expression transfers its established frontier;
    // it is not an implicit cleanup of a freshly created linear value.
    if let Some((expression, _)) = completed_statement_values(program, statement)
        .into_iter()
        .find(|(_, source)| *source == evaluation)
        && (matches!(statement, StatementNode::Transition(_))
            || program.state_by_symbol(state_symbol).is_some_and(|state| {
                statement_index + 1
                    == program
                        .statement_table
                        .statements(state.statement_nodes)
                        .len()
            }))
    {
        for place in places
            .iter_mut()
            .filter(|place| place.root == facts::PlaceRoot::Expression(expression) && place.live)
        {
            record_conditional_carrier_transfer(
                place,
                &[],
                permission_events.len(),
                conditional_carrier_transfers,
            );
            permission_events.push(FlowPermissionEventFact {
                expression,
                evaluation,
                machine_symbol,
                state_symbol,
                source: statement_source,
                kind: PermissionEventKind::Transfer,
                multiplicity: place.multiplicity,
                access: PermissionAccess::Owned,
                claim_identity: place
                    .claim_identity
                    .unwrap_or(PermissionClaimIdentity::Unknown),
                provenance: place.provenance.unwrap_or(PermissionProvenance::Unknown),
                root: place.root,
                segments: facts
                    .flow
                    .ownership
                    .segments
                    .insert_many(place.path.iter().copied()),
                obligation_live: place.multiplicity == Multiplicity::Linear,
            });
            place.live = false;
        }
    }
    for event in &mut permission_events[event_start..] {
        event.evaluation = evaluation;
    }
    selections.check_events(
        program,
        &permission_events[checked_through..],
        &facts.flow.ownership.segments,
    )?;
    Ok(ProducedStorage {
        binding_changes,
        establishments: storage_establishments,
        transfers,
        evaluated_events,
    })
}

#[allow(clippy::too_many_arguments)]
fn complete_body_custody(
    program: &dyn validation::CheckingInput,
    facts: &mut CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    evaluation: checked_trees::FlowPermissionEvaluation,
    places: &mut [LinearPlace],
    remaining: &mut super::super::cleanup_dispositions::RemainingCustody,
    permission_events: &mut Vec<FlowPermissionEventFact>,
    event_start: usize,
    cleanup_dispositions: &mut checked_trees::ContextualCleanupFacts,
    location: crate::checks::multiplicity::CompletedClaimLocation,
) {
    if facts.qualifications.content.conservation_plans.is_empty() {
        return;
    }
    let body_conservations = crate::checks::content::observe_body_completion(
        program,
        facts,
        machine,
        state,
        statement_index,
        expression,
        evaluation,
        places,
        permission_events,
        &permission_events[event_start..],
        location,
    );
    let transformation_start = permission_events.len();
    // A missing conservation grants nothing; ordinary checking still owns
    // admission. Invoke this consumer only once its receiving frontier exists.
    let _ = super::super::content_transformations::commit(
        program,
        facts,
        &body_conservations,
        places,
        permission_events,
    );
    remaining.observe_ordinary(
        program,
        &permission_events[transformation_start..],
        &facts.flow.ownership.segments,
        cleanup_dispositions,
        checked_trees::FlowClaimJoinExit::default(),
        &[],
        false,
    );
}

struct SourceCompletion {
    occurrence: usize,
    expression: typed_trees::expression::ExpressionHandle,
    root: facts::PlaceRoot,
    type_reference: typed_trees::types::TypeReferenceHandle,
    constraints: Option<arena::HandleSpan<checked_trees::FlowConstraintRef>>,
}

/// A copied leaf no longer uses its anonymous supplier's storage. End that
/// supplier at the actual read completion, on the route that established it.
/// Only descriptor-only, claim-free owners have no observable cleanup timing
/// to preserve. Existing loans of the supplier's storage keep their own lifetime;
/// loans stored inside it continue through the independently copied carrier.
fn copy_source_completions(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    evaluation: checked_trees::FlowPermissionEvaluation,
    occurrences: &[crate::checks::ownership_occurrences::StatementOccurrence],
    temporary_owners: &[temporary_results::TemporaryOwner],
) -> Vec<SourceCompletion> {
    let mut completions = Vec::new();
    for (observation_ordinal, observation) in occurrences.iter().enumerate() {
        let StatementStep::Observe { place, expression } = &observation.step else {
            continue;
        };
        let expression = *expression;
        if !expression.is_valid()
            || program
                .expression_table
                .borrow_expression(expression)
                .is_some()
        {
            continue;
        }
        let Some(source) = place.root.as_expression() else {
            continue;
        };
        if place.segments.is_empty()
            || temporary_owners
                .iter()
                .any(|owner| owner.expression == source)
            || completions
                .iter()
                .any(|completion: &SourceCompletion| completion.root == place.root)
        {
            continue;
        }
        let mut reads = occurrences[observation_ordinal + 1..].iter().enumerate().filter(|(_, occurrence)| {
            matches!(occurrence.step, StatementStep::Complete(completed) if completed == expression)
        });
        let Some((read_ordinal, occurrence)) = reads.next() else {
            continue;
        };
        if reads.next().is_some() {
            continue;
        }
        let ordinal = observation_ordinal + 1 + read_ordinal;
        let Some(selected) =
            validation::canonical_place_type_reference(program, state, statement_index, place)
        else {
            continue;
        };
        if !validation::type_satisfies_declared_property(
            program,
            &[],
            selected,
            validation::DeclaredPropertyRequirement::Copy,
        ) {
            continue;
        }
        let mut producers = occurrences[..observation_ordinal].iter().filter(|producer| {
            matches!(producer.step, StatementStep::Complete(completed) if completed == source)
        });
        let Some(producer) = producers.next() else {
            continue;
        };
        if producers.next().is_some()
            // Both ends belong to the same selected evaluation. A read in
            // only one later arm cannot retire a supplier established before
            // that decision: its skipped arm still owes the original death.
            || producer.route != occurrence.route
            || occurrences[ordinal + 1..].iter().any(|later| {
                matches!(&later.step, StatementStep::Observe { place: later_place, .. } if later_place.root == place.root)
            })
        {
            continue;
        }
        let Some(reference) = validation::canonical_place_type_reference(
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
        if program.type_multiplicity(reference) != Multiplicity::Affine
            || crate::checks::type_carries_linear_obligation(program, reference)
            || !validation::has_contents(
                program,
                reference,
                &[],
                validation::ContentsRequirement::DescriptorCleanup,
            )
            || facts.borrow.with_state_borrows(machine, state, |rows| {
                rows.iter()
                    .map(|handle| facts.borrow.states.get(*handle))
                    .any(|state_fact| {
                        facts
                            .borrow
                            .loans
                            .span_or_empty(state_fact.loans)
                            .iter()
                            .any(|loan| loan.root == place.root)
                    })
            })
        {
            continue;
        }
        // The schedule still owns hook-free deaths when no semantic context
        // was published. Context-dependent cleanup must refuse that absence;
        // a known empty exact context remains different from missing evidence.
        let constraints = facts
            .flow
            .control
            .state(machine, state)
            .and_then(|flow_state| facts.flow.state_statement(flow_state, statement_index))
            .and_then(|statement| {
                let mut observations = facts
                    .flow
                    .control
                    .value_observations
                    .span_or_empty(statement.value_observations)
                    .iter()
                    .filter(|observation| {
                        observation.expression == expression && observation.evaluation == evaluation
                    });
                let observation = observations.next()?;
                if observations.next().is_some()
                    || !facts.flow.control.operands.is_valid(observation.receiver)
                    || !validation::canonical_place_from_expression_in_state(
                        program,
                        state,
                        statement_index,
                        facts
                            .flow
                            .control
                            .operands
                            .get(observation.receiver)
                            .expression,
                    )
                    .is_some_and(|receiver| receiver.root == place.root)
                {
                    return None;
                }
                Some(observation.constraints)
            });
        completions.push(SourceCompletion {
            occurrence: ordinal,
            expression,
            root: place.root,
            type_reference: reference,
            constraints,
        });
    }
    completions
}

/// An anonymous owner born on one route cannot leave a cleanup event until
/// after reconvergence: that would revisit an already completed branch.
/// Dispose its complement at the edge that actually completes its value.
#[allow(clippy::too_many_arguments)]
fn establish_targets(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    written_targets: Vec<(WrittenLinearTarget, PermissionEventSource)>,
    places: &mut [LinearPlace],
    replacement_custody: &super::super::cleanup_dispositions::RemainingCustody,
    permission_events: &mut Vec<FlowPermissionEventFact>,
    claim_identities: &mut ClaimIdentityAllocator,
    selections: &mut super::super::owned_selection::SelectionOccurrences,
    event_start: usize,
    checked_through: &mut usize,
    storage_establishments: &mut Vec<StorageEstablishment>,
    claim_outcomes: &mut super::super::claim_outcomes::ClaimOutcomePreparation,
) -> Result<(), diagnostics::Diagnostic> {
    for (target, source) in written_targets {
        let place_index = target.place_index;
        let obligation_live = target.obligation_live;
        let claim_identity = target.claim_identity.unwrap_or_else(|| {
            if obligation_live {
                claim_identities.mint(machine_symbol, state_symbol, source)
            } else {
                PermissionClaimIdentity::Unknown
            }
        });
        let provenance = target.provenance;
        let claim_path = places[place_index].path.clone();
        let segments = facts.flow.ownership.segments.insert_many(claim_path);
        let place = &mut places[place_index];
        debug_assert_eq!(place.root, target.root);
        // Replacement settles the old affine value before establishing the
        // new one. A source transferred into the replacement call is already
        // dead here and must not also receive a caller-side drop. A linear
        // local's zero image is such an affine value: its first assignment
        // retires the image the same way, and only then establishes debt.
        let replaces_zero_image =
            place.multiplicity == Multiplicity::Linear && place.zero_image && place.path.is_empty();
        let replacement_already_owned = storage_establishments.iter().any(|establishment| {
            establishment.destination.root == target.root
                && establishment.destination.segments == target.destination_path
        });
        if !replacement_already_owned
            && ((place.multiplicity == Multiplicity::Affine
                && (place.live || replacement_custody.has_storage_frontier(target.root)))
                || replaces_zero_image)
        {
            for path in replacement_custody.disposal_paths(program, target.root, &place.path) {
                let discarded = validation::CanonicalPlace {
                    root: target.root,
                    segments: path,
                };
                if validation::canonical_place_type_reference(
                    program,
                    state_symbol,
                    statement_index,
                    &discarded,
                )
                .is_none_or(|reference| {
                    let multiplicity = crate::checks::type_multiplicity(program, reference);
                    multiplicity != Multiplicity::Affine
                        && !(replaces_zero_image && multiplicity == Multiplicity::Linear)
                }) {
                    continue;
                }
                let discarded_segments = facts
                    .flow
                    .ownership
                    .segments
                    .insert_many(discarded.segments);
                permission_events.push(FlowPermissionEventFact {
                    expression: target.root.as_expression().unwrap_or_default(),
                    evaluation: checked_trees::FlowPermissionEvaluation::Statement,
                    machine_symbol,
                    state_symbol,
                    source: PermissionEventSource::Statement { statement_index },
                    kind: PermissionEventKind::AffineDrop,
                    multiplicity: Multiplicity::Affine,
                    access: PermissionAccess::Owned,
                    claim_identity: place
                        .claim_identity
                        .unwrap_or(PermissionClaimIdentity::Unknown),
                    provenance: place.provenance.unwrap_or(PermissionProvenance::Unknown),
                    root: target.root,
                    segments: discarded_segments,
                    obligation_live: false,
                });
            }
        }
        // Authenticate the RHS and old-value disposal before a new whole
        // write can end a selected-source occurrence barrier.
        selections.check_events(
            program,
            &permission_events[*checked_through..],
            &facts.flow.ownership.segments,
        )?;
        *checked_through = permission_events.len();
        selections.establish(target.root, &target.destination_path);
        if !storage_establishments.iter().any(|establishment| {
            establishment.destination.root == target.root
                && establishment.destination.segments == target.destination_path
        }) {
            storage_establishments.push(StorageEstablishment {
                event_ordinal: permission_events.len() - event_start,
                destination: validation::CanonicalPlace {
                    root: target.root,
                    segments: target.destination_path.clone(),
                },
            });
        }
        let mut establishment = FlowPermissionEventFact {
            expression: target.root.as_expression().unwrap_or_default(),
            evaluation: checked_trees::FlowPermissionEvaluation::Statement,
            machine_symbol,
            state_symbol,
            source,
            kind: PermissionEventKind::Establish,
            multiplicity: place.multiplicity,
            access: PermissionAccess::Owned,
            claim_identity,
            provenance: provenance
                .unwrap_or_else(|| established_provenance(machine_symbol, state_symbol, source)),
            root: target.root,
            segments,
            obligation_live,
        };
        // Allocate the receiving occurrence before forwarding so ordinal
        // allocation stays stable across preparation iterations. Forwarded
        // claims retain the actual caller identity, never a callee template.
        claim_outcomes.resolve(
            program,
            &facts.flow.ownership.segments,
            &mut establishment,
            permission_events,
        );
        let obligation_live = establishment.obligation_live;
        place.live = obligation_live || place.multiplicity == Multiplicity::Affine;
        place.case_excluded = place.conditional && !obligation_live;
        // A linear local's obligation-free whole write is its zero image:
        // affine storage, not an established value (mirrors `applied_events`).
        place.zero_image = place.multiplicity == Multiplicity::Linear
            && !obligation_live
            && place.path.is_empty()
            && !place.conditional;
        place.ever_established |= !place.zero_image;
        place.claim_identity = Some(establishment.claim_identity);
        place.provenance = Some(establishment.provenance);
        permission_events.push(establishment);
    }

    Ok(())
}

/// Retain this exact provisional transfer only when it moves a containing
/// carrier while its conditional claim is live. Reconciliation may later
/// prove the payload absent and retire the recorded event. Named moves,
/// anonymous call arguments and completed returns share this rule; duplicate
/// transfers and direct payload extractions remain ordinary replay errors.
pub(crate) fn record_conditional_carrier_transfer(
    place: &LinearPlace,
    carrier_path: &[facts::PlaceSegment],
    event_ordinal: usize,
    conditional_carrier_transfers: &mut Vec<usize>,
) {
    if place.live
        && place.multiplicity == Multiplicity::Linear
        && place.conditional
        && place.path.strip_prefix(carrier_path).is_some_and(|suffix| {
            suffix
                .iter()
                .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
        })
    {
        conditional_carrier_transfers.push(event_ordinal);
    }
}

fn move_selects_claim(event_path: &[facts::PlaceSegment], claim: &LinearPlace) -> bool {
    claim.path.starts_with(event_path)
        || (claim.conditional && event_path.starts_with(claim.path.as_slice()))
}

pub(crate) fn select_static_case_alternative(
    root: facts::PlaceRoot,
    event_path: &[facts::PlaceSegment],
    places: &mut [LinearPlace],
) {
    let Some((case_index, selected_variant)) =
        event_path
            .iter()
            .enumerate()
            .find_map(|(index, segment)| match segment {
                facts::PlaceSegment::Case { variant } => Some((index, *variant)),
                _ => None,
            })
    else {
        return;
    };
    let prefix = &event_path[..case_index];
    for place in places {
        if place.root != root
            || place.path.get(..case_index) != Some(prefix)
            || place.path.get(case_index)
                == Some(&facts::PlaceSegment::Case {
                    variant: selected_variant,
                })
        {
            continue;
        }
        if matches!(
            place.path.get(case_index),
            Some(facts::PlaceSegment::Case { .. })
        ) {
            place.live = false;
            place.case_excluded = true;
        }
    }
}

pub(crate) fn established_provenance(
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    source: PermissionEventSource,
) -> PermissionProvenance {
    PermissionProvenance::Established {
        machine_symbol,
        state_symbol,
        source,
    }
}

pub(crate) fn event_statement_index(source: FlowOwnershipEventSource) -> Option<usize> {
    match source {
        FlowOwnershipEventSource::Statement { statement_index }
        | FlowOwnershipEventSource::Call {
            statement_index, ..
        } => Some(statement_index),
    }
}

pub(crate) fn permission_source(source: FlowOwnershipEventSource) -> PermissionEventSource {
    match source {
        FlowOwnershipEventSource::Statement { statement_index } => {
            PermissionEventSource::Statement { statement_index }
        }
        FlowOwnershipEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        },
    }
}

/// Whole-carrier transport carries the actual selected case; an authenticated
/// absent payload is not a resource to consume. An explicit case projection
/// must still prove that its selected payload exists.
fn excluded_carrier_payload(place: &LinearPlace, path: &[facts::PlaceSegment]) -> bool {
    place.ever_established
        && !place.live
        && place.case_excluded
        && !path
            .iter()
            .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
}

// Temporary bounded source-producer diagnostic; remove before landing.
fn trace_transfer_cut(
    program: &dyn validation::CheckingInput,
    machine: SymbolHandle,
    event: &crate::flow::DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
    places: &[LinearPlace],
) {
    let Ok(filter) = std::env::var("OMEGA_TRACE_CLAIM_FRONTIER") else {
        return;
    };
    if filter != "*" && filter != program.symbols.name(machine) {
        return;
    }
    static EMITTED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    if EMITTED.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= 128 {
        return;
    }
    let matching = places
        .iter()
        .filter(|place| place.root == event.root)
        .map(|place| {
            (
                place,
                move_selects_claim(path, place),
                excluded_carrier_payload(place, path),
            )
        })
        .collect::<Vec<_>>();
    eprintln!(
        "CLAIM_TRANSFER_CUT machine={} event={event:?} path={path:?} places={matching:?}",
        program.symbols.name(machine)
    );
}
