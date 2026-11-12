/*
    Validating linear permission events statement by statement: the claim
    frontier, borrow events, permission production and affine cleanup.

    This file validates one machine. `claim_frontier.rs` builds the linear
    claim frontier, `applied_events.rs` applies recorded entry, statement
    and borrow events, `permission_production.rs` applies statement
    permission production, `claim_provenance.rs` names claim identities and
    provenance and `affine_cleanup.rs` derives and replays owner-authorized exit cleanup.
*/

mod affine_cleanup {
    /*
        Scope-exit permission events for eligible owned cleanup.
    */
    use crate::checks::multiplicity::linear_obligations::LinearPlace;
    use crate::flow::state_statements;
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

    /*
        Discover owned cleanup directly from typed custody. Locals precede
        parameters, each in reverse declaration order. Affine disposal does not
        consume a claim; linear disposal names the exact live claim and is selected
        only when the complete type-side owner plan is eligible. Its contextual row
        still carries hook effects and requirements through ordinary admission.
    */
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
            } else if validation::reference_result_custody::result_multiplicity(
                program,
                type_reference,
            ) == Multiplicity::Unrestricted
            {
                return;
            }
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
                let Some(reference) = validation::project_type_reference_from_segments(
                    program,
                    type_reference,
                    &path,
                ) else {
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

        for (statement_index, statement) in
            crate::flow::state_statements(program, state.statement_nodes)
                .iter()
                .enumerate()
                .rev()
        {
            if let Some(local) = statement.as_local_data()
                && !local.relevance.is_erased()
            {
                append(facts::PlaceRoot::Symbol(local.symbol), local.type_reference);
            }
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

    /*
        One exact owned region uses the same eligibility and claim receipts at a
        normal exit or replacement, including a retained skipped-transfer region.
        Returns false when that region has no eligible cleanup; an eligible
        unrestricted region may succeed without emitting an action.
    */
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

    /*
        Reconstruct the selected exit from current custody before accepting its
        recorded consumption. This keeps cleanup authority attached to the exact
        source owner and claim rather than treating any StateExit consume as proof.
    */
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
                    .map(|inactive| cleanup.segments.span_or_empty(inactive.segments))
                    .filter(|inactive| crate::flow::contains_observed_case(path, inactive))
                    .map(|inactive| &inactive[path.len()..])
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
            FlowClaimJoinExitKind::TransitionTarget
            | FlowClaimJoinExitKind::TransitionContinuation => {
                let StatementNode::Transition(transition) =
                    &state_statements(program, state.statement_nodes)[coordinate.statement_index]
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
}
mod applied_events {

    /*
        Applying recorded state-entry, statement and borrow permission events to
        the linear places of one state. The recording pass
        (`permission_events.rs`) is the only producer of these events; this file
        walks them to decide the linear ownership rules (use after transfer, moving
        an unestablished value, overwriting a live linear value), never to record
        them a second time.
    */
    use super::super::instruction_dispositions::valid_receipt;
    use super::super::selected_evaluation::ExpressionRoutes;
    use super::super::selected_evaluation::SelectedEvaluation;
    use super::super::selected_evaluation::place_bindings::SelectionSite;
    use super::super::selected_evaluation::place_bindings::join_recorded_places;
    use super::affine_cleanup::exit_evaluation;
    use crate::checks::multiplicity::linear_obligations::{ClaimIdentityAllocator, LinearPlace};
    use crate::checks::multiplicity::linear_validation::permission_production::established_provenance;
    use crate::flow::call_permission_evaluation;
    use checked_trees::{CheckFacts, FlowPermissionEventFact as EventFact};
    use diagnostics::Diagnostic;
    use language_semantics::{
        Multiplicity, PermissionAccess, PermissionClaimIdentity as ClaimIdentity,
        PermissionEventKind, PermissionEventSource, PermissionProvenance,
    };
    use symbols::SymbolHandle;

    pub(crate) fn apply_recorded_state_entry_events(
        events: &[&EventFact],
        segments: &arena::PagedArenaSet<facts::PlaceSegment>,
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
        events: &[&EventFact],
        places: &mut [LinearPlace],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !events.iter().any(|event| {
            permission_event_statement_index(event.source) == Some(statement_index)
                && event.evaluation == evaluation
                && event.kind != PermissionEventKind::AffineDrop
        }) {
            return;
        }
        let segments = &facts.flow.ownership.segments;
        let routes = ExpressionRoutes::new(
            program,
            machine,
            state,
            statement_index,
            evaluation,
            &facts.operators,
            &facts.flow.control,
        );
        let calls = facts
            .flow
            .control
            .state(machine.symbol, state.symbol)
            .map_or(&[][..], |flow| {
                facts.flow.control.calls.span_or_empty(flow.calls)
            });
        let mut selected = SelectedEvaluation::new();
        let mut current = places.to_vec();
        let site = SelectionSite {
            machine: machine.symbol,
            state: state.symbol,
            statement: statement_index,
            evaluation,
        };
        let join = |branch, current: &mut Vec<LinearPlace>, sibling: &Vec<LinearPlace>| {
            join_recorded_places(&facts.flow.ownership, site, branch, current, sibling)
        };
        for event in events.iter().copied().filter(|event| {
            permission_event_statement_index(event.source) == Some(statement_index)
                && event.evaluation == evaluation
                && event.kind != PermissionEventKind::AffineDrop
        }) {
            if let Err(diagnostic) = selected.enter(
                routes.event(event, calls),
                &mut current,
                |_, _| Ok(()),
                join,
            ) {
                diagnostics.push(diagnostic);
                return;
            }
            if matches!(
                event.source,
                PermissionEventSource::InstructionDisposition { .. }
            ) && !valid_receipt(program, facts, machine, state, event)
            {
                diagnostics.push(Diagnostic::error(
                    "instruction disposition has no matching checked effect receipt",
                ));
                continue;
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
                    apply_recorded_consumption(event, place, diagnostics);
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
                    place.live =
                        event.obligation_live || place.multiplicity == Multiplicity::Affine;
                    place.case_excluded = place.conditional && !event.obligation_live;
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
        if let Err(diagnostic) = selected.enter(&[], &mut current, |_, _| Ok(()), join) {
            diagnostics.push(diagnostic);
            return;
        }
        places.clone_from_slice(&current);
    }

    fn apply_recorded_consumption(
        event: &EventFact,
        place: &mut LinearPlace,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
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
        } else if place.claim_identity.unwrap_or(ClaimIdentity::Unknown) != event.claim_identity
            || place.provenance.unwrap_or(PermissionProvenance::Unknown) != event.provenance
        {
            diagnostics.push(Diagnostic::error(format!(
                            "permission transfer for `{}` does not match its established claim identity and provenance: root {:?}, source {:?}, current {:?}/{:?}, transferred {:?}/{:?}",
                            place.name, event.root, event.source, place.claim_identity, place.provenance, event.claim_identity, event.provenance
                        )));
        } else {
            place.live = false;
        }
    }

    pub(crate) fn apply_recorded_exit_dispositions(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        coordinate: checked_trees::FlowClaimJoinExit,
        events: &[&EventFact],
        places: &mut [LinearPlace],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = PermissionEventSource::InstructionExitDisposition {
            statement_index: coordinate.statement_index,
        };
        let evaluation = exit_evaluation(program, state, coordinate);
        for event in events
            .iter()
            .copied()
            .filter(|event| event.source == source && event.evaluation == evaluation)
        {
            if !valid_receipt(program, facts, machine, state, event) {
                diagnostics.push(Diagnostic::error(
                    "instruction exit disposition has no matching checked effect receipt",
                ));
                continue;
            }
            let path = facts.flow.ownership.segments.span_or_empty(event.segments);
            if let Some(place) = places
                .iter_mut()
                .find(|place| place.root == event.root && place.path == path)
            {
                apply_recorded_consumption(event, place, diagnostics);
            }
        }
    }

    pub(crate) fn permission_event_statement_index(source: PermissionEventSource) -> Option<usize> {
        match source {
            PermissionEventSource::Statement { statement_index }
            | PermissionEventSource::InstructionDisposition { statement_index }
            | PermissionEventSource::Call {
                statement_index, ..
            } => Some(statement_index),
            PermissionEventSource::StateEntry
            | PermissionEventSource::StateExit
            | PermissionEventSource::InstructionExitDisposition { .. } => None,
        }
    }

    pub(crate) fn append_borrow_permission_events(
        program: &dyn ::validation::CheckingInput,
        facts: &mut checked_trees::CheckFactsWalk<'_>,
        permission_events: &mut Vec<EventFact>,
        claim_identities: &mut ClaimIdentityAllocator,
    ) {
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

        facts.flow.ownership.borrow_claims = Default::default();
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
                facts.flow.ownership.borrow_claims.append(
                    checked_trees::flow::FlowBorrowClaimFact {
                        loan: activation.loan,
                        claim_identity,
                    },
                );
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
                let source =
                    if weakening.reason == checked_trees::FlowBorrowWeakeningReason::StateExit {
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
                    .unwrap_or(ClaimIdentity::Unknown);
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
        facts: &mut checked_trees::CheckFactsWalk<'_>,
        permission_events: &mut Vec<EventFact>,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        loan_handle: arena::Handle<checked_trees::BorrowLoanFact>,
        source: PermissionEventSource,
        kind: PermissionEventKind,
        claim_identity: ClaimIdentity,
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
                    call_permission_evaluation(
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
        permission_events.push(EventFact {
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
}
mod case_selection_diagnostics {
    /*
        Explain an existing mixed-frontier rejection without selecting custody.
        Raw guard descriptors and authenticated completed backing coordinates are
        distinct observations. Reuse the production guard traversal and reaching-
        store/loan query; neither a matching label nor a missing claim proves absence.
    */
    use super::super::linear_obligations::LinearPlace;
    use super::super::permission_events::exact_positive_case_test;
    use super::super::permission_events::visit_positive_case_tests;
    use crate::published::places::place_at;
    use checked_trees::{CheckFacts, FlowClaimJoinExit, FlowClaimJoinExitKind};
    use facts::PlaceSegment;
    use symbols::SymbolHandle;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle};
    use typed_trees::state::State;
    use typed_trees::statement::{StatementNode, TransitionGuardNode};
    use validation::{CanonicalPlace, CheckingInput, has_exact_case_membership_meaning};

    pub(super) fn exit_detail(
        program: &dyn CheckingInput,
        facts: &CheckFacts,
        state: &State,
        coordinate: FlowClaimJoinExit,
        place: &LinearPlace,
    ) -> String {
        let Some(StatementNode::Transition(transition)) =
            crate::flow::state_statements(program, state.statement_nodes)
                .get(coordinate.statement_index)
        else {
            return String::new();
        };
        let positive = match coordinate.kind {
            FlowClaimJoinExitKind::TransitionTarget => true,
            FlowClaimJoinExitKind::TransitionContinuation => false,
            _ => return String::new(),
        };
        let TransitionGuardNode::When(guard) = transition.guard else {
            return "; case selection: unconditional guard has no case test".to_owned();
        };
        let mut rows = Vec::new();
        visit_positive_case_tests(program, state.symbol, guard, &mut |expression, test| {
            if rows.len() < 4 {
                rows.push(match test {
                    Some((subject, variant)) => selected_subject_detail(
                        program,
                        facts,
                        state,
                        coordinate.statement_index,
                        expression,
                        subject,
                        variant,
                        place,
                    ),
                    None => unavailable_membership_detail(program, state, expression),
                });
            }
        });
        let polarity = if positive {
            "selected guard"
        } else if exact_positive_case_test(program, state.symbol, guard).is_some() {
            "missed exact guard"
        } else {
            "missed guard is not one exact positive case test"
        };
        format!("; case selection ({polarity}): {}", rows.join(" | "))
    }

    fn unavailable_membership_detail(
        program: &dyn CheckingInput,
        state: &State,
        expression: ExpressionHandle,
    ) -> String {
        let label = program.expression_table.display_name(expression);
        let Some(binary) = program.expression_table.binary_node(expression) else {
            return format!("`{label}` has no binary case-membership operation");
        };
        if !matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::CaseMembership
        ) {
            return format!("`{label}` is not a positive case-membership comparison");
        }
        let authenticated = program
            .machine_holding_state(state.symbol)
            .is_some_and(|machine| {
                has_exact_case_membership_meaning(program, machine, Some(state), expression, binary)
            });
        if authenticated {
            format!("`{label}` authenticates membership but cannot resolve its applied case")
        } else {
            format!("`{label}` has unavailable case-membership authentication")
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn selected_subject_detail(
        program: &dyn CheckingInput,
        facts: &CheckFacts,
        state: &State,
        statement: usize,
        expression: ExpressionHandle,
        subject: ExpressionHandle,
        variant: SymbolHandle,
        place: &LinearPlace,
    ) -> String {
        let direct = place_at(program, state.symbol, statement, subject);
        let describe = |subject: &CanonicalPlace| {
            let relation = if subject.root != place.root
                || !place.path.starts_with(&subject.segments)
            {
                "authenticated unequal canonical subject"
            } else {
                match place.path.get(subject.segments.len()) {
                    Some(PlaceSegment::Case { variant: candidate }) if *candidate == variant => {
                        "compatible selected case"
                    }
                    Some(PlaceSegment::Case { .. }) => "other case under this subject",
                    _ => "subject does not guard this claim path",
                }
            };
            format!("`{}`: {relation}", subject.label(program))
        };
        let direct = direct.as_ref().map_or_else(
            || "canonical guard subject unavailable".to_owned(),
            &describe,
        );
        let Some(machine) = program.machine_holding_state(state.symbol) else {
            return format!(
                "case `{}`; {direct}; owning machine unavailable",
                program.symbols.name(variant)
            );
        };
        let completed = match crate::flow::captured_case_subject_result(
            program,
            facts.into(),
            machine,
            state,
            statement,
            expression,
        ) {
            Ok(captured) => {
                let source = facts.flow.control.operands.get(captured.source);
                place_at(
                    program,
                    state.symbol,
                    source.statement_index,
                    source.expression,
                )
                .as_ref()
                .map_or_else(
                    || "completed backing has no canonical source coordinate".to_owned(),
                    |source| format!("completed backing {}", describe(source)),
                )
            }
            Err(reason) => format!(
                "completed backing correspondence unavailable: {}",
                reason.reason()
            ),
        };
        format!(
            "case `{}`; {direct}; {completed}",
            program.symbols.name(variant)
        )
    }
}
mod claim_frontier {
    /*
        Initial linear places and the linear claim frontier.
    */
    use crate::checks::multiplicity::linear_obligations::type_reference_is_reference;
    use crate::checks::multiplicity::linear_obligations::{LinearClaimTemplate, LinearPlace};
    use crate::checks::multiplicity::linear_validation::permission_production::established_provenance;
    use crate::checks::multiplicity::type_multiplicity::{data_field_name, type_multiplicity};
    use facts::PlaceRoot;
    use facts::PlaceSegment;
    use language_semantics::{Multiplicity, PermissionEventSource};
    use symbols::SymbolHandle;
    use typed_trees::state::State;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
    use validation::owned_receiver_claims;
    use validation::peel_declared_constraints;
    use validation::reference_result_custody::binding_multiplicity;

    pub(crate) fn initial_linear_places(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    ) -> Vec<LinearPlace> {
        let mut places = Vec::new();
        for parameter in program.state_parameters(state) {
            let owned_receiver =
                false && !type_reference_is_reference(program, parameter.type_reference);
            let (claims, receiver_multiplicity) = if owned_receiver {
                let (multiplicity, claims) = owned_receiver_claims(program, machine_symbol);
                (
                    claims
                        .into_iter()
                        .map(|claim| LinearClaimTemplate {
                            path: claim.path,
                            type_reference: if claim.type_reference.is_valid() {
                                claim.type_reference
                            } else {
                                parameter.type_reference
                            },
                            multiplicity: Multiplicity::Linear,
                            conditional: claim.conditional,
                        })
                        .collect::<Vec<_>>(),
                    Some(multiplicity),
                )
            } else {
                (
                    linear_claim_frontier(program, parameter.type_reference),
                    None,
                )
            };
            let place_symbol = if owned_receiver {
                machine_symbol
            } else {
                parameter.symbol
            };
            if claims.is_empty()
                && if let Some(multiplicity) = receiver_multiplicity {
                    multiplicity == Multiplicity::Affine
                } else {
                    type_multiplicity(program, parameter.type_reference) == Multiplicity::Affine
                        && matches!(
                            program
                                .type_reference_table
                                .type_reference(peel_declared_constraints(
                                    program,
                                    parameter.type_reference,
                                )),
                            TypeReferenceNode::Named { .. }
                                | TypeReferenceNode::Generic { .. }
                                | TypeReferenceNode::FixedArray { .. }
                        )
                }
            {
                places.push(LinearPlace {
                    root: PlaceRoot::Symbol(place_symbol),
                    name: parameter.name.as_str().to_owned(),
                    path: Vec::new(),
                    multiplicity: Multiplicity::Affine,
                    claim_identity: None,
                    provenance: Some(established_provenance(
                        machine_symbol,
                        state_symbol,
                        PermissionEventSource::StateEntry,
                    )),
                    live: true,
                    ever_established: true,
                    zero_image: false,
                    conditional: false,
                    case_excluded: false,
                });
            }
            for claim in claims {
                places.push(LinearPlace {
                    root: PlaceRoot::Symbol(place_symbol),
                    name: claim_place_name(program, parameter.name.as_str(), &claim.path),
                    path: claim.path,
                    multiplicity: claim.multiplicity,
                    claim_identity: None,
                    provenance: Some(established_provenance(
                        machine_symbol,
                        state_symbol,
                        PermissionEventSource::StateEntry,
                    )),
                    live: true,
                    ever_established: true,
                    zero_image: false,
                    conditional: claim.conditional,
                    case_excluded: false,
                });
            }
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            let Some(local) = statement.as_local_data() else {
                continue;
            };
            let claims = linear_claim_frontier(program, local.type_reference);
            for claim in &claims {
                places.push(LinearPlace {
                    root: PlaceRoot::Symbol(local.symbol),
                    name: claim_place_name(program, local.name.as_str(), &claim.path),
                    path: claim.path.clone(),
                    multiplicity: claim.multiplicity,
                    claim_identity: None,
                    provenance: None,
                    live: false,
                    ever_established: false,
                    zero_image: false,
                    conditional: claim.conditional,
                    case_excluded: false,
                });
            }
            let type_node = program
                .type_reference_table
                .type_reference(peel_declared_constraints(program, local.type_reference));
            let declaration_established = local.initial_value.is_valid()
                || matches!(
                    type_node,
                    typed_trees::types::TypeReferenceNode::Named { .. }
                        | typed_trees::types::TypeReferenceNode::Generic { .. }
                        | typed_trees::types::TypeReferenceNode::FixedArray { .. }
                );
            if claims.is_empty()
                && binding_multiplicity(program, local.type_reference) == Multiplicity::Affine
                && declaration_established
            {
                places.push(LinearPlace {
                    root: PlaceRoot::Symbol(local.symbol),
                    name: local.name.as_str().to_owned(),
                    path: Vec::new(),
                    multiplicity: Multiplicity::Affine,
                    claim_identity: None,
                    provenance: None,
                    live: false,
                    ever_established: false,
                    zero_image: false,
                    conditional: false,
                    case_excluded: false,
                });
            }
        }
        places
    }

    /*
        The exact linear claim frontier of a type, enumerated by the shared
        validation walker so the checker's place roster and the lowering replay
        cannot diverge. Templates keep the checker's fields; every frontier claim
        is linear by construction and `conditional` rides the case-segment test.
    */
    pub(crate) fn linear_claim_frontier(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Vec<LinearClaimTemplate> {
        validation::linear_claim_frontier(program, type_reference)
            .into_iter()
            .map(|claim| LinearClaimTemplate {
                path: claim.path,
                type_reference: claim.type_reference,
                multiplicity: Multiplicity::Linear,
                conditional: claim.conditional,
            })
            .collect()
    }

    /*
        Reserve custody at a real producer root. No claim exists before the value
        completes; permission production preserves transferred identities or mints
        the genuinely new claims at that completion.
    */
    pub(crate) fn reserve_owner_places(
        program: &dyn ::validation::CheckingInput,
        root: PlaceRoot,
        type_reference: TypeReferenceHandle,
        places: &mut Vec<LinearPlace>,
    ) {
        if places.iter().any(|place| place.root == root) {
            return;
        }
        let name = program.normalized_type_identity_text(type_reference);
        let claims = linear_claim_frontier(program, type_reference);
        if claims.is_empty()
            && binding_multiplicity(program, type_reference)
                == language_semantics::Multiplicity::Affine
        {
            places.push(LinearPlace {
                root,
                name,
                path: Vec::new(),
                multiplicity: Multiplicity::Affine,
                claim_identity: None,
                provenance: None,
                live: false,
                ever_established: false,
                zero_image: false,
                conditional: false,
                case_excluded: false,
            });
        } else {
            for claim in claims {
                places.push(LinearPlace {
                    root,
                    name: claim_place_name(program, &name, &claim.path),
                    path: claim.path,
                    multiplicity: claim.multiplicity,
                    claim_identity: None,
                    provenance: None,
                    live: false,
                    ever_established: false,
                    zero_image: false,
                    conditional: claim.conditional,
                    case_excluded: false,
                });
            }
        }
    }

    fn claim_place_name(
        program: &dyn ::validation::CheckingInput,
        root: &str,
        path: &[PlaceSegment],
    ) -> String {
        let mut name = root.to_owned();
        for segment in path {
            match segment {
                PlaceSegment::Case { variant } => {
                    let case = program
                        .variant_owner(*variant)
                        .map(|(_, candidate)| candidate.name.as_str());
                    name.push_str("::");
                    name.push_str(case.unwrap_or("<case>"));
                }
                PlaceSegment::Field { symbol } => {
                    let field = data_field_name(program, *symbol);
                    name.push('.');
                    name.push_str(field.unwrap_or("<field>"));
                }
                PlaceSegment::FixedIndex { index } => {
                    name.push('[');
                    name.push_str(&index.to_string());
                    name.push(']');
                }
                PlaceSegment::FixedRange { start, end } => {
                    name.push('[');
                    name.push_str(&start.to_string());
                    name.push_str("..");
                    name.push_str(&end.to_string());
                    name.push(']');
                }
                PlaceSegment::Index { .. } => name.push_str("[<index>]"),
                PlaceSegment::OpaqueRange => name.push_str("[?..?]"),
            }
        }
        name
    }
}
mod claim_provenance {
    /*
        Written linear targets and the claim identities and provenance of
        permission expressions.
    */
    use super::super::construction_frontier::zero_image_inactive_case;
    use super::super::projected_affine::has_reference_prefix;
    use crate::checks::multiplicity::linear_obligations::{LinearPlace, WrittenLinearTarget};
    use crate::checks::multiplicity::type_multiplicity::{
        data_field_name, expression_establishes_obligation, literal_variant,
    };
    use crate::flow::{assignment_preserves_reference_binding, expressions};
    use crate::shared::collections::push_unique;
    use checked_trees::FlowPermissionEvaluation;
    use facts::PlaceRoot;
    use facts::PlaceSegment;
    use language_semantics::{Multiplicity, PermissionClaimIdentity, PermissionProvenance};
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::expression::ExpressionNode;
    use typed_trees::statement::{StatementNode, TransitionTargetNode};
    use validation::CanonicalPlace;
    use validation::canonical_place_type_reference;
    use validation::ownership_place_from_expression_in_state;
    use validation::ownership_source_expression;
    use validation::type_requires_establishment;

    /*
        Value expressions complete an owner whether spelled as a statement result
        or a selected transition target. Keep the target's evaluation identity so
        sibling arms cannot establish or transfer each other's custody.
    */
    pub(crate) fn completed_statement_values(
        program: &dyn ::validation::CheckingInput,
        statement: &StatementNode,
    ) -> Vec<(ExpressionHandle, FlowPermissionEvaluation)> {
        match statement {
            StatementNode::Expression(expression) => {
                vec![(*expression, FlowPermissionEvaluation::Statement)]
            }
            StatementNode::Transition(transition) => [transition.target, transition.continuation]
                .into_iter()
                .filter(|target| target.is_valid())
                .filter_map(
                    |target| match program.statement_table.transition_target(target) {
                        TransitionTargetNode::Value(expression) => Some((
                            *expression,
                            FlowPermissionEvaluation::TransitionTarget(target),
                        )),
                        _ => None,
                    },
                )
                .collect(),
            _ => Vec::new(),
        }
    }

    /*
        A receiving cut also completes stored values. The flow producer retains
        the exact evaluated RHS, so named destinations and assignments participate
        without rediscovering an initializer or replaying its expression.
    */
    pub(crate) fn completed_receiving_values(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_index: usize,
        statement: &StatementNode,
    ) -> Vec<(ExpressionHandle, FlowPermissionEvaluation)> {
        let mut values = completed_statement_values(program, statement);
        if let Some(stored) = facts
            .flow
            .control
            .state(machine, state)
            .and_then(|state| facts.flow.state_statement(state, statement_index))
            .and_then(|statement| {
                facts
                    .flow
                    .control
                    .operands
                    .is_valid(statement.stored_value)
                    .then(|| facts.flow.control.operands.get(statement.stored_value))
            })
            && stored.expression.is_valid()
        {
            let completed = (stored.expression, FlowPermissionEvaluation::Statement);
            push_unique(&mut values, completed);
        }
        values
    }

    pub(crate) fn written_linear_targets(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        statement: &StatementNode,
        evaluation: FlowPermissionEvaluation,
        places: &[LinearPlace],
    ) -> Vec<WrittenLinearTarget> {
        let (target, value) = match statement {
            StatementNode::LocalData(local) => {
                if !local.initial_value.is_valid() {
                    if type_requires_establishment(program, local.type_reference) {
                        return Vec::new();
                    }
                    return places
                        .iter()
                        .enumerate()
                        .filter(|(_, tracked)| {
                            tracked.root == PlaceRoot::Symbol(local.symbol)
                                && tracked.multiplicity != Multiplicity::Unrestricted
                                && (tracked.path.is_empty()
                                    || zero_image_inactive_case(
                                        program,
                                        local.type_reference,
                                        &tracked.path,
                                    )
                                    .is_some())
                        })
                        .map(|(place_index, _)| WrittenLinearTarget {
                            value: ExpressionHandle::invalid(),
                            root: PlaceRoot::Symbol(local.symbol),
                            destination_path: Vec::new(),
                            place_index,
                            obligation_live: false,
                            claim_identity: None,
                            provenance: None,
                            source_place: None,
                        })
                        .collect();
                }
                (
                    CanonicalPlace {
                        root: PlaceRoot::Symbol(local.symbol),
                        segments: Vec::new(),
                    },
                    local.initial_value,
                )
            }
            StatementNode::Assignment(assignment) => {
                let Some(place) = ownership_place_from_expression_in_state(
                    program,
                    state_symbol,
                    statement_index,
                    assignment.target,
                ) else {
                    return Vec::new();
                };
                if canonical_place_type_reference(program, state_symbol, statement_index, &place)
                    .is_some_and(|declared| {
                        assignment_preserves_reference_binding(program, assignment, declared)
                    })
                {
                    return Vec::new();
                }
                (place, assignment.value)
            }
            _ => {
                let Some((expression, _)) = completed_statement_values(program, statement)
                    .into_iter()
                    .find(|(_, source)| *source == evaluation)
                else {
                    return Vec::new();
                };
                let Some(place) = ownership_place_from_expression_in_state(
                    program,
                    state_symbol,
                    statement_index,
                    expression,
                ) else {
                    return Vec::new();
                };
                if place.root != PlaceRoot::Expression(expression) {
                    return Vec::new();
                }
                (place, expression)
            }
        };
        written_owner_targets(
            program,
            state_symbol,
            statement_index,
            &target,
            value,
            places,
        )
    }

    /*
        Establishment follows the completed value, not whether its destination has
        a source name. Both local writes and anonymous owners retain the same input
        identities, provenance and conditional claim frontier.
    */
    pub(crate) fn written_owner_targets(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        target: &CanonicalPlace,
        value: ExpressionHandle,
        places: &[LinearPlace],
    ) -> Vec<WrittenLinearTarget> {
        places
            .iter()
            .enumerate()
            .filter_map(|(place_index, tracked)| {
                if tracked.root != target.root
                    || !tracked.path.starts_with(target.segments.as_slice())
                {
                    return None;
                }
                let relative_path = &tracked.path[target.segments.len()..];
                let source_place = ownership_place_from_expression_in_state(
                    program,
                    state_symbol,
                    statement_index,
                    ownership_source_expression(program, state_symbol, statement_index, value),
                )
                .map(|mut source| {
                    source.segments.extend_from_slice(relative_path);
                    source
                });
                let borrowed_extraction = tracked.multiplicity == Multiplicity::Linear
                    && source_place.as_ref().is_some_and(|source| {
                        has_reference_prefix(program, state_symbol, statement_index, source)
                    });
                Some(WrittenLinearTarget {
                    value,
                    root: target.root,
                    destination_path: target.segments.clone(),
                    place_index,
                    source_place,
                    obligation_live: tracked.multiplicity != Multiplicity::Affine
                        && expression_establishes_obligation(
                            program,
                            state_symbol,
                            statement_index,
                            value,
                            relative_path,
                            places,
                        ),
                    claim_identity: (!borrowed_extraction)
                        .then(|| {
                            expression_permission_claim_identity_for_claim(
                                program,
                                state_symbol,
                                statement_index,
                                value,
                                relative_path,
                                places,
                            )
                        })
                        .flatten(),
                    provenance: (!borrowed_extraction)
                        .then(|| {
                            expression_permission_provenance_for_claim(
                                program,
                                state_symbol,
                                statement_index,
                                value,
                                relative_path,
                                places,
                            )
                        })
                        .flatten(),
                })
            })
            .collect()
    }

    /*
        The descent-and-match core both permission provenance walks share: peel a
        `Case`/`FixedIndex`/`Field` segment into the literal that produced it, then
        resolve the remaining expression to its canonical place and take the live
        places its path covers. `descend` is the caller's own recursive step,
        `claim` the per-walk field, `common` the unanimous-match fold.
    */
    #[allow(clippy::too_many_arguments)]
    fn expression_permission_claim_scan<X>(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
        relative_path: &[PlaceSegment],
        places: &[LinearPlace],
        descend: &dyn Fn(
            &dyn ::validation::CheckingInput,
            SymbolHandle,
            usize,
            ExpressionHandle,
            &[PlaceSegment],
            &[LinearPlace],
        ) -> Option<X>,
        claim: fn(&LinearPlace) -> Option<X>,
        common: &dyn Fn(Vec<X>) -> Option<X>,
    ) -> Option<X> {
        let source =
            ownership_source_expression(program, state_symbol, statement_index, expression);
        if source != expression {
            return descend(
                program,
                state_symbol,
                statement_index,
                source,
                relative_path,
                places,
            );
        }
        if let ExpressionNode::StructLiteral(literal) =
            program.expression_table.expression(expression)
            && let Some(PlaceSegment::Case { variant }) = relative_path.first()
        {
            if literal_variant(program, literal).map(|candidate| candidate.symbol) != Some(*variant)
            {
                return None;
            }
            return descend(
                program,
                state_symbol,
                statement_index,
                expression,
                &relative_path[1..],
                places,
            );
        }

        if let ExpressionNode::ArrayLiteral(values) =
            program.expression_table.expression(expression)
            && let Some(PlaceSegment::FixedIndex { index }) = relative_path.first()
        {
            let value = *expressions(program)
                .expression_handles(*values)
                .get(*index)?;
            return descend(
                program,
                state_symbol,
                statement_index,
                value,
                &relative_path[1..],
                places,
            );
        }

        if let ExpressionNode::StructLiteral(literal) =
            program.expression_table.expression(expression)
            && let Some(PlaceSegment::Field { symbol }) = relative_path.first()
        {
            let field_name = data_field_name(program, *symbol)?;
            let field = expressions(program)
                .struct_fields(literal.fields)
                .iter()
                .find(|field| field.name.as_str() == field_name)?;
            return descend(
                program,
                state_symbol,
                statement_index,
                field.value,
                &relative_path[1..],
                places,
            );
        }

        if !relative_path.is_empty()
            && matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Call(_)
            )
        {
            return None;
        }

        let source = ownership_place_from_expression_in_state(
            program,
            state_symbol,
            statement_index,
            expression,
        )?;
        let mut source_path = source.segments;
        source_path.extend_from_slice(relative_path);
        let matches = places
            .iter()
            .filter(|place| place.root == source.root && place.live && place.path == source_path)
            .filter_map(claim)
            .collect();
        common(matches)
    }

    fn expression_permission_claim_identity_for_claim(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
        relative_path: &[PlaceSegment],
        places: &[LinearPlace],
    ) -> Option<PermissionClaimIdentity> {
        if relative_path.is_empty()
            && matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Call(_)
            )
        {
            return None;
        }

        expression_permission_claim_scan(
            program,
            state_symbol,
            statement_index,
            expression,
            relative_path,
            places,
            &expression_permission_claim_identity_for_claim,
            |place| place.claim_identity,
            &|matches: Vec<_>| common_permission_claim_identity(matches.into_iter()),
        )
    }

    fn expression_permission_provenance_for_claim(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
        relative_path: &[PlaceSegment],
        places: &[LinearPlace],
    ) -> Option<PermissionProvenance> {
        if program.expression_table.call_node(expression).is_some() {
            return None;
        }
        expression_permission_claim_scan(
            program,
            state_symbol,
            statement_index,
            expression,
            relative_path,
            places,
            &expression_permission_provenance_for_claim,
            |place| place.provenance,
            &|matches: Vec<_>| common_permission_provenance(matches.into_iter()),
        )
    }

    fn common_permission_provenance(
        mut origins: impl Iterator<Item = PermissionProvenance>,
    ) -> Option<PermissionProvenance> {
        let first = origins.next()?;
        origins.all(|origin| origin == first).then_some(first)
    }

    fn common_permission_claim_identity(
        mut identities: impl Iterator<Item = PermissionClaimIdentity>,
    ) -> Option<PermissionClaimIdentity> {
        let first = identities.next()?;
        identities
            .all(|identity| identity == first)
            .then_some(first)
    }
}
pub(crate) mod loan_completions {
    /*
        Normal exclusive calls return storage, not the old value's identity. The
        shared mutation owner supplies exact storage writes and the signature's
        exclusive authority ceiling, including retained aliases and carried loans.
        Completion publishes the possible claim paths of the returned region;
        actual stored case guards select its live subset during execution.
    */
    use super::super::linear_obligations::{ClaimIdentityAllocator, LinearPlace};
    use checked_trees::{FlowCallFact, FlowLoanCompletionFact, FlowPermissionEventFact};
    use language_semantics::{
        Multiplicity, PermissionAccess, PermissionEventKind, PermissionEventSource,
    };
    use symbols::SymbolHandle;

    pub(crate) fn regions_for_calls(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        machine: SymbolHandle,
        state: SymbolHandle,
        calls: &[FlowCallFact],
        frames: Option<&validation::CallFrameResolver<'_>>,
        summaries: &crate::flow::StateMutationSummaryCache,
    ) -> Result<
        Vec<(PermissionEventSource, Vec<validation::CanonicalPlace>)>,
        diagnostics::Diagnostic,
    > {
        calls
            .iter()
            .map(|call| {
                let source = PermissionEventSource::Call {
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                    target_symbol: call.target_symbol,
                };
                let caller = program
                    .machine_by_symbol(machine)
                    .ok_or_else(|| diagnostics::Diagnostic::error("loan completion lost its caller"))?;
                if validation::terminal_callable_target(program, caller, call.target_symbol).is_none()
                    || validation::boundary_requirement_completion(program, call.target_symbol)
                        != semantic_vocabulary::BoundaryCompletion::NormalReturn
                {
                    return Ok((source, Vec::new()));
                }
                let borrowed = checked_trees::BorrowCallFact {
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                    authored_expression: call.authored_expression,
                    input_reference_loans: None,
                    reference_slot_effects: Default::default(),
                    completed_reference_loans: Default::default(),
                    receiver_symbol: call.receiver_symbol,
                    target_symbol: call.target_symbol,
                    has_receiver: call.has_receiver,
                    accesses: call.accesses,
                };
                let ceiling = crate::flow::linear_restoration_ceiling_regions(
                    program,
                    machine,
                    state,
                    Some(facts.borrow),
                    &borrowed,
                    frames,
                )
                .ok_or_else(|| {
                    diagnostics::Diagnostic::error(format!(
                        "exclusive call completion of `{}` in `{}` state `{}` at statement {} call {} has no representable caller storage authority",
                        program.symbols.display_path(call.target_symbol,"::"), caller.name,
                        program.state_by_symbol(state).map_or("<unknown>",|state| state.name.as_str()), call.statement_index,call.call_ordinal,
                    ))
                })?;
                if ceiling.is_empty() {
                    return Ok((source, Vec::new()));
                }
                let writes = crate::flow::call_mutated_regions(
                    program,
                    machine,
                    state,
                    facts.borrow,
                    &borrowed,
                    summaries,
                    frames,
                )
                .unwrap_or_else(|| ceiling.clone());
                let mut regions = Vec::new();
                for write in writes {
                    for authority in &ceiling {
                        let region = if authority.contains(&write) {
                            &write
                        } else if write.contains(authority) {
                            authority
                        } else {
                            continue;
                        };
                        let Some(mut region) = region.clone().into_declared() else {
                            continue;
                        };
                        for segment in &mut region.segments {
                            if let facts::PlaceSegment::Index { expression } = *segment
                                && let Some(index) = program
                                    .closed_integer_expression_value_in(expression, machine)
                                    .and_then(|value| value.to_u64())
                                    .and_then(|value| usize::try_from(value).ok())
                            {
                                *segment = facts::PlaceSegment::FixedIndex { index };
                            }
                        }
                        if let Some(unresolved) = region.segments.iter().position(|segment| {
                            !matches!(segment, facts::PlaceSegment::Field { .. }
                                | facts::PlaceSegment::Case { .. }
                                | facts::PlaceSegment::FixedIndex { .. })
                        }) {
                            region.segments.truncate(unresolved);
                            if authority.anchor().root != region.root
                                || !region.segments.starts_with(&authority.anchor().segments)
                            {
                                return Err(diagnostics::Diagnostic::error(
                                    "loan completion has no static region within its exclusive authority",
                                ));
                            }
                        }
                        if !regions.iter().any(|existing| existing == &region) {
                            regions.push(region);
                        }
                    }
                }
                let candidates = regions.clone();
                regions.retain(|region| {
                    !candidates.iter().any(|other| {
                        other != region && other.root == region.root
                            && region.segments.starts_with(&other.segments)
                    })
                });
                Ok((source, regions))
            })
            .collect()
    }

    pub(crate) fn restore(
        program: &dyn validation::CheckingInput,
        facts: &mut checked_trees::CheckFactsWalk<'_>,
        machine: SymbolHandle,
        state: SymbolHandle,
        source: PermissionEventSource,
        regions: &[validation::CanonicalPlace],
        places: &mut [LinearPlace],
        identities: &mut ClaimIdentityAllocator,
        events: &mut Vec<FlowPermissionEventFact>,
    ) -> Result<(), diagnostics::Diagnostic> {
        for region in regions {
            let selected = places
                .iter()
                .enumerate()
                .filter_map(|(position, place)| {
                    (place.root == region.root
                        && place.ever_established
                        && place.path.starts_with(&region.segments)
                        && place.multiplicity == Multiplicity::Linear)
                        .then_some(position)
                })
                .collect::<Vec<_>>();
            if selected.is_empty() {
                continue;
            }
            let PermissionEventSource::Call {
                statement_index, ..
            } = source
            else {
                return Err(diagnostics::Diagnostic::error(
                    "loan completion is not attached to a call",
                ));
            };
            let reference =
                validation::canonical_place_type_reference(program, state, statement_index, region)
                    .ok_or_else(|| {
                        diagnostics::Diagnostic::error(
                            "loan completion has no exact owned region type",
                        )
                    })?;
            facts
                .flow
                .ownership
                .loan_completions
                .insert(FlowLoanCompletionFact {
                    machine,
                    state,
                    source,
                    root: region.root,
                    segments: facts
                        .flow
                        .ownership
                        .segments
                        .insert_many(region.segments.iter().copied()),
                    multiplicity: program.type_multiplicity(reference),
                });
            for position in selected {
                let place = &mut places[position];
                let identity = identities.mint(machine, state, source);
                let provenance =
                    super::permission_production::established_provenance(machine, state, source);
                place.live = true;
                place.case_excluded = false;
                place.claim_identity = Some(identity);
                place.provenance = Some(provenance);
                events.push(FlowPermissionEventFact {
                    expression: Default::default(),
                    machine_symbol: machine,
                    state_symbol: state,
                    source,
                    evaluation: checked_trees::FlowPermissionEvaluation::Statement,
                    kind: PermissionEventKind::Restore,
                    multiplicity: place.multiplicity,
                    access: PermissionAccess::Owned,
                    claim_identity: identity,
                    provenance,
                    root: place.root,
                    segments: facts
                        .flow
                        .ownership
                        .segments
                        .insert_many(place.path.iter().copied()),
                    obligation_live: true,
                });
            }
        }
        Ok(())
    }
}
mod permission_production {

    /*
        Permission production follows the shared source occurrence inventory.
        Operand moves, invocation completion and anonymous result establishment
        happen before the next occurrence; only named receiving writes wait for
        the expression join. Grouping calls separately from statement moves would
        establish a temporary after its consuming branch had already reconverged.
        The emitted ledger retains exact producer identities for outcome mapping
        and the storage extent of each write for contextual cleanup.
    */
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
    use crate::checks::multiplicity::selected_evaluation::place_bindings::{
        SelectionEntry, SelectionSite, enter_produced,
    };
    use crate::checks::multiplicity::selected_evaluation::{ExpressionRoutes, SelectedEvaluation};
    use crate::checks::multiplicity::{
        projected_affine, projected_replacements, temporary_results,
    };
    use crate::checks::ownership_occurrences::{StatementOccurrence, StatementStep};
    use crate::flow::FlowOwnershipEventSource;
    use crate::flow::expressions;
    use crate::published::places::place_at;
    use checked_trees::FlowPermissionEventFact;
    use language_semantics::{
        Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
        PermissionEventSource, PermissionProvenance,
    };
    use symbols::SymbolHandle;
    use typed_trees::statement::StatementNode;

    mod observed_completions {
        /*
            A completed observation owns the normal death of its anonymous receiver,
            unless its actual result retains that home. This selects a completion only;
            the occurrence producer still disposes the exact RemainingCustody complement
            with ordinary claim receipts, before disposing its backing dependencies.
        */
        use super::{SourceCompletion, StatementOccurrence, StatementStep};
        use crate::checks::multiplicity::temporary_results::{self, TemporaryOwner};
        use crate::published::places::place_at;
        use checked_trees::FlowPermissionEvaluation;
        use symbols::SymbolHandle;

        /*
            Commit source death after the value's own consuming moves, before changing
            routes. A produced value and its receiving transfer are distinct occurrences.
        */
        #[allow(clippy::too_many_arguments)]
        pub(super) fn complete_at(
            program: &dyn validation::CheckingInput,
            facts: &mut checked_trees::CheckFactsWalk<'_>,
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
                    if let Some(place) = places.iter().find(|place| {
                        place.root == root
                            && place.multiplicity == Multiplicity::Linear
                            && place.live
                            && place.ever_established
                            && !place.case_excluded
                            && remaining.is_present(root, &place.path)
                    }) {
                        return Err(super::super::unconsumed_linear_value_diagnostic(
                            program,
                            machine_symbol,
                            state_symbol,
                            place,
                        )
                        .with_source_span(program.expression_table.source_span(expression)));
                    }
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
            facts: checked_trees::CheckFactsRef<'_>,
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
                let Some(place) = place_at(program, state, statement_index, receiver.expression)
                else {
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
                if observations.iter().any(|later| {
                    later.evaluation == evaluation
                        && facts.flow.control.operands.is_valid(later.receiver)
                        && place_at(program, state, statement_index, facts.flow.control.operands.get(later.receiver).expression).is_some_and(|later_place| later_place.root == place.root)
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
                    crate::flow::captured_lifetime_roots(program, facts.flow.control, result)
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
                    || validation::reference_result_custody::binding_multiplicity(
                        program,
                        type_reference,
                    ) == language_semantics::Multiplicity::Unrestricted
                {
                    continue;
                }
                let Some(alternatives) = crate::flow::completed_values::terminal_alternatives(
                    facts.flow.control,
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

        /*
            A value completes before its consuming moves in the shared occurrence
            schedule. Saved reads have no new producer completion, but their actual
            transfers still supply this cut. Never borrow a later, unrelated use.
        */
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
                || occurrences[first..=last].iter().skip(1).any(|occurrence| {
                    !own_move(occurrence) || occurrence.route != occurrences[first].route
                })
            {
                return None;
            }
            Some(last)
        }
    }
    mod projected_scopes {
        /*
            Anonymous projected owners die at their actual normal scope boundary.

            A branch-born call product cannot wait for statement-wide joined custody:
            that meet correctly forgets its establishment on skipped paths. Nor can it
            die at its first child move: later operands and held call arguments still
            own their evaluation order. Close each producing scope immediately before
            that scope's switch/join, after nested scopes have reconverged. Root-scope
            owners keep the ordinary statement completion; call-held owners keep their
            exact normal continuation. Cleanup still authenticates the full CURRENT
            residual partition, not a remembered path or stale Move-time snapshot.
        */
        use crate::checks::multiplicity::cleanup_dispositions::RemainingCustody;
        use crate::checks::multiplicity::linear_obligations::LinearPlace;
        use crate::checks::multiplicity::selected_evaluation::place_bindings::SelectionSite;
        use crate::checks::multiplicity::temporary_results::{self, TemporaryOwner};
        use crate::checks::ownership_occurrences::{
            EvaluationBranch, StatementOccurrence, StatementStep,
        };
        use checked_trees::{ContextualCleanupFacts, FlowOwnershipFacts, FlowPermissionEventFact};
        use diagnostics::Diagnostic;
        use language_semantics::{PermissionEventKind, PermissionEventSource};
        use typed_trees::expression::ExpressionHandle;

        struct ProjectedOwner {
            expression: ExpressionHandle,
            completion: ExpressionHandle,
            path: Vec<facts::PlaceSegment>,
            scope: Option<EvaluationBranch>,
            establishment: usize,
        }

        #[derive(Default)]
        pub(super) struct ProjectedScopes {
            owners: Vec<ProjectedOwner>,
        }

        impl ProjectedScopes {
            pub(super) fn observe_move(
                &mut self,
                occurrences: &[StatementOccurrence],
                held: &[TemporaryOwner],
                event: &crate::flow::DiscoveredMoveEvent,
                path: &[facts::PlaceSegment],
            ) -> Result<(), Diagnostic> {
                let Some(expression) = event.root.as_expression() else {
                    return Ok(());
                };
                if path.is_empty()
                    || held.iter().any(|owner| owner.expression == expression)
                    || self
                        .owners
                        .iter()
                        .any(|owner| owner.expression == expression)
                {
                    return Ok(());
                }
                let mut producers = occurrences.iter().enumerate().filter(|(_, occurrence)| {
                    matches!(occurrence.step, StatementStep::Complete(completed) if completed == expression)
                });
                let (Some((establishment, producer)), None) = (producers.next(), producers.next())
                else {
                    return Err(Diagnostic::error(
                        "projected temporary has no unique evaluated producer completion",
                    ));
                };
                self.owners.push(ProjectedOwner {
                    expression,
                    completion: event.expression,
                    path: path.to_vec(),
                    scope: producer.route.last().copied(),
                    establishment,
                });
                Ok(())
            }

            #[allow(clippy::too_many_arguments)]
            pub(super) fn retire_scope(
                &mut self,
                program: &dyn validation::CheckingInput,
                site: SelectionSite,
                scope: Option<EvaluationBranch>,
                places: &mut [LinearPlace],
                remaining: &mut RemainingCustody,
                ownership: &mut FlowOwnershipFacts,
                permissions: &mut Vec<FlowPermissionEventFact>,
                dispositions: &mut ContextualCleanupFacts,
            ) -> Result<(), Diagnostic> {
                self.owners
                    .sort_by_key(|owner| std::cmp::Reverse(owner.establishment));
                let mut position = 0;
                while position < self.owners.len() {
                    if self.owners[position].scope != scope {
                        position += 1;
                        continue;
                    }
                    let owner = self.owners.remove(position);
                    let root = facts::PlaceRoot::Expression(owner.expression);
                    if remaining.is_present(root, &owner.path)
                        || !remaining.is_present(root, &[])
                        || temporary_results::projectable_owner(
                            program,
                            site.state,
                            site.statement,
                            owner.expression,
                        )
                        .is_none()
                    {
                        continue;
                    }
                    let producer = permissions
                        .iter()
                        .rev()
                        .find(|event| {
                            event.kind == PermissionEventKind::Establish
                                && event.machine_symbol == site.machine
                                && event.state_symbol == site.state
                                && event.root == root
                        })
                        .map(|event| event.provenance)
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "projected temporary has no actual establishment at its normal scope exit",
                            )
                        })?;
                    let source = PermissionEventSource::Statement {
                        statement_index: site.statement,
                    };
                    let first = permissions.len();
                    temporary_results::retire_owner(
                        program,
                        ownership,
                        site.machine,
                        site.state,
                        temporary_results::ProjectedConsumer {
                            source,
                            evaluation: site.evaluation,
                            producer,
                        },
                        owner.expression,
                        &owner.path,
                        places,
                        remaining,
                        permissions,
                    )?;
                    let disposition_start = dispositions.dispositions.len();
                    remaining.observe_ordinary(
                        program,
                        &permissions[first..],
                        &ownership.segments,
                        dispositions,
                        checked_trees::FlowClaimJoinExit::default(),
                        &[],
                        false,
                    );
                    remaining.retire_storage(root, site.machine, site.state, source, dispositions);
                    let mut ordinal = 0;
                    dispositions.dispositions.for_each_mut(|_, disposition| {
                        if ordinal >= disposition_start {
                            disposition.completion = owner.completion;
                        }
                        ordinal += 1;
                    });
                }
                Ok(())
            }
        }
    }
    use projected_scopes::ProjectedScopes;

    pub(crate) struct ProducedStorage {
        pub(crate) binding_changes: Vec<StorageBindingChange>,
        pub(crate) establishments: Vec<StorageEstablishment>,
        pub(crate) transfers: Vec<TransferCustody>,
        /*
            Prefix already reflected in joined storage custody. Claim events alone
            cannot reconstruct whole-carrier moves that contain several claims.
        */
        pub(crate) evaluated_events: usize,
    }

    /*
        Expression evaluation has joined, but the receiving statement has not
        installed its value. Keep the actual transfer snapshots and pending targets
        across this cut: selection binding consumes evaluated arrivals, never a
        second walk of the source or the statement's entry custody.
    */
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

    /*
        Custody at an actual operand read, before its following move or reborrow.
        Reference bindings have ordinary reborrow usage and need not emit a value
        Transfer event. Their selected permission still needs the same post-effect
        arrival cut as an owned aggregate; an absent event is not an absent read.
    */
    pub(crate) struct ObservedCustody {
        /* Permission-event prefix at this actual read, before its following move. */
        pub(crate) cut: CustodyCut,
        pub(crate) expression: typed_trees::expression::ExpressionHandle,
        pub(crate) place: validation::CanonicalPlace,
        pub(crate) places: Vec<LinearPlace>,
        pub(crate) remaining: super::super::cleanup_dispositions::RemainingCustody,
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn evaluate_statement_permissions(
        program: &dyn ::validation::CheckingInput,
        facts: &mut checked_trees::CheckFactsWalk<'_>,
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
        conservation_candidates: &[crate::checks::content::ConservationCandidate],
    ) -> Result<Option<(EvaluatedStorage, ExpressionRoutes)>, diagnostics::Diagnostic> {
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
            /*
                A destination written from `producer() as T` receives the
                producer's own value through a value-preserving cast, exactly
                as `let x = producer()` does. The temporary is that write's
                source, not a second anonymous owner with its own obligation.
            */
            if written_targets.iter().any(|(written, _)| {
                (written.value == expression
                    || validation::ownership_source_expression(
                        program,
                        state_symbol,
                        statement_index,
                        written.value,
                    ) == expression)
                    && (written.root == target.root || !captured_source)
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
                    call.statement_index == statement_index
                        && call.authored_expression == expression
                })
            {
                *source = PermissionEventSource::Call {
                    statement_index,
                    call_ordinal: call.call_ordinal,
                    target_symbol: call.target_symbol,
                };
            }
        }
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
            facts.view(),
            machine_symbol,
            state_symbol,
            statement_index,
            evaluation,
            occurrences,
            temporary_owners,
        );
        observed_completions::append(
            program,
            facts.view(),
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
        let routes =
            ExpressionRoutes::from_steps(program, occurrences, facts.flow.control, selection_site);
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
                    SelectionEntry {
                        site: selection_site,
                        routes: &routes,
                    },
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
        macro_rules! complete_occurrence {
            ($ordinal:expr, $places:ident, $remaining:ident) => {
                observed_completions::complete_at(
                    program,
                    facts,
                    &source_completions,
                    occurrences,
                    moves,
                    temporary_owners,
                    $ordinal,
                    selection_site,
                    $places,
                    $remaining,
                    permission_events,
                    cleanup_dispositions,
                )?
            };
        }
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
                    retire_produced!(&occurrence.route);
                    let (places, remaining) = &mut custody;
                    if !transfers_custody {
                        complete_occurrence!(occurrence_ordinal, places, remaining);
                        continue;
                    }
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
                        complete_occurrence!(occurrence_ordinal, places, remaining);
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
                        facts.view(),
                        machine_symbol,
                        state_symbol,
                        event,
                        &event_path,
                        places,
                        remaining,
                        permission_events,
                    )?;
                    transfers[transfer_snapshot].events.end = permission_events.len() - event_start;
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
                        let obligation_live =
                            place.live && place.multiplicity == Multiplicity::Linear;
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
                    complete_occurrence!(occurrence_ordinal, places, remaining);
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
                            prior_value_transferred: establishment.prior_value_transferred,
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
                        facts.view(),
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
                            conservation_candidates,
                        );
                    }
                    complete_occurrence!(occurrence_ordinal, places, remaining);
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
        Ok(Some((
            EvaluatedStorage {
                binding_changes,
                transfers,
                observations,
                event_start,
                checked_through,
                written_targets,
                calls,
                projected_scopes,
            },
            routes,
        )))
    }

    /*
        Finish the receiving write only after all evaluated source arrivals have
        been retained. The selected-value binder may supply its exact receiving
        claims at this seam; ordinary destinations use the same pending targets.
    */
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn complete_statement_permissions(
        program: &dyn validation::CheckingInput,
        facts: &mut checked_trees::CheckFactsWalk<'_>,
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
        conservation_candidates: &[crate::checks::content::ConservationCandidate],
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
            facts.flow.ownership,
            permission_events,
            cleanup_dispositions,
        )?;
        let evaluated_events = permission_events.len() - event_start;
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
            facts.view(),
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
                    diagnostics::Diagnostic::error(
                        "receiving completion has no owning machine/state",
                    )
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
                conservation_candidates,
            );
        }
        if let Some((expression, _)) = completed_statement_values(program, statement)
            .into_iter()
            .find(|(_, source)| *source == evaluation)
            && (matches!(statement, StatementNode::Transition(_))
                || program.state_by_symbol(state_symbol).is_some_and(|state| {
                    statement_index + 1
                        == crate::flow::state_statements(program, state.statement_nodes).len()
                }))
        {
            for place in places.iter_mut().filter(|place| {
                place.root == facts::PlaceRoot::Expression(expression) && place.live
            }) {
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
        facts: &mut checked_trees::CheckFactsWalk<'_>,
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
        conservation_candidates: &[crate::checks::content::ConservationCandidate],
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
            conservation_candidates,
        );
        let transformation_start = permission_events.len();
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

    /*
        A copied leaf no longer uses its anonymous supplier's storage. End that
        supplier at the actual read completion, on the route that established it.
        Only descriptor-only, claim-free owners have no observable cleanup timing
        to preserve. Existing loans of the supplier's storage keep their own lifetime;
        loans stored inside it continue through the independently copied carrier.
    */
    fn copy_source_completions(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
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
                || expressions(program).borrow_expression(expression).is_some()
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
                            observation.expression == expression
                                && observation.evaluation == evaluation
                        });
                    let observation = observations.next()?;
                    if observations.next().is_some()
                        || !facts.flow.control.operands.is_valid(observation.receiver)
                        || !place_at(
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

    /*
        An anonymous owner born on one route cannot leave a cleanup event until
        after reconvergence: that would revisit an already completed branch.
        Dispose its complement at the edge that actually completes its value.
    */
    #[allow(clippy::too_many_arguments)]
    fn establish_targets(
        program: &dyn ::validation::CheckingInput,
        facts: &mut checked_trees::CheckFactsWalk<'_>,
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
            let replaces_zero_image = place.multiplicity == Multiplicity::Linear
                && place.zero_image
                && place.path.is_empty();
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
                    prior_value_transferred: replacement_custody
                        .definitely_transferred(target.root, &target.destination_path),
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
                provenance: provenance.unwrap_or_else(|| {
                    established_provenance(machine_symbol, state_symbol, source)
                }),
                root: target.root,
                segments,
                obligation_live,
            };
            claim_outcomes.resolve(
                program,
                &facts.flow.ownership.segments,
                &mut establishment,
                permission_events,
            );
            let obligation_live = establishment.obligation_live;
            place.live = obligation_live || place.multiplicity == Multiplicity::Affine;
            place.case_excluded = place.conditional && !obligation_live;
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

    /*
        Retain this exact provisional transfer only when it moves a containing
        carrier while its conditional claim is live. Reconciliation may later
        prove the payload absent and retire the recorded event. Named moves,
        anonymous call arguments and completed returns share this rule; duplicate
        transfers and direct payload extractions remain ordinary replay errors.
    */
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

    /*
        Whole-carrier transport carries the actual selected case; an authenticated
        absent payload is not a resource to consume. An explicit case projection
        must still prove that its selected payload exists.
    */
    fn excluded_carrier_payload(place: &LinearPlace, path: &[facts::PlaceSegment]) -> bool {
        place.ever_established
            && !place.live
            && place.case_excluded
            && !path
                .iter()
                .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
    }

    /* Temporary bounded source-producer diagnostic; remove before landing. */
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
}

pub(crate) use affine_cleanup::{
    append_owned_cleanup_permission_events, append_owned_region_cleanup_permission_events,
    exit_evaluation,
};
pub(crate) use applied_events::{
    append_borrow_permission_events, apply_recorded_state_entry_events,
    apply_recorded_statement_events, permission_event_statement_index,
};
pub(crate) use claim_frontier::{
    initial_linear_places, linear_claim_frontier, reserve_owner_places,
};
pub(crate) use claim_provenance::{completed_statement_values, written_linear_targets};
pub(crate) use permission_production::{
    ObservedCustody, complete_statement_permissions, established_provenance,
    evaluate_statement_permissions, event_statement_index, permission_source,
    record_conditional_carrier_transfer,
};

use crate::checks::multiplicity::owned_selection;
use crate::checks::multiplicity::type_multiplicity::type_carries_linear_obligation;
use checked_trees::{CheckFacts, FlowClaimOutcomeSource, FlowPermissionEventFact};
use diagnostics::Diagnostic;
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource,
};

/*
    A live linear obligation cannot be completed by observing its scalar value.
    Use the same refusal at an anonymous normal-death cut and a named scope exit.
*/
fn unconsumed_linear_value_diagnostic(
    program: &dyn ::validation::CheckingInput,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    place: &super::linear_obligations::LinearPlace,
) -> Diagnostic {
    let mut diagnostic = place.unconsumed_scope_exit();
    diagnostic.message.push_str(&format!(
        " in machine `{}` state `{}`",
        program.symbols.name(machine),
        program.symbols.name(state),
    ));
    diagnostic
}

pub(crate) fn validate_linear_permission_events(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let join_failures =
        super::claim_outcomes::validate_call_claim_joins(program, facts, &mut diagnostics);
    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(state) = program.state_by_symbol(state_flow.state_symbol) else {
            continue;
        };
        let Some(machine) = program.machine_by_symbol(state_flow.machine_symbol) else {
            continue;
        };
        if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
            continue;
        }
        let mut places =
            initial_linear_places(program, state, state_flow.machine_symbol, state.symbol);
        let events = super::permission_events::owned_permission_events(
            facts,
            state_flow.machine_symbol,
            state.symbol,
        );
        for event in events
            .iter()
            .filter(|event| event.kind == PermissionEventKind::Establish)
        {
            if !matches!(event.root, facts::PlaceRoot::Expression(_)) {
                continue;
            }
            let root = validation::CanonicalPlace {
                root: event.root,
                segments: Vec::new(),
            };
            let Some(type_reference) = validation::canonical_place_type_reference(
                program,
                state.symbol,
                permission_event_statement_index(event.source).unwrap_or_default(),
                &root,
            ) else {
                continue;
            };
            reserve_owner_places(program, event.root, type_reference, &mut places);
        }
        apply_recorded_state_entry_events(&events, &facts.flow.ownership.segments, &mut places);
        append_unresolved_state_result_mapping_diagnostics(
            program,
            state,
            &events,
            &facts.flow.ownership,
            &join_failures,
            &mut diagnostics,
        );

        let has_case_places = places.iter().any(|place| {
            place
                .path
                .iter()
                .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
        });
        let mut outcomes = Vec::new();
        super::permission_schedule::walk(
            program,
            state,
            facts.flow.control.calls.span_or_empty(state_flow.calls),
            (
                places.clone(),
                owned_selection::SelectionOccurrences::default(),
            ),
            |step, custody| {
                let (places, selections) = custody;
                if has_case_places
                    && let Some(selected) = super::permission_schedule::SelectedCases::at_step(
                        program, facts, state_flow, machine, state, step,
                    )
                {
                    selected.apply_claims(places);
                }
                match step {
                    super::permission_schedule::PermissionStep::Evaluate {
                        statement,
                        evaluation,
                    } => {
                        if let Err(diagnostic) = selections.replay_statement(
                            program,
                            state,
                            statement,
                            evaluation,
                            &events,
                            &facts.flow.ownership,
                            places,
                        ) {
                            diagnostics.push(diagnostic);
                        }
                        apply_recorded_statement_events(
                            program,
                            facts,
                            machine,
                            state,
                            statement,
                            evaluation,
                            &events,
                            places,
                            &mut diagnostics,
                        );
                        if let Some((_, receipt)) =
                            facts.flow.ownership.owned_selection_at_evaluation(
                                state.symbol,
                                statement as u32,
                                evaluation,
                            )
                        {
                            selections.consume(program, &facts.flow.ownership, receipt);
                            owned_selection::apply_availability(
                                program,
                                &facts.flow.ownership,
                                receipt,
                                places,
                            );
                        }
                    }
                    super::permission_schedule::PermissionStep::Exit {
                        coordinate,
                        crash: false,
                    } => {
                        if let Err(diagnostic) = selections.check_events(
                            program,
                            events.iter().copied().filter(|event| {
                                event.source == PermissionEventSource::StateExit
                                    && event.evaluation
                                        == affine_cleanup::exit_evaluation(
                                            program, state, coordinate,
                                        )
                            }),
                            &facts.flow.ownership.segments,
                        ) {
                            diagnostics.push(diagnostic);
                        }
                        applied_events::apply_recorded_exit_dispositions(
                            program,
                            facts,
                            machine,
                            state,
                            coordinate,
                            &events,
                            places,
                            &mut diagnostics,
                        );
                        affine_cleanup::apply_recorded_cleanup_events(
                            program,
                            state,
                            state_flow.machine_symbol,
                            coordinate,
                            &events,
                            &facts.flow.ownership.segments,
                            &facts.contextual_cleanup,
                            places,
                            &mut diagnostics,
                        );
                        outcomes.push((coordinate, places.clone()))
                    }
                    _ => {}
                }
            },
        );
        let mut mixed_places = Vec::new();
        let has_ordinary_exit = !outcomes.is_empty();
        if let Some((_, first)) = outcomes.first() {
            for place_index in 0..places.len() {
                let live = first[place_index].live;
                if places[place_index].multiplicity == Multiplicity::Linear
                    && outcomes
                        .iter()
                        .skip(1)
                        .any(|(_, outcome)| outcome[place_index].live != live)
                {
                    let exits = outcomes
                        .first()
                        .into_iter()
                        .chain(
                            outcomes
                                .iter()
                                .skip(1)
                                .filter(|(_, outcome)| outcome[place_index].live != live)
                                .take(1),
                        )
                        .map(|(coordinate, outcome)| {
                            let place = &outcome[place_index];
                            let selection = case_selection_diagnostics::exit_detail(
                                program,
                                facts,
                                state,
                                *coordinate,
                                place,
                            );
                            format!(
                                "statement {} {:?}: live={}, case_excluded={}, established={}{}",
                                coordinate.statement_index,
                                coordinate.kind,
                                place.live,
                                place.case_excluded,
                                place.ever_established,
                                selection,
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("; ");
                    diagnostics.push(Diagnostic::error(format!(
                            "linear value `{}` has inconsistent treatment across transition arms; every path must consume/transfer it or every path must preserve the same live obligation (machine `{}` state `{}`; {exits})",
                            places[place_index].name,
                            program.symbols.name(machine.symbol),
                            program.symbols.name(state.symbol),
                        )));
                    mixed_places.push((places[place_index].root, places[place_index].path.clone()));
                } else {
                    places[place_index].live = live;
                    places[place_index].ever_established = outcomes
                        .iter()
                        .any(|(_, outcome)| outcome[place_index].ever_established);
                }
            }
        }

        if has_ordinary_exit {
            for place in places.iter().filter(|place| {
                place.multiplicity == Multiplicity::Linear
                    && place.live
                    && !mixed_places
                        .iter()
                        .any(|(symbol, path)| *symbol == place.root && *path == place.path)
            }) {
                diagnostics.push(unconsumed_linear_value_diagnostic(
                    program,
                    machine.symbol,
                    state.symbol,
                    place,
                ));
            }
        }
    }

    crate::checks::finish(diagnostics)
}

fn append_unresolved_state_result_mapping_diagnostics(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    events: &[&FlowPermissionEventFact],
    ownership: &checked_trees::FlowOwnershipFacts,
    join_failures: &[super::claim_outcomes::CallClaimJoinFailure],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut unresolved_statements = Vec::new();
    for event in events {
        if event.kind != PermissionEventKind::Establish
            || event.access != PermissionAccess::Owned
            || !event.obligation_live
        {
            continue;
        }
        let Some((statement_index, result_expression, result_path)) =
            super::claim_outcomes::receiving_value(program, &ownership.segments, event)
        else {
            continue;
        };
        let typed_trees::expression::ExpressionNode::Call(call) =
            program.expression_table.expression(result_expression)
        else {
            continue;
        };
        let Some(target_state) = validation::call_target_state(program, call.target_symbol) else {
            continue;
        };
        if crate::flow::state_statements(program, target_state.statement_nodes).is_empty()
            || !type_carries_linear_obligation(program, target_state.return_type)
        {
            continue;
        }
        let mapped_from_call = ownership.with_state_permissions(target_state.symbol, |handles| {
            handles.iter().copied().any(|handle| {
                let candidate = ownership.permissions.get(handle);
                candidate.kind == PermissionEventKind::Transfer
                    && candidate.access == PermissionAccess::Owned
                    && candidate.obligation_live
                    && candidate.claim_identity != PermissionClaimIdentity::Unknown
                    && candidate.claim_identity == event.claim_identity
                    && candidate.state_symbol == target_state.symbol
            })
        }) || ownership.with_state_permissions(state.symbol, |handles| {
            handles.iter().copied().any(|handle| {
                let candidate = ownership.permissions.get(handle);
                candidate.kind == PermissionEventKind::Transfer
                    && candidate.access == PermissionAccess::Owned
                    && candidate.obligation_live
                    && candidate.claim_identity != PermissionClaimIdentity::Unknown
                    && candidate.claim_identity == event.claim_identity
                    && candidate.state_symbol == state.symbol
                    && permission_event_statement_index(candidate.source) == Some(statement_index)
            })
        });
        let receiving_path = ownership.segments.span_or_empty(event.segments);
        let mapped_by_checked_outcome = ownership
            .claim_outcome_maps
            .iter()
            .filter(|(_, map)| map.state_symbol == target_state.symbol)
            .flat_map(|(_, map)| ownership.claim_outcome_entries.span_or_empty(map.entries))
            .any(|entry| {
                ownership.segments.span_or_empty(entry.output_segments) == result_path
                    && matches!(
                        entry.source,
                        FlowClaimOutcomeSource::Established {
                            claim_identity,
                            provenance,
                        } if claim_identity == event.claim_identity
                            && provenance == event.provenance
                    )
            });
        let mapped_by_join = ownership.claim_join_receipts.iter().any(|(_, receipt)| {
            receipt.machine_symbol == event.machine_symbol
                && receipt.state_symbol == state.symbol
                && receipt.statement_index == statement_index
                && receipt.expression == result_expression
                && receipt.target_symbol == target_state.symbol
                && receipt.result_root == event.root
                && ownership.segments.span_or_empty(receipt.result_segments) == receiving_path
                && receipt.claim_identity == event.claim_identity
                && matches!(
                    event.provenance,
                    language_semantics::PermissionProvenance::Joined { .. }
                )
        });
        if mapped_from_call || mapped_by_checked_outcome || mapped_by_join {
            continue;
        }
        if let Ok(filter) = std::env::var("OMEGA_TRACE_CLAIM_FRONTIER")
            && validation::find_state(program, state.symbol).is_some_and(|(machine, _)| {
                filter == "*" || filter == program.symbols.name(machine.symbol)
            })
        {
            static EMITTED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            if EMITTED.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 24 {
                let maps = ownership
                    .claim_outcome_maps
                    .iter()
                    .filter(|(_, map)| map.state_symbol == target_state.symbol)
                    .map(|(_, map)| {
                        (
                            map,
                            ownership.claim_outcome_entries.span_or_empty(map.entries),
                        )
                    })
                    .collect::<Vec<_>>();
                let joins = ownership
                    .claim_join_receipts
                    .iter()
                    .filter(|(_, receipt)| {
                        receipt.state_symbol == state.symbol
                            && receipt.statement_index == statement_index
                            && receipt.expression == result_expression
                    })
                    .map(|(_, receipt)| receipt)
                    .collect::<Vec<_>>();
                let events = ownership
                    .permissions
                    .iter()
                    .filter(|(_, candidate)| {
                        candidate.state_symbol == state.symbol
                            && permission_event_statement_index(candidate.source)
                                == Some(statement_index)
                    })
                    .map(|(_, candidate)| {
                        (
                            candidate,
                            ownership.segments.span_or_empty(candidate.segments),
                        )
                    })
                    .collect::<Vec<_>>();
                eprintln!(
                    "CLAIM_RECEIVER state={} stmt={statement_index} target={} expression={result_expression:?} result_path={result_path:?} event={event:?} receiving_path={receiving_path:?} maps={maps:?} joins={joins:?} events={events:?}",
                    program.symbols.name(state.symbol),
                    program.symbols.name(target_state.symbol)
                );
            }
        }
        if !unresolved_statements
            .iter()
            .any(|(index, _, _)| *index == statement_index)
        {
            let root = match event.root {
                facts::PlaceRoot::Symbol(symbol) => program.symbols.name(symbol).to_owned(),
                other => format!("{other:?}"),
            };
            let path = receiving_path
                .iter()
                .map(|segment| match segment {
                    facts::PlaceSegment::Field { symbol } => {
                        format!(".{}", program.symbols.name(*symbol))
                    }
                    facts::PlaceSegment::Case { variant } => {
                        format!("::{}", program.symbols.name(*variant))
                    }
                    facts::PlaceSegment::FixedIndex { index } => format!("[{index}]"),
                    other => format!("{other:?}"),
                })
                .collect::<String>();
            let detail = join_failures.iter().find_map(|failure| {
                failure.detail_for(program, event, result_expression, &ownership.segments)
            });
            unresolved_statements.push((statement_index, format!("{root}{path}"), detail));
        }
    }

    if unresolved_statements.is_empty() {
        return;
    }
    let machine_name = validation::find_state(program, state.symbol)
        .map(|(machine, _)| machine.name.as_str())
        .unwrap_or("<unknown machine>");
    for (statement_index, path, detail) in unresolved_statements {
        let detail = detail.map_or_else(String::new, |detail| format!("; {detail}"));
        diagnostics.push(Diagnostic::error(format!(
            "linear state-call result {path} in {machine_name}::{} at statement {statement_index} has no unique conserved claim mapping; return a path-aligned source place or publish an explicit outcome mapping{detail}",
            program.symbols.name(state.symbol)
        )));
    }
}
