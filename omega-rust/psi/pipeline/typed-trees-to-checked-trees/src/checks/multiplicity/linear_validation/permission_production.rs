//! Statement permission production, moves and static case selection.

use crate::checks::multiplicity::linear_obligations::{ClaimIdentityAllocator, LinearPlace};
use crate::checks::multiplicity::linear_validation::claim_provenance::written_linear_targets;
use crate::checks::multiplicity::type_multiplicity::type_carries_linear_obligation;
use crate::checks::multiplicity::{projected_affine, temporary_results};
use crate::flow::FlowOwnershipEventSource;
use checked_trees::{CheckFacts, FlowPermissionEventFact};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_statement_permission_production(
    program: &typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    moves: &[crate::flow::DiscoveredMoveEvent],
    statement_index: usize,
    statement: &StatementNode,
    places: &mut [LinearPlace],
    permission_events: &mut Vec<FlowPermissionEventFact>,
    claim_identities: &mut ClaimIdentityAllocator,
    conditional_carrier_transfers: &mut Vec<usize>,
) {
    // Destructure coverage markers are proof-only reads synthesized by the
    // parser; they neither transfer a value nor establish user storage.
    if matches!(statement, StatementNode::LocalData(local) if local.name.as_str().starts_with("__arm_destructure#"))
    {
        return;
    }

    let written_targets =
        written_linear_targets(program, state_symbol, statement_index, statement, places);

    // Moves out of initializer/assignment sources happen before the
    // destination becomes established. The old move-only summary also
    // contains a production event *at* the destination; exclude that
    // compatibility event here rather than mistaking creation for use.
    let calls = facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == machine_symbol && state.state_symbol == state_symbol)
                .then(|| facts.flow.control.calls.span_or_empty(state.calls))
        })
        .unwrap_or_default()
        .to_vec();
    let mut statement_moves = moves
        .iter()
        .filter(|event| {
            !event.source_arm.is_valid()
                && event_statement_index(event.source) == Some(statement_index)
        })
        .collect::<Vec<_>>();
    // Call ordinals are authored preorder coordinates, not evaluation order.
    // Reuse the captured control-flow order: nested producers execute before
    // their consumer; an initializer's destination is established afterwards.
    statement_moves.sort_by_key(|event| match event.source {
        FlowOwnershipEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => calls
            .iter()
            .position(|call| {
                call.statement_index == statement_index
                    && call.call_ordinal == call_ordinal
                    && call.target_symbol == target_symbol
            })
            .unwrap_or(calls.len()),
        FlowOwnershipEventSource::Statement { .. } => calls.len(),
    });
    for event in statement_moves {
        if matches!(event.root, facts::PlaceRoot::Expression(_)) {
            temporary_results::append_whole_affine_transfer(
                program,
                facts,
                machine_symbol,
                state_symbol,
                &calls,
                event,
                permission_events,
            );
            continue;
        }
        let facts::PlaceRoot::Symbol(symbol) = event.root else {
            continue;
        };
        let event_path = facts
            .flow
            .ownership
            .segments
            .span_or_empty(event.segments)
            .to_vec();
        select_static_case_alternative(symbol, &event_path, places);
        if matches!(event.source, FlowOwnershipEventSource::Statement { .. })
            && written_targets.iter().any(|target| {
                target.root == event.root && target.destination_path.as_slice() == event_path
            })
        {
            continue;
        }
        let matching = places
            .iter()
            .enumerate()
            .filter_map(|(index, place)| {
                (place.symbol == symbol && move_selects_claim(&event_path, place)).then_some(index)
            })
            .collect::<Vec<_>>();
        if matching.is_empty() {
            projected_affine::append_transfer(
                program,
                facts,
                machine_symbol,
                state_symbol,
                event,
                &event_path,
                places,
                permission_events,
            );
            continue;
        }
        let kind = permission_kind_for_move(program, facts, machine_symbol, state_symbol, event);
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
            // This is a move of the carrier, not an extraction of its case
            // payload. A checked callee may later prove the payload absent.
            // Keep the occurrence until that correspondence is known; false
            // duplicate moves and explicit payload projections are not in
            // this list and must still reject during replay.
            if obligation_live
                && place.conditional
                && place
                    .path
                    .strip_prefix(event_path.as_slice())
                    .is_some_and(|suffix| {
                        suffix
                            .iter()
                            .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
                    })
            {
                conditional_carrier_transfers.push(permission_events.len());
            }
            permission_events.push(FlowPermissionEventFact {
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
    }

    for target in written_targets {
        let facts::PlaceRoot::Symbol(symbol) = target.root else {
            continue;
        };
        let place_index = target.place_index;
        let obligation_live = target.obligation_live;
        let claim_identity = target.claim_identity.unwrap_or_else(|| {
            if obligation_live {
                {
                    claim_identities.mint(
                        machine_symbol,
                        state_symbol,
                        PermissionEventSource::Statement { statement_index },
                    )
                }
            } else {
                PermissionClaimIdentity::Unknown
            }
        });
        let provenance = target.provenance;
        let claim_path = places[place_index].path.clone();
        let segments = facts.flow.ownership.segments.insert_many(claim_path);
        let place = &mut places[place_index];
        debug_assert_eq!(place.symbol, symbol);
        // Replacement settles the old affine value before establishing the
        // new one. A source transferred into the replacement call is already
        // dead here and must not also receive a caller-side drop.
        if place.live && place.multiplicity == Multiplicity::Affine {
            permission_events.push(FlowPermissionEventFact {
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
                root: facts::PlaceRoot::Symbol(symbol),
                segments,
                obligation_live: false,
            });
        }
        place.live = obligation_live || place.multiplicity == Multiplicity::Affine;
        place.case_excluded = place.conditional && !obligation_live;
        place.ever_established = true;
        place.claim_identity = Some(claim_identity);
        place.provenance = Some(provenance.unwrap_or_else(|| {
            established_provenance(
                machine_symbol,
                state_symbol,
                PermissionEventSource::Statement { statement_index },
            )
        }));
        permission_events.push(FlowPermissionEventFact {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::Statement { statement_index },
            kind: PermissionEventKind::Establish,
            multiplicity: place.multiplicity,
            access: PermissionAccess::Owned,
            claim_identity,
            provenance: place
                .provenance
                .expect("an established place has explicit provenance"),
            root: facts::PlaceRoot::Symbol(symbol),
            segments,
            obligation_live,
        });
    }
}

fn move_selects_claim(event_path: &[facts::PlaceSegment], claim: &LinearPlace) -> bool {
    claim.path.starts_with(event_path)
        || (claim.conditional && event_path.starts_with(claim.path.as_slice()))
}

pub(crate) fn select_static_case_alternative(
    symbol: SymbolHandle,
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
        if place.symbol != symbol
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

pub(crate) fn permission_kind_for_move(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    event: &crate::flow::DiscoveredMoveEvent,
) -> PermissionEventKind {
    let FlowOwnershipEventSource::Call {
        statement_index,
        call_ordinal,
        target_symbol,
    } = event.source
    else {
        return PermissionEventKind::Transfer;
    };
    let Some(call_site) = crate::semantic_calls::find_call_site(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        call_ordinal,
    ) else {
        return PermissionEventKind::Transfer;
    };
    let Some(target_state) = crate::semantic_calls::find_state(program, target_symbol) else {
        return PermissionEventKind::Transfer;
    };
    let arguments = crate::semantic_calls::call_site_argument_expressions(program, &call_site);
    let parameters = program.state_parameters(target_state);
    let event_segments = facts.flow.ownership.segments.span_or_empty(event.segments);
    if arguments.len() == parameters.len() {
        for (parameter, argument) in parameters.iter().zip(arguments) {
            if !parameter.is_self {
                continue;
            }
            let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                program,
                state_symbol,
                statement_index,
                *argument,
            ) else {
                continue;
            };
            if place.root == event.root && place.segments.as_slice() == event_segments {
                return if type_carries_linear_obligation(program, target_state.return_type) {
                    PermissionEventKind::Transfer
                } else {
                    PermissionEventKind::Consume
                };
            }
        }
    } else if let Some(place) = crate::flow::owned_method_receiver_place(
        program,
        state_symbol,
        statement_index,
        &call_site,
        parameters,
        SymbolHandle::invalid(),
    ) && place.root == event.root
        && place.segments.as_slice() == event_segments
    {
        return if type_carries_linear_obligation(program, target_state.return_type) {
            PermissionEventKind::Transfer
        } else {
            PermissionEventKind::Consume
        };
    }
    PermissionEventKind::Transfer
}
