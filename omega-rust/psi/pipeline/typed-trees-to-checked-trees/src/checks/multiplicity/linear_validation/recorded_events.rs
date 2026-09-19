//! Replaying recorded state-entry, statement and borrow permission events.

use crate::checks::multiplicity::linear_obligations::{ClaimIdentityAllocator, LinearPlace};
use crate::checks::multiplicity::linear_validation::permission_production::established_provenance;
use checked_trees::{CheckFacts, FlowPermissionEventFact};
use diagnostics::Diagnostic;
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};
use symbols::SymbolHandle;

/// Reconstruct custody from source and prerequisite access facts, never from
/// the ownership rows being checked. Receipt replay alone cannot detect a
/// forged establishment and transfer that agree with one another but disagree
/// with the authored initializer. Reuse production's semantic derivation with
/// fresh output arenas; compare paths structurally rather than arena handles.
pub(super) fn validate_permission_source_replay(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    // The source, not a removable receipt/provenance marker, selects replay.
    // Only a bound call returning a linear frontier can publish this kind of
    // join. Its alternatives may be hidden behind named states or wrappers,
    // so do not guess ambiguity from the immediate callee's surface shape.
    let has_claim_result_call = program.machines().iter().any(|machine| {
        program.machine_states(machine).iter().any(|state| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    let expression = match statement {
                        typed_trees::statement::StatementNode::LocalData(local) => {
                            local.initial_value
                        }
                        typed_trees::statement::StatementNode::Assignment(assignment) => {
                            assignment.value
                        }
                        _ => return false,
                    };
                    if !expression.is_valid() {
                        return false;
                    }
                    let typed_trees::expression::ExpressionNode::Call(call) =
                        program.expression_table.expression(expression)
                    else {
                        return false;
                    };
                    crate::semantic_calls::find_state(program, call.target_symbol).is_some_and(
                        |target| {
                            super::super::type_multiplicity::type_carries_linear_obligation(
                                program,
                                target.return_type,
                            )
                        },
                    )
                })
        })
    });
    if !has_claim_result_call {
        return Ok(());
    }
    let mut reconstructed = CheckFacts {
        borrow: facts.borrow.clone(),
        operators: facts.operators.clone(),
        flow: checked_trees::FlowFacts {
            contexts: facts.flow.contexts.clone(),
            control: facts.flow.control.clone(),
            borrow_lifetimes: facts.flow.borrow_lifetimes.clone(),
            ..Default::default()
        },
        ..Default::default()
    };
    let call_frames = validation::CallFrameResolver::new(program);
    let incoming_guards = crate::checks::ranges::incoming_guards::IncomingGuardIndex::build(
        program,
        call_frames.as_ref(),
    );
    crate::checks::multiplicity::permission_events::record_permission_events_with_incoming_guards(
        program,
        &mut reconstructed,
        &incoming_guards,
    )?;
    let expected = &reconstructed.flow.ownership;
    let recorded = &facts.flow.ownership;
    let mut expected_events = expected.permissions.iter();
    let mut recorded_events = recorded.permissions.iter();
    loop {
        match (expected_events.next(), recorded_events.next()) {
            (None, None) => return Ok(()),
            (Some((_, expected_event)), Some((_, recorded_event))) => {
                let expected_path = expected.segments.span(expected_event.segments);
                let recorded_path = recorded.segments.span(recorded_event.segments);
                let mut expected_row = expected_event.clone();
                let mut recorded_row = recorded_event.clone();
                expected_row.segments = arena::HandleSpan::empty();
                recorded_row.segments = arena::HandleSpan::empty();
                if expected_path.is_some()
                    && expected_path == recorded_path
                    && expected_row == recorded_row
                {
                    continue;
                }
            }
            _ => {}
        }
        return Err(vec![Diagnostic::error(
            "permission events differ from source-derived claim establishment and transfer",
        )]);
    }
}

pub(crate) fn apply_recorded_state_entry_events(
    events: &[&FlowPermissionEventFact],
    segments: &arena::Arena<facts::PlaceSegment>,
    places: &mut [LinearPlace],
) {
    for event in events.iter().copied().filter(|event| {
        event.source == PermissionEventSource::StateEntry
            && event.kind == PermissionEventKind::Establish
    }) {
        let facts::PlaceRoot::Symbol(symbol) = event.root else {
            continue;
        };
        let event_path = segments.span_or_empty(event.segments);
        let Some(place) = places
            .iter_mut()
            .find(|place| place.symbol == symbol && place.path.as_slice() == event_path)
        else {
            continue;
        };
        place.live = event.obligation_live || place.multiplicity == Multiplicity::Affine;
        place.case_excluded = place.conditional && !event.obligation_live;
        place.ever_established = true;
        place.claim_identity = Some(event.claim_identity);
        place.provenance = Some(event.provenance);
    }
}

pub(crate) fn apply_recorded_statement_events(
    statement_index: usize,
    events: &[&FlowPermissionEventFact],
    segments: &arena::Arena<facts::PlaceSegment>,
    places: &mut [LinearPlace],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for event in events.iter().copied().filter(|event| {
        permission_event_statement_index(event.source) == Some(statement_index)
            && event.kind != PermissionEventKind::AffineDrop
    }) {
        let facts::PlaceRoot::Symbol(symbol) = event.root else {
            continue;
        };
        let event_path = segments.span_or_empty(event.segments);
        let Some(place) = places
            .iter_mut()
            .find(|place| place.symbol == symbol && place.path.as_slice() == event_path)
        else {
            continue;
        };
        match event.kind {
            PermissionEventKind::Transfer | PermissionEventKind::Consume => {
                if !place.ever_established {
                    diagnostics.push(Diagnostic::error(format!(
                        "linear value `{}` has not been established (implicit zero-fill creates no linear obligation); it cannot be moved here",
                        place.name
                    )));
                } else if !place.live {
                    let ownership = if place.multiplicity == Multiplicity::Affine {
                        "affine"
                    } else {
                        "linear"
                    };
                    diagnostics.push(Diagnostic::error(format!(
                        "{ownership} value `{}` was already transferred or consumed; it cannot be moved here",
                        place.name
                    )));
                } else if place
                    .claim_identity
                    .unwrap_or(PermissionClaimIdentity::Unknown)
                    != event.claim_identity
                    || place.provenance.unwrap_or(PermissionProvenance::Unknown) != event.provenance
                {
                    // Reconciliation rewrites an entire occurrence, including
                    // its establishment. A transfer must still name the exact
                    // claim currently held at this place; matching a forged
                    // outcome receipt cannot substitute another live source.
                    diagnostics.push(Diagnostic::error(format!(
                        "permission transfer for `{}` does not match its established claim identity and provenance",
                        place.name
                    )));
                } else {
                    place.live = false;
                }
            }
            PermissionEventKind::Establish => {
                if place.live && place.multiplicity == Multiplicity::Linear {
                    diagnostics.push(Diagnostic::error(format!(
                        "assignment would overwrite live linear value `{}`; consume or transfer the existing obligation first",
                        place.name
                    )));
                }
                place.live = event.obligation_live || place.multiplicity == Multiplicity::Affine;
                place.case_excluded = place.conditional && !event.obligation_live;
                place.ever_established = true;
                place.claim_identity = Some(event.claim_identity);
                place.provenance = Some(event.provenance);
            }
            PermissionEventKind::AffineDrop => {}
        }
    }
}

pub(crate) fn permission_event_statement_index(source: PermissionEventSource) -> Option<usize> {
    match source {
        PermissionEventSource::Statement { statement_index }
        | PermissionEventSource::Call {
            statement_index, ..
        } => Some(statement_index),
        PermissionEventSource::StateEntry | PermissionEventSource::StateExit => None,
    }
}

pub(crate) fn append_borrow_permission_events(
    facts: &mut CheckFacts,
    permission_events: &mut Vec<FlowPermissionEventFact>,
    claim_identities: &mut ClaimIdentityAllocator,
) {
    // Clone only the small state/span index so the ownership-segment arena can
    // be extended while the already-built borrow facts remain immutable.
    let states = facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state)| {
            (
                state.machine_symbol,
                state.state_symbol,
                state.borrow_activations,
                state.borrow_weakenings,
            )
        })
        .collect::<Vec<_>>();

    let mut loan_claim_identities = Vec::new();
    for (machine_symbol, state_symbol, activations, weakenings) in states {
        for activation in facts
            .flow
            .borrow_lifetimes
            .activations
            .span_or_empty(activations)
            .to_vec()
        {
            let claim_identity = claim_identities.mint(
                machine_symbol,
                state_symbol,
                permission_source_from_invalidation(activation.source),
            );
            loan_claim_identities.push((activation.loan, claim_identity));
            append_borrow_permission_event(
                facts,
                permission_events,
                machine_symbol,
                state_symbol,
                activation.loan,
                permission_source_from_invalidation(activation.source),
                PermissionEventKind::Establish,
                claim_identity,
            );
        }
        for weakening in facts
            .flow
            .borrow_lifetimes
            .weakenings
            .span_or_empty(weakenings)
            .to_vec()
        {
            let source = if weakening.reason == checked_trees::FlowBorrowWeakeningReason::StateExit
            {
                PermissionEventSource::StateExit
            } else {
                permission_source_from_invalidation(weakening.source)
            };
            let claim_identity = loan_claim_identities
                .iter()
                .rev()
                .find_map(|(loan, identity)| (*loan == weakening.loan).then_some(*identity))
                .unwrap_or(PermissionClaimIdentity::Unknown);
            append_borrow_permission_event(
                facts,
                permission_events,
                machine_symbol,
                state_symbol,
                weakening.loan,
                source,
                PermissionEventKind::Consume,
                claim_identity,
            );
        }
    }
}

fn append_borrow_permission_event(
    facts: &mut CheckFacts,
    permission_events: &mut Vec<FlowPermissionEventFact>,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    loan_handle: arena::Handle<checked_trees::BorrowLoanFact>,
    source: PermissionEventSource,
    kind: PermissionEventKind,
    claim_identity: PermissionClaimIdentity,
) {
    let loan = facts.borrow.loans.get(loan_handle).clone();
    let segments = facts
        .flow
        .ownership
        .segments
        .insert_many(facts.borrow.loan_segments(&loan).iter().copied());
    let (multiplicity, access) = match loan.kind {
        checked_trees::BorrowAccessKind::Read => {
            (Multiplicity::Unrestricted, PermissionAccess::Shared)
        }
        checked_trees::BorrowAccessKind::Mutable => {
            (Multiplicity::Affine, PermissionAccess::Exclusive)
        }
        checked_trees::BorrowAccessKind::WriteOnly => {
            // A write-only borrow is exclusive and therefore follows the same
            // affine use discipline as a mutable borrow. Its observation
            // restriction is validated separately.
            (Multiplicity::Affine, PermissionAccess::Exclusive)
        }
    };
    permission_events.push(FlowPermissionEventFact {
        machine_symbol,
        state_symbol,
        source,
        kind,
        multiplicity,
        access,
        claim_identity,
        provenance: established_provenance(
            machine_symbol,
            state_symbol,
            PermissionEventSource::Statement {
                statement_index: loan.statement_index,
            },
        ),
        root: facts::PlaceRoot::Symbol(loan.root_symbol),
        segments,
        obligation_live: false,
    });
}

fn permission_source_from_invalidation(
    source: checked_trees::FlowInvalidationSource,
) -> PermissionEventSource {
    match source {
        checked_trees::FlowInvalidationSource::Statement { statement_index } => {
            PermissionEventSource::Statement { statement_index }
        }
        checked_trees::FlowInvalidationSource::Call {
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
