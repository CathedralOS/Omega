//! Scope-exit permission events for eligible owned cleanup.

use crate::checks::multiplicity::linear_obligations::LinearPlace;
use crate::checks::multiplicity::type_multiplicity::type_multiplicity;
use arena::Arena;
use checked_trees::{
    FlowClaimJoinExit, FlowClaimJoinExitKind, FlowPermissionEvaluation, FlowPermissionEventFact,
};
use diagnostics::Diagnostic;
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceHandle;

/// Discover owned cleanup directly from typed custody. Locals precede
/// parameters, each in reverse declaration order. Affine disposal does not
/// consume a claim; linear disposal names the exact live claim and is selected
/// only when the complete type-side owner plan is eligible. Its contextual row
/// still carries hook effects and requirements through ordinary admission.
pub(crate) fn append_owned_cleanup_permission_events(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    machine_symbol: SymbolHandle,
    coordinate: FlowClaimJoinExit,
    tracked_places: &[LinearPlace],
    remaining: &super::super::cleanup_dispositions::RemainingCustody,
    segments: &mut Arena<facts::PlaceSegment>,
    permission_events: &mut Vec<FlowPermissionEventFact>,
) {
    let evaluation = exit_evaluation(program, state, coordinate);
    let mut append = |symbol: SymbolHandle, type_reference: TypeReferenceHandle| {
        let tracked_root = tracked_places
            .iter()
            .find(|place| place.root == facts::PlaceRoot::Symbol(symbol) && place.path.is_empty());
        if let Some(place) = tracked_root {
            if !place.live && !remaining.is_reserved(facts::PlaceRoot::Symbol(symbol)) {
                return;
            }
            if place.multiplicity == Multiplicity::Linear
                && (!crate::flow::whole_cleanup_is_eligible(program, type_reference)
                    || place.claim_identity.is_none()
                    || place.claim_identity == Some(PermissionClaimIdentity::Unknown)
                    || place.provenance.is_none()
                    || place.provenance == Some(PermissionProvenance::Unknown))
            {
                return;
            }
        } else if tracked_places
            .iter()
            .any(|place| place.root == facts::PlaceRoot::Symbol(symbol))
            || type_multiplicity(program, type_reference) == Multiplicity::Unrestricted
        {
            return;
        }
        let multiplicity = tracked_root.map_or(Multiplicity::Affine, |place| place.multiplicity);
        let root = facts::PlaceRoot::Symbol(symbol);
        let paths = remaining
            .cleanup_paths(root)
            .map(|paths| {
                paths
                    .map(<[facts::PlaceSegment]>::to_vec)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![Vec::new()]);
        for path in paths {
            if multiplicity == Multiplicity::Affine
                && crate::flow::project_type_reference_from_segments(program, type_reference, &path)
                    .is_none_or(|reference| {
                        type_multiplicity(program, reference) != Multiplicity::Affine
                    })
            {
                continue;
            }
            permission_events.push(FlowPermissionEventFact {
                evaluation,
                machine_symbol,
                state_symbol: state.symbol,
                source: PermissionEventSource::StateExit,
                kind: if multiplicity == Multiplicity::Linear {
                    PermissionEventKind::Consume
                } else {
                    PermissionEventKind::AffineDrop
                },
                multiplicity,
                access: PermissionAccess::Owned,
                claim_identity: if multiplicity == Multiplicity::Linear {
                    tracked_root
                        .and_then(|place| place.claim_identity)
                        .unwrap_or(PermissionClaimIdentity::Unknown)
                } else {
                    PermissionClaimIdentity::Unknown
                },
                provenance: tracked_root
                    .and_then(|place| place.provenance)
                    .unwrap_or(PermissionProvenance::Unknown),
                root,
                segments: segments.insert_many(path),
                obligation_live: false,
            });
        }
    };

    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .rev()
    {
        if let StatementNode::LocalData(local) = statement
            && !local.relevance.is_erased()
        {
            append(local.symbol, local.type_reference);
        }
    }
    for parameter in program
        .state_parameters(state)
        .iter()
        .rev()
        .filter(|parameter| !parameter.relevance.is_erased())
    {
        // An owned `self` receiver's place roots at the machine symbol — the
        // same durable root `normalized_event_place_root` publishes for its
        // moves. Borrowed receivers keep their unrestricted reference type and
        // produce no cleanup event either way.
        let symbol = if false
            && !crate::checks::multiplicity::linear_obligations::type_reference_is_reference(
                program,
                parameter.type_reference,
            ) {
            machine_symbol
        } else {
            parameter.symbol
        };
        append(symbol, parameter.type_reference);
    }
}

/// Reconstruct the selected exit from current custody before accepting its
/// recorded consumption. This keeps cleanup authority attached to the exact
/// source owner and claim rather than treating any StateExit consume as proof.
pub(crate) fn apply_recorded_cleanup_events(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    machine_symbol: SymbolHandle,
    coordinate: FlowClaimJoinExit,
    events: &[&FlowPermissionEventFact],
    places: &mut [LinearPlace],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut expected = Vec::new();
    append_owned_cleanup_permission_events(
        program,
        state,
        machine_symbol,
        coordinate,
        places,
        &super::super::cleanup_dispositions::RemainingCustody::default(),
        &mut Arena::default(),
        &mut expected,
    );
    expected.retain(|event| event.kind == PermissionEventKind::Consume);
    let evaluation = exit_evaluation(program, state, coordinate);
    let mut recorded = events.iter().copied().filter(|event| {
        event.source == PermissionEventSource::StateExit
            && event.kind == PermissionEventKind::Consume
            && event.evaluation == evaluation
    });
    if !expected.iter().all(|event| recorded.next() == Some(event)) || recorded.next().is_some() {
        diagnostics.push(Diagnostic::error(
            "scope-exit cleanup differs from its exact owner-authorized claim disposition",
        ));
        return;
    }
    for event in expected {
        if event.kind == PermissionEventKind::Consume {
            let facts::PlaceRoot::Symbol(symbol) = event.root else {
                unreachable!()
            };
            let place = places
                .iter_mut()
                .find(|place| {
                    place.root == facts::PlaceRoot::Symbol(symbol) && place.path.is_empty()
                })
                .expect("reconstructed whole cleanup has its live source claim");
            place.live = false;
        }
    }
}

fn exit_evaluation(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    coordinate: FlowClaimJoinExit,
) -> FlowPermissionEvaluation {
    match coordinate.kind {
        FlowClaimJoinExitKind::TransitionTarget | FlowClaimJoinExitKind::TransitionContinuation => {
            let StatementNode::Transition(transition) = &program
                .statement_table
                .statements(state.statement_nodes)[coordinate.statement_index]
            else {
                unreachable!()
            };
            FlowPermissionEvaluation::TransitionTarget(
                if coordinate.kind == FlowClaimJoinExitKind::TransitionTarget {
                    transition.target
                } else {
                    transition.continuation
                },
            )
        }
        _ => FlowPermissionEvaluation::Statement,
    }
}
