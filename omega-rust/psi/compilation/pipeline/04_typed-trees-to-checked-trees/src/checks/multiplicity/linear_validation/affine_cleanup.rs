//! Scope-exit permission events for eligible owned cleanup.

use crate::checks::multiplicity::linear_obligations::LinearPlace;
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
    observed_temporaries_by_statement: &[Vec<(
        typed_trees::expression::ExpressionHandle,
        TypeReferenceHandle,
    )>],
    state: &typed_trees::state::State,
    machine_symbol: SymbolHandle,
    coordinate: FlowClaimJoinExit,
    tracked_places: &[LinearPlace],
    remaining: &super::super::cleanup_dispositions::RemainingCustody,
    segments: &mut arena::PagedArenaSet<facts::PlaceSegment>,
    permission_events: &mut Vec<FlowPermissionEventFact>,
) {
    visit_owned_cleanup_events(
        program,
        observed_temporaries_by_statement,
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
    observed_temporaries_by_statement: &[Vec<(
        typed_trees::expression::ExpressionHandle,
        TypeReferenceHandle,
    )>],
    state: &typed_trees::state::State,
    machine_symbol: SymbolHandle,
    coordinate: FlowClaimJoinExit,
    tracked_places: &[LinearPlace],
    remaining: &super::super::cleanup_dispositions::RemainingCustody,
    mut emit: impl FnMut(FlowPermissionEventFact, Vec<facts::PlaceSegment>),
) {
    let evaluation = exit_evaluation(program, state, coordinate);
    let mut append = |root: facts::PlaceRoot, type_reference: TypeReferenceHandle| {
        // Availability records a descriptor occurrence, not an independent
        // cleanup owner. Its exact loan cohort owns direct-reference release.
        if validation::reference_shell(program, type_reference).is_some() {
            return;
        }
        let tracked_root = tracked_places
            .iter()
            .find(|place| place.root == root && place.path.is_empty());
        if let Some(place) = tracked_root {
            if !place.live && !remaining.has_storage_frontier(root) {
                return;
            }
        } else if validation::reference_result_custody::result_multiplicity(program, type_reference)
            == Multiplicity::Unrestricted
        {
            // Loan-release scheduling owns direct reference roots, including
            // local descriptors and borrowed formals. An owned aggregate
            // instead retires its stored carriers, even when shared access
            // permits repeated source-level copies.
            return;
        }
        // A transparent owner has contained claims rather than a fabricated
        // aggregate claim. Once those claims have left, its remaining home
        // still needs an ordinary exit disposition with the actual transfers
        // excluded. A whole-owner transfer, unlike a projection transfer,
        // leaves no source home to retire.
        if !remaining.is_present(root, &[]) && !remaining.has_storage_frontier(root) {
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
            visit_owned_region_cleanup_events(
                program,
                reference,
                root,
                &path,
                tracked_places,
                remaining,
                FlowPermissionEventFact {
                    expression: Default::default(),
                    evaluation,
                    machine_symbol,
                    state_symbol: state.symbol,
                    source: PermissionEventSource::StateExit,
                    kind: PermissionEventKind::AffineDrop,
                    multiplicity: Multiplicity::Affine,
                    access: PermissionAccess::Owned,
                    claim_identity: PermissionClaimIdentity::Unknown,
                    provenance: PermissionProvenance::Unknown,
                    root,
                    segments: Default::default(),
                    obligation_live: false,
                },
                &mut emit,
            );
        }
    };

    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .rev()
    {
        if let Some(local) = statement.as_local_data()
            && !local.relevance.is_erased()
        {
            append(facts::PlaceRoot::Symbol(local.symbol), local.type_reference);
        }
        // A saved expression is an owner too. It completed before this
        // statement's receiving local and after owners in earlier statements.
        // Only established roots participate; a reserved sibling-arm result
        // cannot acquire cleanup authority on this exit.
        if let Some(observed) = observed_temporaries_by_statement.get(statement_index) {
            for &(expression, reference) in observed.iter().rev() {
                let root = facts::PlaceRoot::Expression(expression);
                if tracked_places
                    .iter()
                    .any(|place| place.root == root && place.ever_established)
                {
                    append(root, reference);
                }
            }
        }
    }
    for parameter in program
        .state_parameters(state)
        .iter()
        .rev()
        .filter(|parameter| !parameter.relevance.is_erased())
    {
        append(
            facts::PlaceRoot::Symbol(parameter.symbol),
            parameter.type_reference,
        );
    }
}

/// One exact owned region uses the same eligibility and claim receipts at a
/// normal exit or replacement, including a retained skipped-transfer region.
/// Returns false when that region has no eligible cleanup; an eligible
/// unrestricted region may succeed without emitting an action.
pub(crate) fn append_owned_region_cleanup_permission_events(
    program: &dyn validation::CheckingInput,
    reference: TypeReferenceHandle,
    root: facts::PlaceRoot,
    path: &[facts::PlaceSegment],
    tracked_places: &[LinearPlace],
    remaining: &super::super::cleanup_dispositions::RemainingCustody,
    template: FlowPermissionEventFact,
    segments: &mut arena::PagedArenaSet<facts::PlaceSegment>,
    permission_events: &mut Vec<FlowPermissionEventFact>,
) -> bool {
    visit_owned_region_cleanup_events(
        program,
        reference,
        root,
        path,
        tracked_places,
        remaining,
        template,
        |mut event, path| {
            event.segments = segments.insert_many(path);
            permission_events.push(event);
        },
    )
}

fn visit_owned_region_cleanup_events(
    program: &dyn validation::CheckingInput,
    reference: TypeReferenceHandle,
    root: facts::PlaceRoot,
    path: &[facts::PlaceSegment],
    tracked_places: &[LinearPlace],
    remaining: &super::super::cleanup_dispositions::RemainingCustody,
    template: FlowPermissionEventFact,
    mut emit: impl FnMut(FlowPermissionEventFact, Vec<facts::PlaceSegment>),
) -> bool {
    let region_claim = tracked_places
        .iter()
        .find(|place| place.root == root && place.path == path && place.live);
    let multiplicity = region_claim.map_or_else(
        || match validation::reference_result_custody::result_multiplicity(program, reference) {
            Multiplicity::Unrestricted => Multiplicity::Unrestricted,
            // Transparent arrays inherit element multiplicity, but
            // only an exact region claim can authorize Consume.
            Multiplicity::Affine | Multiplicity::Linear => Multiplicity::Affine,
        },
        |place| place.multiplicity,
    );
    if multiplicity == Multiplicity::Unrestricted {
        return true;
    }
    let Some(receipts) = super::super::owned_cleanup::claim_receipts(
        program,
        reference,
        root,
        path,
        tracked_places,
        &remaining.removed_paths(program, root, path),
        &remaining.inactive_case_paths(root, path),
    ) else {
        return false;
    };
    let event = FlowPermissionEventFact {
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
        obligation_live: false,
        ..template
    };
    emit(event.clone(), path.to_vec());
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
        emit(receipt, place.path.to_vec());
    }
    true
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
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    cleanup: &checked_trees::ContextualCleanupFacts,
    places: &mut [LinearPlace],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut expected = Vec::new();
    let evaluation = exit_evaluation(program, state, coordinate);
    for handle in cleanup.state_disposition_handles(machine_symbol, state.symbol) {
        let disposition = cleanup.dispositions.get(handle);
        if disposition.source != PermissionEventSource::StateExit
            || disposition.exit != coordinate
            || cleanup.is_selected_disposition(handle)
        {
            continue;
        }
        let path = segments.span_or_empty(disposition.path);
        let root = disposition.root;
        let reference = validation::canonical_place_type_reference(
            program,
            state.symbol,
            coordinate.statement_index,
            &validation::CanonicalPlace {
                root,
                segments: path.to_vec(),
            },
        );
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
            let inactive_cases = cleanup
                .exclusions
                .span_or_empty(disposition.inactive_cases)
                .iter()
                .filter(|inactive| inactive.root == root)
                .filter_map(|inactive| {
                    cleanup
                        .segments
                        .span_or_empty(inactive.segments)
                        .strip_prefix(path)
                })
                .collect::<Vec<_>>();
            super::super::owned_cleanup::claim_receipts(
                program,
                reference,
                root,
                path,
                places,
                &removed,
                &inactive_cases,
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

pub(crate) fn exit_evaluation(
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
