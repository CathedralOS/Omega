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
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    machine_symbol: SymbolHandle,
    coordinate: FlowClaimJoinExit,
    tracked_places: &[LinearPlace],
    remaining: &super::super::cleanup_dispositions::RemainingCustody,
    segments: &mut Arena<facts::PlaceSegment>,
    permission_events: &mut Vec<FlowPermissionEventFact>,
) {
    visit_owned_cleanup_events(
        program,
        state,
        machine_symbol,
        coordinate,
        tracked_places,
        remaining,
        |mut event, path| {
            event.segments = segments.insert_many(path);
            permission_events.push(event);
        },
    );
}

fn visit_owned_cleanup_events(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    machine_symbol: SymbolHandle,
    coordinate: FlowClaimJoinExit,
    tracked_places: &[LinearPlace],
    remaining: &super::super::cleanup_dispositions::RemainingCustody,
    mut emit: impl FnMut(FlowPermissionEventFact, Vec<facts::PlaceSegment>),
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
        } else if type_multiplicity(program, type_reference) == Multiplicity::Unrestricted {
            return;
        }
        let root = facts::PlaceRoot::Symbol(symbol);
        // A transparent owner has contained claims rather than a fabricated
        // aggregate claim. Once those claims have left, its remaining home
        // still needs an ordinary exit disposition with the actual transfers
        // excluded. A whole-owner transfer, unlike a projection transfer,
        // leaves no source home to retire.
        if !remaining.is_present(root, &[]) && !remaining.is_reserved(root) {
            return;
        }
        let mut paths = remaining
            .cleanup_paths(root)
            .map(|paths| {
                paths
                    .map(<[facts::PlaceSegment]>::to_vec)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![Vec::new()]);
        if remaining.needs_empty_retirement(program, root) {
            paths.clear();
            paths.push(Vec::new());
        }
        for path in paths {
            let Some(reference) =
                validation::project_type_reference_from_segments(program, type_reference, &path)
            else {
                continue;
            };
            let region_claim = tracked_places
                .iter()
                .find(|place| place.root == root && place.path == path && place.live);
            let multiplicity = region_claim.map_or_else(
                || match type_multiplicity(program, reference) {
                    Multiplicity::Unrestricted => Multiplicity::Unrestricted,
                    // Transparent arrays inherit element multiplicity, but
                    // only an exact region claim can authorize Consume.
                    Multiplicity::Affine | Multiplicity::Linear => Multiplicity::Affine,
                },
                |place| place.multiplicity,
            );
            if multiplicity == Multiplicity::Unrestricted {
                continue;
            }
            let Some(receipts) = super::super::owned_cleanup::claim_receipts(
                program,
                reference,
                root,
                &path,
                tracked_places,
                &remaining.removed_paths(program, root, &path),
            ) else {
                continue;
            };
            let event = FlowPermissionEventFact {
                expression: Default::default(),
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
                    region_claim
                        .and_then(|place| place.claim_identity)
                        .unwrap_or(PermissionClaimIdentity::Unknown)
                } else {
                    PermissionClaimIdentity::Unknown
                },
                provenance: region_claim
                    .and_then(|place| place.provenance)
                    .unwrap_or(PermissionProvenance::Unknown),
                root,
                segments: Default::default(),
                obligation_live: false,
            };
            emit(event.clone(), path.clone());
            for place in receipts {
                if event.kind == PermissionEventKind::Consume && place.path == path {
                    continue;
                }
                let mut receipt = event.clone();
                receipt.kind = PermissionEventKind::Consume;
                receipt.multiplicity = Multiplicity::Linear;
                receipt.claim_identity = place
                    .claim_identity
                    .expect("eligible cleanup has exact claim");
                receipt.provenance = place
                    .provenance
                    .expect("eligible cleanup has exact provenance");
                emit(receipt, place.path.clone());
            }
        }
    };

    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .rev()
    {
        if let Some(local) = statement.as_local_data()
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
        append(parameter.symbol, parameter.type_reference);
    }
}

/// Reconstruct the selected exit from current custody before accepting its
/// recorded consumption. This keeps cleanup authority attached to the exact
/// source owner and claim rather than treating any StateExit consume as proof.
pub(crate) fn apply_recorded_cleanup_events(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    machine_symbol: SymbolHandle,
    coordinate: FlowClaimJoinExit,
    events: &[&FlowPermissionEventFact],
    segments: &Arena<facts::PlaceSegment>,
    cleanup: &checked_trees::ContextualCleanupFacts,
    places: &mut [LinearPlace],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut expected = Vec::new();
    let evaluation = exit_evaluation(program, state, coordinate);
    for (handle, disposition) in cleanup.dispositions.iter().filter(|(_, disposition)| {
        disposition.machine == machine_symbol
            && disposition.state == state.symbol
            && disposition.source == PermissionEventSource::StateExit
            && disposition.exit == coordinate
    }) {
        if cleanup
            .selected_dispositions
            .iter()
            .any(|(_, selected)| *selected == handle)
        {
            continue;
        }
        let path = segments.span_or_empty(disposition.path);
        let root = disposition.root;
        let Some(symbol) = root.as_symbol() else {
            continue;
        };
        let reference = program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == symbol)
            .map(|parameter| parameter.type_reference)
            .or_else(|| {
                program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .filter_map(|statement| statement.as_local_data())
                    .find(|local| local.symbol == symbol)
                    .map(|local| local.type_reference)
            })
            .and_then(|reference| {
                validation::project_type_reference_from_segments(program, reference, path)
            });
        let removed = cleanup
            .exclusions
            .span_or_empty(disposition.exclusions)
            .iter()
            .filter(|removed| removed.root == root)
            .filter_map(|removed| {
                cleanup
                    .segments
                    .span_or_empty(removed.segments)
                    .strip_prefix(path)
            })
            .collect::<Vec<_>>();
        let receipts = reference.and_then(|reference| {
            super::super::owned_cleanup::claim_receipts(
                program, reference, root, path, places, &removed,
            )
        });
        let Some(receipts) = receipts else {
            diagnostics.push(Diagnostic::error(
                "scope-exit cleanup lacks its exact owner-authorized residual claim plan",
            ));
            return;
        };
        for place in receipts {
            expected.push((
                FlowPermissionEventFact {
                    expression: Default::default(),
                    evaluation,
                    machine_symbol,
                    state_symbol: state.symbol,
                    source: PermissionEventSource::StateExit,
                    kind: PermissionEventKind::Consume,
                    multiplicity: Multiplicity::Linear,
                    access: PermissionAccess::Owned,
                    claim_identity: place
                        .claim_identity
                        .expect("eligible cleanup has exact claim"),
                    provenance: place
                        .provenance
                        .expect("eligible cleanup has exact provenance"),
                    root,
                    segments: Default::default(),
                    obligation_live: false,
                },
                place.path.clone(),
            ));
        }
    }
    let mut recorded = events.iter().copied().filter(|event| {
        event.source == PermissionEventSource::StateExit
            && event.kind == PermissionEventKind::Consume
            && event.evaluation == evaluation
    });
    if !expected.iter().all(|(event, path)| {
        let Some(recorded) = recorded.next() else {
            return false;
        };
        let mut canonical = recorded.clone();
        canonical.segments = Default::default();
        &canonical == event && segments.span_or_empty(recorded.segments) == path
    }) || recorded.next().is_some()
    {
        diagnostics.push(Diagnostic::error(
            "scope-exit cleanup differs from its exact owner-authorized claim disposition",
        ));
        return;
    }
    for (event, path) in expected {
        let place = places
            .iter_mut()
            .find(|place| {
                place.root == event.root
                    && place.path == path
                    && place.claim_identity == Some(event.claim_identity)
                    && place.provenance == Some(event.provenance)
            })
            .expect("reconstructed cleanup has its exact live source claim");
        place.live = false;
    }
}

pub(super) fn exit_evaluation(
    program: &dyn ::validation::CheckingInput,
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
