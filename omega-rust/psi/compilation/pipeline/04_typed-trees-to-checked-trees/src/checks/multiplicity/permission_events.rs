//! Recording permission events: crash frontiers, proven case membership
//! and the statically inactive call results that shape which events count.

use super::cleanup_dispositions::RemainingCustody;
use crate::checks::multiplicity::claim_outcomes::{
    apply_claim_origin_rewrites, call_result_origin_rewrites, derive_checked_claim_outcome_maps,
    publish_call_claim_joins, publish_claim_outcome_maps,
};
use crate::checks::multiplicity::linear_obligations::{
    CheckedClaimOutcomeMap, ClaimIdentityAllocator, LinearPlace,
};
use crate::checks::multiplicity::linear_validation::{
    append_borrow_permission_events, append_owned_cleanup_permission_events,
    apply_recorded_state_entry_events, apply_recorded_statement_events,
    apply_statement_permission_production, initial_linear_places, reserve_owner_places,
    select_static_case_alternative,
};
use crate::checks::multiplicity::owned_selection;
use crate::checks::multiplicity::temporary_results;
use checked_trees::{CheckFacts, ContextualCleanupFacts, FlowPermissionEventFact};
use diagnostics::Diagnostic;
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource,
};
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;

/// The `Owned` permission events recorded for one `(machine, state)` flow, in
/// emission order — the roster both the linear replay and the entry-claim
/// walk replay against the same published rules.
pub(crate) fn owned_permission_events(
    facts: &CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Vec<&FlowPermissionEventFact> {
    facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter_map(|(_, event)| {
            (event.machine_symbol == machine_symbol
                && event.state_symbol == state_symbol
                && event.access == PermissionAccess::Owned)
                .then_some(event)
        })
        .collect()
}

/// Prepare source custody without inspecting derived callable contracts. Crash
/// frontier projection belongs after those contracts have been constructed.
pub(crate) fn prepare_permission_events(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut cleanup_dispositions = ContextualCleanupFacts::default();
    facts.flow.ownership.instruction_dispositions = arena::Arena::default();
    facts.flow.ownership.instruction_effect_receipts = arena::Arena::default();
    facts.flow.ownership.storage_transfers = arena::Arena::default();
    facts.flow.ownership.selected_cleanup_permissions = arena::Arena::default();
    let mut permission_events = Vec::new();
    let mut conditional_carrier_transfers = Vec::new();
    let mut selection_diagnostics = Vec::new();
    facts.flow.ownership.owned_selections = arena::Arena::default();
    facts.flow.ownership.selection_sources = arena::Arena::default();
    facts.flow.ownership.selection_transfers = arena::Arena::default();
    facts.flow.ownership.selection_transfer_claims = arena::Arena::default();
    let mut claim_identities = ClaimIdentityAllocator::default();
    facts.flow.ownership.loan_completions = arena::Arena::default();
    let call_frames = validation::CallFrameResolver::new(program);
    let mutation_summaries = crate::flow::StateMutationSummaryCache::default();

    let state_flows = facts
        .flow
        .control
        .states
        .iter()
        .map(|(handle, state)| (handle, state.clone()))
        .collect::<Vec<_>>();
    for (state_handle, state_flow) in state_flows {
        let Some(machine) = program.machine_by_symbol(state_flow.machine_symbol) else {
            continue;
        };
        let Some(state) = program.state_by_symbol(state_flow.state_symbol) else {
            continue;
        };
        let statements = program.statement_table.statements(state.statement_nodes);
        let remaining = RemainingCustody::default();
        let mut places =
            initial_linear_places(program, state, state_flow.machine_symbol, state.symbol);

        for place in places.iter_mut().filter(|place| {
            place.ever_established
                && (place.multiplicity == Multiplicity::Linear || place.conditional)
        }) {
            let claim_identity = claim_identities.mint(
                state_flow.machine_symbol,
                state.symbol,
                PermissionEventSource::StateEntry,
            );
            place.claim_identity = Some(claim_identity);
            let segments = facts
                .flow
                .ownership
                .segments
                .insert_many(place.path.iter().copied());
            permission_events.push(FlowPermissionEventFact {
                expression: Default::default(),
                evaluation: checked_trees::FlowPermissionEvaluation::Statement,
                machine_symbol: state_flow.machine_symbol,
                state_symbol: state.symbol,
                source: PermissionEventSource::StateEntry,
                kind: PermissionEventKind::Establish,
                multiplicity: place.multiplicity,
                access: PermissionAccess::Owned,
                claim_identity,
                provenance: place.provenance.expect("entry place has provenance"),
                root: place.root,
                segments,
                obligation_live: true,
            });
        }

        // A bodyless contract still names ingress claims for its signature,
        // but has no executable evaluation or scope-exit cleanup. Walking an
        // empty declaration would fabricate a FinalExpression death with no
        // semantic outgoing edge. Actual provider/member bodies own disposal
        // and returned custody; their schedules remain checked normally.
        if machine.has_bodyless_signature() {
            continue;
        }

        let moves = crate::flow::discover_state_move_events(
            program,
            &facts.borrow,
            &facts.operators,
            machine,
            state,
            &mut facts.flow.ownership.segments,
        );
        // Snapshot the state's ordered calls once while permission production
        // mutates ownership arenas. Each evaluation selects from this snapshot.
        let calls = facts
            .flow
            .control
            .calls
            .span_or_empty(state_flow.calls)
            .to_vec();
        let completion_regions = if !places
            .iter()
            .any(|place| place.multiplicity == Multiplicity::Linear)
        {
            Vec::new()
        } else {
            match super::linear_validation::loan_completions::regions_for_calls(
                program,
                facts,
                machine.symbol,
                state.symbol,
                &calls,
                call_frames.as_ref(),
                &mutation_summaries,
            ) {
                Ok(regions) => regions,
                Err(diagnostic) => {
                    selection_diagnostics.push(diagnostic);
                    continue;
                }
            }
        };
        let temporary_owners =
            temporary_results::temporary_owners(program, facts, machine, state, &calls);
        // Expression-owned completions need the same claim namespace as
        // named destinations. A name/projection already refers to existing
        // custody; only a real expression root reserves a new owner here.
        for (statement_index, statement) in statements.iter().enumerate() {
            for (expression, reference) in
                crate::checks::ownership_occurrences::observed_owned_temporaries(
                    program,
                    machine,
                    state,
                    statement_index,
                    statement,
                    &facts.operators,
                )
            {
                reserve_owner_places(
                    program,
                    facts::PlaceRoot::Expression(expression),
                    reference,
                    &mut places,
                );
            }
            for (expression, _) in
                super::linear_validation::completed_statement_values(program, statement)
            {
                let Some(place) = validation::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    expression,
                ) else {
                    continue;
                };
                if place.root != facts::PlaceRoot::Expression(expression) {
                    continue;
                }
                if let Some(reference) = validation::canonical_place_type_reference(
                    program,
                    state.symbol,
                    statement_index,
                    &place,
                ) {
                    reserve_owner_places(program, place.root, reference, &mut places);
                }
            }
        }
        for owner in &temporary_owners {
            reserve_owner_places(
                program,
                facts::PlaceRoot::Expression(owner.expression),
                owner.type_reference,
                &mut places,
            );
        }
        if moves
            .iter()
            .any(|event| event.evaluation == checked_trees::FlowPermissionEvaluation::Unknown)
        {
            selection_diagnostics.push(Diagnostic::error(
                "ownership occurrence has no exact source evaluation region",
            ));
            continue;
        }
        super::permission_schedule::walk(
            program,
            state,
            (
                places,
                remaining,
                owned_selection::SelectionOccurrences::default(),
                super::selected_cleanup::SelectedCleanup::default(),
                super::instruction_dispositions::InstructionDispositions::default(),
            ),
            |step, custody| {
                let (places, remaining, selections, selected_cleanup, instruction_dispositions) =
                    custody;
                super::permission_schedule::select_cases(program, state, step, places);
                match step {
                    super::permission_schedule::PermissionStep::Evaluate {
                        statement: statement_index,
                        evaluation,
                    } => {
                        if !matches!(
                            evaluation,
                            checked_trees::FlowPermissionEvaluation::TransitionTarget(_)
                        ) {
                            if let Err(diagnostic) = selections.check_statement(
                                program,
                                facts,
                                machine,
                                state,
                                statement_index,
                                &statements[statement_index],
                            ) {
                                selection_diagnostics.push(diagnostic);
                                return;
                            }
                            match owned_selection::record_statement(
                                program,
                                facts,
                                machine,
                                state,
                                statement_index,
                                &statements[statement_index],
                                places,
                            ) {
                                Ok(true) => {
                                    if let Some((_, receipt)) = facts
                                        .flow
                                        .ownership
                                        .owned_selection_at(state.symbol, statement_index as u32)
                                    {
                                        selections.consume(program, &facts.flow.ownership, receipt);
                                        for place in selections.unavailable_places() {
                                            remaining.transfer_storage(place.root, &place.segments);
                                        }
                                    }
                                    return;
                                }
                                Ok(false) => {}
                                Err(diagnostic) => {
                                    selection_diagnostics.push(diagnostic);
                                    return;
                                }
                            }
                        }
                        let selected =
                            crate::checks::ownership_occurrences::statement_steps_at_evaluation(
                                program,
                                machine,
                                state,
                                statement_index,
                                &statements[statement_index],
                                &moves,
                                &facts.operators,
                                Some(evaluation),
                            )
                            .into_iter()
                            .filter_map(|occurrence| match occurrence.step {
                                crate::checks::ownership_occurrences::StatementStep::Move {
                                    event,
                                    conditional: true,
                                    ordered: true,
                                } if moves[event].evaluation == evaluation => {
                                    Some(moves[event].expression)
                                }
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        let has_selected_move = !selected.is_empty();
                        let before_places = if has_selected_move {
                            places.clone()
                        } else {
                            Vec::new()
                        };
                        let event_start = permission_events.len();
                        remaining.reserve(program, &statements[statement_index]);
                        let before_remaining = remaining.clone();
                        let routes = super::selected_evaluation::ExpressionRoutes::new(
                            program,
                            machine,
                            state,
                            statement_index,
                            evaluation,
                            &facts.operators,
                        );
                        let produced_storage = match apply_statement_permission_production(
                            program,
                            facts,
                            state_flow.machine_symbol,
                            state.symbol,
                            &moves,
                            &calls,
                            &completion_regions,
                            &temporary_owners,
                            statement_index,
                            &statements[statement_index],
                            evaluation,
                            places,
                            remaining,
                            &mut permission_events,
                            &mut claim_identities,
                            &mut conditional_carrier_transfers,
                            selections,
                            &mut cleanup_dispositions,
                        ) {
                            Ok(establishments) => establishments,
                            Err(diagnostic) => {
                                selection_diagnostics.push(diagnostic);
                                return;
                            }
                        };
                        instruction_dispositions.statement(
                            program,
                            facts,
                            machine,
                            state,
                            statement_index,
                            evaluation,
                            places,
                            &mut permission_events,
                        );
                        if let Err(diagnostic) = selected_cleanup.observe(
                            program,
                            &selected,
                            &before_places,
                            &before_remaining,
                            &routes,
                            &calls,
                            &permission_events[event_start..],
                            &produced_storage.establishments,
                            &mut facts.flow.ownership,
                            &mut cleanup_dispositions,
                        ) {
                            selection_diagnostics.push(diagnostic);
                            return;
                        }
                        // The producer already retained canonical storage
                        // moves and joined expression custody. Rebuilding it
                        // from leaf claim events would resurrect transferred
                        // transparent homes and schedule duplicate cleanup.
                        let receiving = produced_storage
                            .establishments
                            .iter()
                            .filter(|establishment| {
                                establishment.event_ordinal >= produced_storage.evaluated_events
                            })
                            .map(|establishment| {
                                super::cleanup_dispositions::StorageEstablishment {
                                    event_ordinal: establishment.event_ordinal
                                        - produced_storage.evaluated_events,
                                    destination: establishment.destination.clone(),
                                }
                            })
                            .collect::<Vec<_>>();
                        if let Err(diagnostic) = remaining.observe_evaluation(
                            program,
                            &routes,
                            &calls,
                            &permission_events[event_start + produced_storage.evaluated_events..],
                            &facts.flow.ownership.segments,
                            &mut cleanup_dispositions,
                            &receiving,
                        ) {
                            selection_diagnostics.push(diagnostic);
                        }
                    }
                    super::permission_schedule::PermissionStep::Exit {
                        coordinate,
                        crash: false,
                    } => {
                        let event_start = permission_events.len();
                        instruction_dispositions.exit(
                            program,
                            facts,
                            machine,
                            state,
                            coordinate,
                            places,
                            &mut permission_events,
                        );
                        append_owned_cleanup_permission_events(
                            program,
                            &facts.operators,
                            state,
                            state_flow.machine_symbol,
                            coordinate,
                            places,
                            remaining,
                            &mut facts.flow.ownership.segments,
                            &mut permission_events,
                        );
                        if let Err(diagnostic) = selected_cleanup.finish(
                            program,
                            state,
                            coordinate,
                            &permission_events[event_start..],
                            remaining,
                            &mut facts.flow.ownership,
                            &mut cleanup_dispositions,
                        ) {
                            selection_diagnostics.push(diagnostic);
                        }
                    }
                    super::permission_schedule::PermissionStep::Exit { crash: true, .. }
                    | super::permission_schedule::PermissionStep::MissedGuard { .. } => {}
                }
            },
        );
        // Publish the source storage occurrences, not expanded claim events.
        // Loan exclusion needs even a claim-free or inactive carrier's move.
        facts
            .flow
            .control
            .states
            .get_mut(state_handle)
            .storage_transfers = facts.flow.ownership.storage_transfers.insert_many(moves);
    }

    append_borrow_permission_events(
        program,
        facts,
        &mut permission_events,
        &mut claim_identities,
    );
    let claim_outcome_maps = reconcile_state_call_result_origins(
        program,
        &facts.borrow,
        &facts.flow.ownership,
        &mut permission_events,
    );

    // Reconciliation can prove that a returned sum never establishes a
    // particular payload. The provisional carrier moves recorded above then
    // contain no transfer for that payload. Keep its inactive establishment,
    // but do not ask replay to consume a nonexistent claim. Only occurrences
    // recorded while live and selected through a containing carrier qualify;
    // neither duplicate moves nor explicit inactive payload access is waived.
    let mut event_index = 0;
    permission_events.retain(|event| {
        let absent_carrier_transfer = conditional_carrier_transfers
            .binary_search(&event_index)
            .is_ok()
            && !event.obligation_live
            && event.claim_identity == PermissionClaimIdentity::Unknown;
        event_index += 1;
        !absent_carrier_transfer
    });

    facts.flow.ownership.permissions = arena::Arena::default();
    facts
        .flow
        .ownership
        .permissions
        .insert_many(permission_events);
    publish_claim_outcome_maps(facts, claim_outcome_maps);
    publish_call_claim_joins(program, facts);
    let joined_events = facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event.clone())
        .collect::<Vec<_>>();
    let joined_maps = derive_checked_claim_outcome_maps(
        program,
        &facts.borrow,
        &facts.flow.ownership,
        &joined_events,
    );
    publish_claim_outcome_maps(facts, joined_maps);
    if selection_diagnostics.is_empty() {
        cleanup_dispositions.entry_requirements =
            std::mem::take(&mut facts.contextual_cleanup.entry_requirements);
        facts.contextual_cleanup = cleanup_dispositions;
        // A replacement kills the old referent, not the borrowed access
        // carrier. Select that death while source windows still distinguish
        // a live old value from an already extracted hole, before deriving
        // hook prerequisites and effects from the complete death roster.
        super::linear_obligations::prepare_storage_deaths(program, facts)?;
        let mut cleanup_dispositions = std::mem::take(&mut facts.contextual_cleanup);
        crate::facts::contextual_cleanup::bind_applications(
            program,
            &facts.flow,
            &mut cleanup_dispositions,
        );
        facts.contextual_cleanup = cleanup_dispositions;
        Ok(())
    } else {
        Err(selection_diagnostics)
    }
}

#[derive(Debug)]
struct DerivedCrashFrontier {
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_ordinal: u32,
    claims: Vec<PermissionClaimIdentity>,
}

/// Retain the machine-local claims that are definitely live at each explicit
/// crash. The result is an underapproximation by design: a conditional sum
/// payload is omitted until active-case proof can make its liveness definite.
/// Crash abandons these claims; it does not synthesize cleanup or consumption.
pub(crate) fn record_crash_frontier_lower_bounds(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    incoming_guards: &crate::checks::ranges::incoming_guards::IncomingGuardIndex,
) {
    let mut derived = Vec::new();
    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(machine) = program.machine_by_symbol(state_flow.machine_symbol) else {
            continue;
        };
        let Some(state) = program.state_by_symbol(state_flow.state_symbol) else {
            continue;
        };
        let events = owned_permission_events(facts, state_flow.machine_symbol, state.symbol);
        let mut places =
            initial_linear_places(program, state, state_flow.machine_symbol, state.symbol);
        apply_recorded_state_entry_events(&events, &facts.flow.ownership.segments, &mut places);
        let incoming = incoming_guards.for_machine(machine.symbol);
        let proven_conditional_claims =
            proven_conditional_entry_claims(program, state, incoming, &places);

        let mut ignored_diagnostics = Vec::new();
        super::permission_schedule::walk(program, state, places, |step, places| {
            super::permission_schedule::select_cases(program, state, step, places);
            match step {
                super::permission_schedule::PermissionStep::Evaluate {
                    statement,
                    evaluation,
                } => {
                    if evaluation == checked_trees::FlowPermissionEvaluation::Statement
                        && let Some((_, receipt)) = facts
                            .flow
                            .ownership
                            .owned_selection_at(state.symbol, statement as u32)
                    {
                        owned_selection::apply_availability(
                            program,
                            &facts.flow.ownership,
                            receipt,
                            places,
                        );
                        return;
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
                        &mut ignored_diagnostics,
                    );
                }
                super::permission_schedule::PermissionStep::Exit {
                    coordinate,
                    crash: true,
                } => {
                    let claims = places
                        .iter()
                        .filter_map(|place| {
                            (place.live
                                && (!place.conditional
                                    || place.claim_identity.is_some_and(|identity| {
                                        proven_conditional_claims.contains(&identity)
                                    })))
                            .then_some(place.claim_identity?)
                            .filter(|identity| *identity != PermissionClaimIdentity::Unknown)
                        })
                        .collect();
                    derived.push(DerivedCrashFrontier {
                        machine_symbol: state_flow.machine_symbol,
                        state_symbol: state.symbol,
                        statement_ordinal: u32::try_from(coordinate.statement_index)
                            .expect("state-local statement ordinal exceeds checked identity range"),
                        claims,
                    });
                }
                _ => {}
            }
        });
    }

    for contract in &mut facts.contract_plans.machines {
        let checked_sites = contract
            .crash
            .checked_sites()
            .iter()
            .map(|site| {
                let location = site.location();
                let frontier = derived
                    .iter()
                    .find(|frontier| {
                        frontier.machine_symbol == contract.machine
                            && frontier.state_symbol == location.state()
                            && frontier.statement_ordinal == location.statement_ordinal()
                    })
                    .map(|frontier| frontier.claims.clone())
                    .unwrap_or_else(|| site.frontier_lower_bound().to_vec());
                site.clone().with_frontier_lower_bound(frontier)
            })
            .collect();
        contract.crash = contract
            .crash
            .clone()
            .with_checked_sites(checked_sites)
            .expect("derived crash frontiers retain valid checked-site identity");
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProvenCaseMembership {
    parameter: SymbolHandle,
    /// Source-independent path below the final-state parameter at which this
    /// case tag was tested. The selected variant is stored separately.
    subject_path: Vec<facts::PlaceSegment>,
    variant: SymbolHandle,
}

/// Conditional sum payloads are only a crash-frontier lower bound when the
/// path into this state proves every case on the payload path active.
/// Case-pattern dispatch is retained in typed trees as a symbol-stamped
/// `value == Type::Case` guard. Incoming-edge argument composition rebinds the
/// tested subject to a final-state parameter plus a canonical symbol path.
///
/// The proof is attached to the claim identity rather than the parameter
/// spelling. A whole-value transfer in the target state therefore preserves
/// the proof, while overwriting the parameter mints a different identity and
/// cannot accidentally inherit it. A nested claim enters the lower bound only
/// when membership evidence covers every case segment.
fn proven_conditional_entry_claims(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    incoming: &[crate::checks::ranges::incoming_guards::IncomingGuard],
    places: &[LinearPlace],
) -> Vec<PermissionClaimIdentity> {
    let parameters = program.state_parameters(state).iter().collect::<Vec<_>>();
    let mut memberships = Vec::new();

    for entry in incoming.iter().filter(|entry| entry.holds_at(state.symbol)) {
        let mut case_tests = Vec::new();
        collect_positive_case_tests(program, entry.guard(), &mut case_tests);
        for (subject, variant) in case_tests {
            let Some(subject) = crate::flow::source_independent_place(program, subject) else {
                continue;
            };
            for parameter in &parameters {
                let Some(argument) = entry.argument_place_for_parameter(parameter.symbol) else {
                    continue;
                };
                if subject.root != argument.root
                    || !subject.segments.starts_with(&argument.segments)
                {
                    continue;
                }
                let membership = ProvenCaseMembership {
                    parameter: parameter.symbol,
                    subject_path: subject.segments[argument.segments.len()..].to_vec(),
                    variant,
                };
                if !memberships.contains(&membership) {
                    memberships.push(membership);
                }
            }
        }
    }

    let mut proven = Vec::new();
    for place in places.iter().filter(|place| place.conditional) {
        let mut subject_path = Vec::new();
        let all_cases_proven = place.path.iter().all(|segment| {
            let proven = match segment {
                facts::PlaceSegment::Case { variant } => memberships.iter().any(|evidence| {
                    facts::PlaceRoot::Symbol(evidence.parameter) == place.root
                        && evidence.subject_path == subject_path
                        && evidence.variant == *variant
                }),
                _ => true,
            };
            subject_path.push(*segment);
            proven
        });
        if all_cases_proven
            && let Some(identity) = place.claim_identity
            && identity != PermissionClaimIdentity::Unknown
            && !proven.contains(&identity)
        {
            proven.push(identity);
        }
    }
    proven
}

/// Extract positive case-membership conjuncts. Boolean-arm lowering may wrap
/// a predicate as `predicate == true`; unwrap that shell but deliberately do
/// not infer through negation or disjunction here.
pub(crate) fn collect_positive_case_tests(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    tests: &mut Vec<(typed_trees::expression::ExpressionHandle, SymbolHandle)>,
) {
    use typed_trees::expression::{BinaryOperator, ExpressionNode};

    let Some(binary) = program.expression_table.binary_node(expression) else {
        return;
    };
    if binary.operator == BinaryOperator::And {
        collect_positive_case_tests(program, binary.left, tests);
        collect_positive_case_tests(program, binary.right, tests);
        return;
    }
    if binary.operator != BinaryOperator::Equal {
        return;
    }

    let is_true = |candidate| {
        matches!(
            program.expression_table.expression(candidate),
            ExpressionNode::Boolean(true)
        )
    };
    if is_true(binary.left) {
        collect_positive_case_tests(program, binary.right, tests);
        return;
    }
    if is_true(binary.right) {
        collect_positive_case_tests(program, binary.left, tests);
        return;
    }

    let case_variant = |candidate| match program.expression_table.expression(candidate) {
        ExpressionNode::Name(path)
            if program.data_definitions().iter().any(|definition| {
                program.data_members(definition).iter().any(|member| {
                    matches!(
                        member,
                        typed_trees::data::DataMember::Variant(variant)
                            if variant.symbol == path.symbol
                    )
                })
            }) =>
        {
            Some(path.symbol)
        }
        _ => None,
    };
    if let Some(variant) = case_variant(binary.right) {
        tests.push((binary.left, variant));
    } else if let Some(variant) = case_variant(binary.left) {
        tests.push((binary.right, variant));
    }
}

pub(crate) fn exact_positive_case_test(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<(typed_trees::expression::ExpressionHandle, SymbolHandle)> {
    use typed_trees::expression::{BinaryOperator, ExpressionNode};

    let Some(binary) = program.expression_table.binary_node(expression) else {
        return None;
    };
    if binary.operator != BinaryOperator::Equal {
        return None;
    }
    let is_true = |candidate| {
        matches!(
            program.expression_table.expression(candidate),
            ExpressionNode::Boolean(true)
        )
    };
    if is_true(binary.left) {
        return exact_positive_case_test(program, binary.right);
    }
    if is_true(binary.right) {
        return exact_positive_case_test(program, binary.left);
    }
    let mut tests = Vec::new();
    collect_positive_case_tests(program, expression, &mut tests);
    let [test] = tests.as_slice() else {
        return None;
    };
    Some(*test)
}

pub(crate) fn case_transition_run_is_exhaustive(
    program: &dyn ::validation::CheckingInput,
    statements: &[StatementNode],
    arm_indices: &[usize],
) -> bool {
    let mut subject_label: Option<String> = None;
    let mut covered = Vec::new();
    for &statement_index in arm_indices {
        let Some(StatementNode::Transition(transition)) = statements.get(statement_index) else {
            return false;
        };
        let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard else {
            return false;
        };
        let Some((subject, variant)) = exact_positive_case_test(program, guard) else {
            // A guarded arm (`Case { .. } if extra -> ...`) tests its case and
            // more, so it covers nothing: a later arm must still cover that
            // case. It continues the run rather than ending it.
            let mut tests = Vec::new();
            collect_positive_case_tests(program, guard, &mut tests);
            if tests.is_empty() {
                return false;
            }
            for (subject, _) in tests {
                let label = program.expression_table.display_name(subject);
                if subject_label
                    .as_ref()
                    .is_some_and(|expected| *expected != label)
                {
                    return false;
                }
                subject_label = Some(label);
            }
            continue;
        };
        let label = program.expression_table.display_name(subject);
        if subject_label
            .as_ref()
            .is_some_and(|expected| *expected != label)
        {
            return false;
        }
        subject_label = Some(label);
        if !covered.contains(&variant) {
            covered.push(variant);
        }
    }
    let Some(first_variant) = covered.first() else {
        return false;
    };
    // Coverage is over the cases this definition can carry: a case whose
    // `where` fact the instance refutes is impossible and owes no arm, as
    // typing's exhaustiveness count and the closed-sum terminator agree.
    program.data_definitions().iter().any(|definition| {
        let variants = program
            .data_members(definition)
            .iter()
            .filter_map(|member| match member {
                typed_trees::data::DataMember::Variant(variant)
                    if !::validation::impossible_variant(program, variant) =>
                {
                    Some(variant.symbol)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        variants.contains(first_variant)
            && variants.len() == covered.len()
            && variants.iter().all(|variant| covered.contains(variant))
    })
}

/// The symbol-rooted canonical path a guard `subject` resolves to — the
/// shared peel both case-alternative guards apply before their path tests.
fn guarded_case_subject(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    subject: typed_trees::expression::ExpressionHandle,
) -> Option<(SymbolHandle, Vec<facts::PlaceSegment>)> {
    let place = validation::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        subject,
    )?;
    let symbol = place.root.as_symbol()?;
    Some((symbol, place.segments))
}

pub(crate) fn select_case_alternative_from_guard(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    subject: typed_trees::expression::ExpressionHandle,
    variant: SymbolHandle,
    places: &mut [LinearPlace],
) {
    let Some((symbol, mut selected_path)) =
        guarded_case_subject(program, state_symbol, statement_index, subject)
    else {
        return;
    };
    selected_path.push(facts::PlaceSegment::Case { variant });
    select_static_case_alternative(facts::PlaceRoot::Symbol(symbol), &selected_path, places);
}

pub(crate) fn exclude_case_alternative(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    subject: typed_trees::expression::ExpressionHandle,
    variant: SymbolHandle,
    places: &mut [LinearPlace],
) {
    let Some((symbol, path)) =
        guarded_case_subject(program, state_symbol, statement_index, subject)
    else {
        return;
    };
    let case_index = path.len();
    for candidate in places.iter_mut().filter(|candidate| {
        candidate.root == facts::PlaceRoot::Symbol(symbol)
            && candidate.path.get(..case_index) == Some(path.as_slice())
            && candidate.path.get(case_index) == Some(&facts::PlaceSegment::Case { variant })
    }) {
        candidate.live = false;
        candidate.case_excluded = true;
    }
}

/// Forward an invariant input/result correspondence to the caller's held
/// claim. A callee-local establishment instead retains the receiving
/// occurrence's identity; the later call join records its exact return
/// correspondence without copying a template identity across invocations.
fn reconcile_state_call_result_origins(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    ownership: &checked_trees::FlowOwnershipFacts,
    permission_events: &mut [FlowPermissionEventFact],
) -> Vec<CheckedClaimOutcomeMap> {
    let segments = &ownership.segments;
    let iteration_limit = permission_events.len().saturating_add(1);
    for _ in 0..iteration_limit {
        let maps = derive_checked_claim_outcome_maps(program, borrow, ownership, permission_events);
        let rewrites = call_result_origin_rewrites(program, segments, permission_events, &maps);
        let origins_changed = apply_claim_origin_rewrites(permission_events, &rewrites);
        let liveness_changed =
            apply_statically_inactive_call_results(program, segments, permission_events, &maps);
        if !origins_changed && !liveness_changed {
            return derive_checked_claim_outcome_maps(
                program,
                borrow,
                ownership,
                permission_events,
            );
        }
    }
    derive_checked_claim_outcome_maps(program, borrow, ownership, permission_events)
}

fn apply_statically_inactive_call_results(
    program: &dyn ::validation::CheckingInput,
    segments: &arena::Arena<facts::PlaceSegment>,
    permission_events: &mut [FlowPermissionEventFact],
    maps: &[CheckedClaimOutcomeMap],
) -> bool {
    let inactive_identities = permission_events
        .iter()
        .filter_map(|event| {
            if event.kind != PermissionEventKind::Establish
                || event.access != PermissionAccess::Owned
                || !event.obligation_live
            {
                return None;
            }
            let (_, expression, receiving_path) =
                super::claim_outcomes::receiving_value(program, segments, event)?;
            let typed_trees::expression::ExpressionNode::Call(call) =
                program.expression_table.expression(expression)
            else {
                return None;
            };
            let target = program.state_by_symbol(call.target_symbol)?;
            let map = maps.iter().find(|map| map.state_symbol == target.symbol)?;
            (!map
                .entries
                .iter()
                .any(|entry| entry.output_path == receiving_path))
            .then_some(event.claim_identity)
        })
        .filter(|identity| *identity != PermissionClaimIdentity::Unknown)
        .collect::<Vec<_>>();
    if inactive_identities.is_empty() {
        return false;
    }
    let mut changed = false;
    for event in permission_events.iter_mut().filter(|event| {
        event.access == PermissionAccess::Owned
            && inactive_identities.contains(&event.claim_identity)
    }) {
        changed |=
            event.obligation_live || event.claim_identity != PermissionClaimIdentity::Unknown;
        event.obligation_live = false;
        event.claim_identity = PermissionClaimIdentity::Unknown;
    }
    changed
}
