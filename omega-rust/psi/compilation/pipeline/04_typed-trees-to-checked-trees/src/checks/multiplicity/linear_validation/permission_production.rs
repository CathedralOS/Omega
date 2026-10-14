//! Permission production follows the shared source occurrence inventory.
//! Operand moves, invocation completion and anonymous result establishment
//! happen before the next occurrence; only named receiving writes wait for
//! the expression join. Grouping calls separately from statement moves would
//! establish a temporary after its consuming branch had already reconverged.
//! The emitted ledger retains exact producer identities for outcome mapping
//! and the storage extent of each write for contextual cleanup.

use crate::checks::multiplicity::cleanup_dispositions::StorageEstablishment;
use crate::checks::multiplicity::linear_obligations::{
    ClaimIdentityAllocator, LinearPlace, WrittenLinearTarget,
};
use crate::checks::multiplicity::linear_validation::claim_provenance::{
    completed_statement_values, written_linear_targets, written_owner_targets,
};
use crate::checks::multiplicity::selected_evaluation::{SelectedEvaluation, join_places};
use crate::checks::multiplicity::{projected_affine, temporary_results};
use crate::checks::ownership_occurrences::{StatementStep, statement_steps_at_evaluation};
use crate::flow::FlowOwnershipEventSource;
use checked_trees::{CheckFacts, FlowPermissionEventFact};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;

pub(crate) struct ProducedStorage {
    pub(crate) establishments: Vec<StorageEstablishment>,
    /// Prefix already reflected in joined storage custody. Claim events alone
    /// cannot reconstruct whole-carrier moves that contain several claims.
    pub(crate) evaluated_events: usize,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_statement_permission_production(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    moves: &[crate::flow::DiscoveredMoveEvent],
    state_calls: &[checked_trees::FlowCallFact],
    completion_regions: &[(PermissionEventSource, Vec<validation::CanonicalPlace>)],
    temporary_owners: &[temporary_results::TemporaryOwner],
    statement_index: usize,
    statement: &StatementNode,
    evaluation: checked_trees::FlowPermissionEvaluation,
    places: &mut [LinearPlace],
    remaining: &mut super::super::cleanup_dispositions::RemainingCustody,
    permission_events: &mut Vec<FlowPermissionEventFact>,
    claim_identities: &mut ClaimIdentityAllocator,
    conditional_carrier_transfers: &mut Vec<usize>,
    selections: &mut super::super::owned_selection::SelectionOccurrences,
    cleanup_dispositions: &mut checked_trees::ContextualCleanupFacts,
) -> Result<ProducedStorage, diagnostics::Diagnostic> {
    // Destructure coverage markers are proof-only reads synthesized by the
    // parser; they neither transfer a value nor establish user storage.
    if matches!(statement, StatementNode::LocalData(local) if local.role == language_core::LocalBindingRole::ArmPattern)
    {
        return Ok(ProducedStorage {
            establishments: Vec::new(),
            evaluated_events: 0,
        });
    }

    let event_start = permission_events.len();
    let statement_source = PermissionEventSource::Statement { statement_index };
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
    if let Some((machine, state)) = program
        .machine_by_symbol(machine_symbol)
        .zip(program.state_by_symbol(state_symbol))
    {
        for (expression, _) in crate::checks::ownership_occurrences::observed_owned_temporaries(
            program,
            machine,
            state,
            statement_index,
            statement,
            &facts.operators,
        ) {
            if temporary_owners
                .iter()
                .any(|owner| owner.expression == expression)
            {
                continue;
            }
            let target = validation::CanonicalPlace {
                root: facts::PlaceRoot::Expression(expression),
                segments: Vec::new(),
            };
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
    }
    for owner in temporary_owners.iter().filter(|owner| {
        matches!(owner.consumer, PermissionEventSource::Call { statement_index: index, .. } if index == statement_index)
            && owner.evaluation == evaluation
    }) {
        let target = validation::CanonicalPlace { root: facts::PlaceRoot::Expression(owner.expression), segments: Vec::new() };
        written_targets.extend(written_owner_targets(program, state_symbol, statement_index, &target, owner.expression, places)
            .into_iter().map(|target| (target, owner.producer)));
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
    let (machine, state) = program
        .machine_by_symbol(machine_symbol)
        .zip(program.state_by_symbol(state_symbol))
        .ok_or_else(|| {
            diagnostics::Diagnostic::error("permission evaluation has no owning machine/state")
        })?;
    let occurrences = statement_steps_at_evaluation(
        program,
        machine,
        state,
        statement_index,
        statement,
        moves,
        &facts.operators,
        Some(evaluation),
    );
    let mut storage_establishments = Vec::new();
    let mut checked_through = event_start;
    let mut custody = (places.to_vec(), remaining.clone());
    let mut selected = SelectedEvaluation::new();
    let mut projected_temporaries = Vec::new();
    let join = |current: &mut (
        Vec<LinearPlace>,
        super::super::cleanup_dispositions::RemainingCustody,
    ),
                sibling: &(
        Vec<LinearPlace>,
        super::super::cleanup_dispositions::RemainingCustody,
    )| {
        join_places(&mut current.0, &sibling.0)?;
        current.1.join(&sibling.1);
        Ok(())
    };
    // Calls are completion facts, not a second ordering authority. Grouping
    // them before construction moves delays anonymous custody past joins.
    for occurrence in occurrences {
        match occurrence.step {
            StatementStep::Observe { .. } => continue,
            StatementStep::Move { event, ordered, .. } => {
                let event = &moves[event];
                if !ordered || event.source_arm.is_valid() || event.evaluation != evaluation {
                    continue;
                }
                selected.enter(&occurrence.route, &mut custody, join)?;
                let (places, remaining) = &mut custody;
                // Moving a subtree out of an anonymous owner is that owner's
                // last use. A call that receives the subtree directly retires
                // the owner at its own continuation (temporary owners); any
                // other consumer leaves it to this statement's completion.
                if let facts::PlaceRoot::Expression(expression) = event.root
                    && !event.segments.is_empty()
                {
                    projected_temporaries.push((
                        expression,
                        facts
                            .flow
                            .ownership
                            .segments
                            .span_or_empty(event.segments)
                            .to_vec(),
                    ));
                }
                if matches!(event.root, facts::PlaceRoot::Expression(_))
                    && !places.iter().any(|place| place.root == event.root)
                {
                    let transfer_start = permission_events.len();
                    temporary_results::append_whole_affine_transfer(
                        program,
                        machine_symbol,
                        state_symbol,
                        &calls,
                        event,
                        permission_events,
                    );
                    remaining.advance(
                        program,
                        &permission_events[transfer_start..],
                        &facts.flow.ownership.segments,
                    );
                    continue;
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
                    continue;
                }
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
                );
                // A projected affine subtree may have no tracked claim leaf.
                // Its captured storage transfer still changes this branch's
                // cleanup complement before any enclosing join.
                remaining.advance(
                    program,
                    &permission_events[projected_start..],
                    &facts.flow.ownership.segments,
                );
                if matching.is_empty() {
                    continue;
                }
                let kind = PermissionEventKind::Transfer;
                for index in matching {
                    let claim_path = places[index].path.clone();
                    if places[index].ever_established
                        && !places[index].live
                        && places[index].case_excluded
                        && !event_path
                            .iter()
                            .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
                    {
                        continue;
                    }
                    let segments = facts.flow.ownership.segments.insert_many(claim_path);
                    let place = &mut places[index];
                    let obligation_live = place.live && place.multiplicity == Multiplicity::Linear;
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
                remaining.transfer_storage(event.root, &event_path);
            }

            StatementStep::Complete(expression) => {
                selected.enter(&occurrence.route, &mut custody, join)?;
                let (places, remaining) = &mut custody;
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
                    temporary_results::append_constructed_argument_transfers(
                        program,
                        machine_symbol,
                        state_symbol,
                        statement_index,
                        std::slice::from_ref(call),
                        permission_events,
                    );
                    temporary_results::append_owned_transfer(
                        facts,
                        machine_symbol,
                        state_symbol,
                        source,
                        evaluation,
                        temporary_owners,
                        places,
                        permission_events,
                        conditional_carrier_transfers,
                    );
                    let retired_owners = temporary_results::append_continuation_cleanup(
                        program,
                        facts,
                        machine_symbol,
                        state_symbol,
                        source,
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
                let (completed, pending): (Vec<_>, Vec<_>) =
                    written_targets.into_iter().partition(|(target, _)| {
                        target.root == facts::PlaceRoot::Expression(expression)
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
                )?;
                let establishments = storage_establishments[first_storage..]
                    .iter()
                    .map(|establishment| StorageEstablishment {
                        event_ordinal: establishment.event_ordinal + event_start
                            - establishment_start,
                        destination: establishment.destination.clone(),
                    })
                    .collect::<Vec<_>>();
                remaining.observe_ordinary(
                    program,
                    &permission_events[establishment_start..],
                    &facts.flow.ownership.segments,
                    cleanup_dispositions,
                    checked_trees::FlowClaimJoinExit::default(),
                    &establishments,
                    false,
                );
            }
        }
    }
    selected.enter(&[], &mut custody, join)?;
    places.clone_from_slice(&custody.0);
    *remaining = custody.1;
    // An anonymous owner's untouched complement dies when the statement that
    // moved its selected subtree completes: nothing can name the owner again.
    // A transition completes only at its selected exit, which retains that
    // owner's residual as an ordinary exit disposition.
    if !matches!(statement, StatementNode::Transition(_)) {
        for (expression, path) in projected_temporaries {
            let root = facts::PlaceRoot::Expression(expression);
            // Call-consumed owners and whole moves keep their own disposition;
            // only a transferred subtree of a still-held owner leaves one here.
            // A linear complement is refused by the partial-move check.
            if temporary_owners
                .iter()
                .any(|owner| owner.expression == expression)
                || remaining.is_present(root, &path)
                || !remaining.is_present(root, &[])
                || temporary_results::projectable_owner(
                    program,
                    state_symbol,
                    statement_index,
                    expression,
                )
                .is_none()
            {
                continue;
            }
            let Some(producer) = permission_events
                .iter()
                .rev()
                .find(|event| {
                    event.kind == PermissionEventKind::Establish
                        && event.machine_symbol == machine_symbol
                        && event.state_symbol == state_symbol
                        && event.root == root
                })
                .map(|event| event.provenance)
            else {
                continue;
            };
            let retirement_start = permission_events.len();
            temporary_results::retire_owner(
                program,
                facts,
                machine_symbol,
                state_symbol,
                temporary_results::ProjectedConsumer {
                    source: statement_source,
                    evaluation,
                    producer,
                },
                expression,
                &path,
                places,
                remaining,
                permission_events,
            )?;
            remaining.observe_ordinary(
                program,
                &permission_events[retirement_start..],
                &facts.flow.ownership.segments,
                cleanup_dispositions,
                checked_trees::FlowClaimJoinExit::default(),
                &[],
                false,
            );
            remaining.retire_storage(
                root,
                machine_symbol,
                state_symbol,
                statement_source,
                cleanup_dispositions,
            );
        }
    }
    let evaluated_events = permission_events.len() - event_start;
    // Calls have already joined their branch-local custody. Replaying their
    // flattened event roster here would import transfers from sibling arms.
    let replacement_custody = remaining.clone();
    if written_targets.is_empty()
        && let Some(mut establishment) = projected_affine::append_replacement(
            program,
            facts,
            machine_symbol,
            state_symbol,
            statement_index,
            statement,
            places,
            &replacement_custody,
            permission_events,
        )
    {
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
    )?;
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
        &permission_events[checked_through..],
        &facts.flow.ownership.segments,
    )?;
    Ok(ProducedStorage {
        establishments: storage_establishments,
        evaluated_events,
    })
}

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
        if (place.multiplicity == Multiplicity::Affine
            && (place.live || replacement_custody.is_reserved(target.root)))
            || replaces_zero_image
        {
            for path in replacement_custody.disposal_paths(target.root, &place.path) {
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
        place.live = obligation_live || place.multiplicity == Multiplicity::Affine;
        place.case_excluded = place.conditional && !obligation_live;
        // A linear local's obligation-free whole write is its zero image:
        // affine storage, not an established value (mirrors `applied_events`).
        place.zero_image = place.multiplicity == Multiplicity::Linear
            && !obligation_live
            && place.path.is_empty()
            && !place.conditional;
        place.ever_established |= !place.zero_image;
        place.claim_identity = Some(claim_identity);
        place.provenance = Some(
            provenance
                .unwrap_or_else(|| established_provenance(machine_symbol, state_symbol, source)),
        );
        permission_events.push(FlowPermissionEventFact {
            expression: target.root.as_expression().unwrap_or_default(),
            evaluation: checked_trees::FlowPermissionEvaluation::Statement,
            machine_symbol,
            state_symbol,
            source,
            kind: PermissionEventKind::Establish,
            multiplicity: place.multiplicity,
            access: PermissionAccess::Owned,
            claim_identity,
            provenance: place
                .provenance
                .expect("an established place has explicit provenance"),
            root: target.root,
            segments,
            obligation_live,
        });
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
