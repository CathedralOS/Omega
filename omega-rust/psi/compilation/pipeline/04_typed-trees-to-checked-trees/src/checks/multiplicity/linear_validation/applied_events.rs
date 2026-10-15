//! Applying recorded state-entry, statement and borrow permission events to
//! the linear places of one state. The recording pass
//! (`permission_events.rs`) is the only producer of these events; this file
//! walks them to decide the linear ownership rules (use after transfer, moving
//! an unestablished value, overwriting a live linear value), never to record
//! them a second time.

use crate::checks::multiplicity::linear_obligations::{ClaimIdentityAllocator, LinearPlace};
use crate::checks::multiplicity::linear_validation::permission_production::established_provenance;
use checked_trees::{CheckFacts, FlowPermissionEventFact};
use diagnostics::Diagnostic;
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};
use symbols::SymbolHandle;

pub(crate) fn apply_recorded_state_entry_events(
    events: &[&FlowPermissionEventFact],
    segments: &arena::Arena<facts::PlaceSegment>,
    places: &mut [LinearPlace],
) {
    for event in events.iter().copied().filter(|event| {
        event.source == PermissionEventSource::StateEntry
            && event.kind == PermissionEventKind::Establish
    }) {
        let event_path = segments.span_or_empty(event.segments);
        let Some(place) = places
            .iter_mut()
            .find(|place| place.root == event.root && place.path.as_slice() == event_path)
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
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    evaluation: checked_trees::FlowPermissionEvaluation,
    events: &[&FlowPermissionEventFact],
    places: &mut [LinearPlace],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let segments = &facts.flow.ownership.segments;
    let routes = super::super::selected_evaluation::ExpressionRoutes::new(
        program,
        machine,
        state,
        statement_index,
        evaluation,
        &facts.operators,
    );
    let calls = facts
        .flow
        .control
        .state(machine.symbol, state.symbol)
        .map_or(&[][..], |flow| {
            facts.flow.control.calls.span_or_empty(flow.calls)
        });
    let mut selected = super::super::selected_evaluation::SelectedEvaluation::new();
    let mut current = places.to_vec();
    let join = |current: &mut Vec<LinearPlace>, sibling: &Vec<LinearPlace>| {
        super::super::selected_evaluation::join_places(current, sibling)
    };
    for event in events.iter().copied().filter(|event| {
        permission_event_statement_index(event.source) == Some(statement_index)
            && event.evaluation == evaluation
            && event.kind != PermissionEventKind::AffineDrop
    }) {
        if let Err(diagnostic) = selected.enter(routes.event(event, calls), &mut current, join) {
            diagnostics.push(diagnostic);
            return;
        }
        let event_path = segments.span_or_empty(event.segments);
        let Some(place) = current
            .iter_mut()
            .find(|place| place.root == event.root && place.path.as_slice() == event_path)
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
            PermissionEventKind::Establish | PermissionEventKind::Restore => {
                if event.kind == PermissionEventKind::Establish
                    && place.live
                    && place.multiplicity == Multiplicity::Linear
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "assignment would overwrite live linear value `{}`; consume or transfer the existing obligation first",
                        place.name
                    )));
                }
                if event.kind == PermissionEventKind::Restore && !place.ever_established {
                    diagnostics.push(Diagnostic::error(format!(
                        "exclusive call cannot restore unestablished owner `{}`",
                        place.name
                    )));
                }
                place.live = event.obligation_live || place.multiplicity == Multiplicity::Affine;
                place.case_excluded = place.conditional && !event.obligation_live;
                // A linear local's obligation-free whole write is its zero
                // image: the place holds affine storage but no established
                // value. Only the unconditional root qualifies; an inactive
                // case payload is established debt-free by the enclosing
                // literal and must stay movable through its sum.
                place.zero_image = place.multiplicity == Multiplicity::Linear
                    && !event.obligation_live
                    && place.path.is_empty()
                    && !place.conditional;
                place.ever_established |= !place.zero_image;
                place.claim_identity = Some(event.claim_identity);
                place.provenance = Some(event.provenance);
            }
            PermissionEventKind::AffineDrop => {}
        }
    }
    if let Err(diagnostic) = selected.enter(&[], &mut current, join) {
        diagnostics.push(diagnostic);
        return;
    }
    places.clone_from_slice(&current);
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
    program: &dyn ::validation::CheckingInput,
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

    facts.flow.ownership.borrow_claims = arena::Arena::default();
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
            facts
                .flow
                .ownership
                .borrow_claims
                .append(checked_trees::flow::FlowBorrowClaimFact {
                    loan: activation.loan,
                    claim_identity,
                });
            append_borrow_permission_event(
                program,
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
            let claim_identity = facts
                .flow
                .ownership
                .borrow_claims
                .iter()
                .filter_map(|(_, claim)| {
                    (claim.loan == weakening.loan).then_some(claim.claim_identity)
                })
                .last()
                .unwrap_or(PermissionClaimIdentity::Unknown);
            append_borrow_permission_event(
                program,
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
    program: &dyn ::validation::CheckingInput,
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
    let evaluation = match source {
        PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            ..
        } => program
            .machine_by_symbol(machine_symbol)
            .zip(program.state_by_symbol(state_symbol))
            .map(|(machine, state)| {
                crate::flow::call_permission_evaluation(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                )
            })
            .unwrap_or(checked_trees::FlowPermissionEvaluation::Unknown),
        _ => checked_trees::FlowPermissionEvaluation::Statement,
    };
    permission_events.push(FlowPermissionEventFact {
        expression: Default::default(),
        evaluation,
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
        root: loan.root,
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
