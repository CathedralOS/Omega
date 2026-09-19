//! Recording permission events: crash frontiers, proven case membership
//! and the statically inactive call results that shape which events count.

use crate::checks::multiplicity::claim_outcomes::{
    apply_claim_origin_rewrites, call_result_origin_rewrites, derive_checked_claim_outcome_maps,
    publish_claim_outcome_maps, publish_conditional_claim_joins,
};
use crate::checks::multiplicity::linear_obligations::{
    CheckedClaimOutcomeMap, ClaimIdentityAllocator, LinearPlace,
};
use crate::checks::multiplicity::linear_validation::{
    append_affine_cleanup_permission_events, append_borrow_permission_events,
    apply_recorded_state_entry_events, apply_recorded_statement_events,
    apply_statement_permission_production, initial_linear_places, select_static_case_alternative,
};
use crate::checks::multiplicity::owned_selection;
use crate::checks::multiplicity::temporary_results;
use checked_trees::{CheckFacts, FlowPermissionEventFact};
use diagnostics::Diagnostic;
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource,
};
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;

#[cfg(test)]
pub(crate) fn record_permission_events(program: &typed_trees::TypedTrees, facts: &mut CheckFacts) {
    let call_frames = validation::CallFrameResolver::new(program);
    let incoming_guards = crate::checks::ranges::incoming_guards::IncomingGuardIndex::build(
        program,
        call_frames.as_ref(),
    );
    record_permission_events_with_incoming_guards(program, facts, &incoming_guards)
        .expect("fixture permission production");
}

pub(crate) fn record_permission_events_with_incoming_guards(
    program: &typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    incoming_guards: &crate::checks::ranges::incoming_guards::IncomingGuardIndex,
) -> Result<(), Vec<Diagnostic>> {
    let mut permission_events = Vec::new();
    let mut conditional_carrier_transfers = Vec::new();
    let mut selection_diagnostics = Vec::new();
    facts.flow.ownership.owned_selections = arena::Arena::default();
    facts.flow.ownership.selection_sources = arena::Arena::default();
    facts.flow.ownership.selection_transfers = arena::Arena::default();
    facts.flow.ownership.selection_transfer_claims = arena::Arena::default();
    let mut claim_identities = ClaimIdentityAllocator::default();

    let state_flows = facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state)| state.clone())
        .collect::<Vec<_>>();
    for state_flow in state_flows {
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == state_flow.machine_symbol)
        else {
            continue;
        };
        let Some(state) = crate::semantic_calls::find_state(program, state_flow.state_symbol)
        else {
            continue;
        };
        let statements = program.statement_table.statements(state.statement_nodes);
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
                machine_symbol: state_flow.machine_symbol,
                state_symbol: state.symbol,
                source: PermissionEventSource::StateEntry,
                kind: PermissionEventKind::Establish,
                multiplicity: place.multiplicity,
                access: PermissionAccess::Owned,
                claim_identity,
                provenance: place.provenance.expect("entry place has provenance"),
                root: facts::PlaceRoot::Symbol(place.symbol),
                segments,
                obligation_live: true,
            });
        }

        let moves = crate::flow::discover_state_move_events(
            program,
            &facts.borrow,
            &facts.operators,
            machine,
            state,
            &mut facts.flow.ownership.segments,
        );
        let first_transition = statements
            .iter()
            .position(|statement| matches!(statement, StatementNode::Transition(_)));
        let prefix_end = first_transition.unwrap_or(statements.len());
        for (statement_index, statement) in statements[..prefix_end].iter().enumerate() {
            match owned_selection::record_statement(
                program,
                facts,
                machine,
                state,
                statement_index,
                statement,
                &mut places,
            ) {
                Ok(true) => continue,
                Ok(false) => {}
                Err(diagnostic) => {
                    selection_diagnostics.push(diagnostic);
                    continue;
                }
            }
            apply_statement_permission_production(
                program,
                facts,
                state_flow.machine_symbol,
                state.symbol,
                &moves,
                statement_index,
                statement,
                &mut places,
                &mut permission_events,
                &mut claim_identities,
                &mut conditional_carrier_transfers,
            );
        }

        if let Some(first_transition) = first_transition {
            let entry = places.clone();
            let arm_indices = (first_transition..statements.len())
                .filter(|index| matches!(statements[*index], StatementNode::Transition(_)))
                .collect::<Vec<_>>();
            for statement_index in arm_indices.iter().copied() {
                let mut outcome = entry.clone();
                if let Err(diagnostic) = owned_selection::record_statement(
                    program,
                    facts,
                    machine,
                    state,
                    statement_index,
                    &statements[statement_index],
                    &mut outcome,
                ) {
                    selection_diagnostics.push(diagnostic);
                    continue;
                }
                apply_statement_permission_production(
                    program,
                    facts,
                    state_flow.machine_symbol,
                    state.symbol,
                    &moves,
                    statement_index,
                    &statements[statement_index],
                    &mut outcome,
                    &mut permission_events,
                    &mut claim_identities,
                    &mut conditional_carrier_transfers,
                );
            }
        }

        append_affine_cleanup_permission_events(
            program,
            state,
            state_flow.machine_symbol,
            &places,
            &mut permission_events,
        );
        temporary_results::append_shared_borrow(
            program,
            facts,
            machine,
            state,
            &mut permission_events,
        );
    }

    append_borrow_permission_events(facts, &mut permission_events, &mut claim_identities);
    let claim_outcome_maps =
        reconcile_state_call_result_origins(program, &facts.flow.ownership, &mut permission_events);

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
    publish_conditional_claim_joins(program, facts);
    let joined_events = facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event.clone())
        .collect::<Vec<_>>();
    let joined_maps =
        derive_checked_claim_outcome_maps(program, &facts.flow.ownership, &joined_events);
    publish_claim_outcome_maps(facts, joined_maps);
    record_crash_frontier_lower_bounds(program, facts, incoming_guards);
    if selection_diagnostics.is_empty() {
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
fn record_crash_frontier_lower_bounds(
    program: &typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    incoming_guards: &crate::checks::ranges::incoming_guards::IncomingGuardIndex,
) {
    let mut derived = Vec::new();
    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == state_flow.machine_symbol)
        else {
            continue;
        };
        let Some(state) = crate::semantic_calls::find_state(program, state_flow.state_symbol)
        else {
            continue;
        };
        let statements = program.statement_table.statements(state.statement_nodes);
        let events = facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter_map(|(_, event)| {
                (event.machine_symbol == state_flow.machine_symbol
                    && event.state_symbol == state.symbol
                    && event.access == PermissionAccess::Owned)
                    .then_some(event)
            })
            .collect::<Vec<_>>();
        let mut places =
            initial_linear_places(program, state, state_flow.machine_symbol, state.symbol);
        apply_recorded_state_entry_events(&events, &facts.flow.ownership.segments, &mut places);
        let incoming = incoming_guards.for_machine(machine.symbol);
        let proven_conditional_claims =
            proven_conditional_entry_claims(program, state, incoming, &places);

        let first_transition = statements
            .iter()
            .position(|statement| matches!(statement, StatementNode::Transition(_)));
        let prefix_end = first_transition.unwrap_or(statements.len());
        let mut ignored_diagnostics = Vec::new();
        for statement_index in 0..prefix_end {
            if let Some((_, receipt)) = facts
                .flow
                .ownership
                .owned_selection_at(state.symbol, statement_index as u32)
            {
                owned_selection::apply_availability(
                    program,
                    &facts.flow.ownership,
                    receipt,
                    &mut places,
                );
                continue;
            }
            apply_recorded_statement_events(
                statement_index,
                &events,
                &facts.flow.ownership.segments,
                &mut places,
                &mut ignored_diagnostics,
            );
        }

        let Some(first_transition) = first_transition else {
            continue;
        };
        let entry = places;
        for statement_index in (first_transition..statements.len())
            .filter(|index| matches!(statements[*index], StatementNode::Transition(_)))
        {
            let StatementNode::Transition(transition) = &statements[statement_index] else {
                unreachable!("transition indices contain only transitions")
            };
            if !matches!(
                transition.exit,
                typed_trees::statement::TransitionExit::Crash(_)
            ) {
                continue;
            }
            let mut outcome = entry.clone();
            apply_recorded_statement_events(
                statement_index,
                &events,
                &facts.flow.ownership.segments,
                &mut outcome,
                &mut ignored_diagnostics,
            );
            let claims = outcome
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
                statement_ordinal: u32::try_from(statement_index)
                    .expect("state-local statement ordinal exceeds checked identity range"),
                claims,
            });
        }
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
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    incoming: &[crate::checks::ranges::incoming_guards::IncomingGuard],
    places: &[LinearPlace],
) -> Vec<PermissionClaimIdentity> {
    let parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let mut memberships = Vec::new();

    for entry in incoming.iter().filter(|entry| entry.holds_at(state.symbol)) {
        let mut case_tests = Vec::new();
        collect_positive_case_tests(program, entry.guard(), &mut case_tests);
        for (subject, variant) in case_tests {
            let Some(subject) = source_independent_case_subject(program, subject) else {
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
                    evidence.parameter == place.symbol
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

fn source_independent_case_subject(
    program: &typed_trees::TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<crate::flow::CanonicalPlace> {
    let place = crate::flow::canonical_place_from_expression(program, expression)?;
    if !matches!(place.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
        || !place.segments.iter().all(|segment| match segment {
            facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
            facts::PlaceSegment::Case { variant } => variant.is_valid(),
            facts::PlaceSegment::FixedIndex { .. } => true,
            facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::Index { .. } => false,
        })
    {
        return None;
    }
    Some(place)
}

/// Extract positive case-membership conjuncts. Boolean-arm lowering may wrap
/// a predicate as `predicate == true`; unwrap that shell but deliberately do
/// not infer through negation or disjunction here.
pub(crate) fn collect_positive_case_tests(
    program: &typed_trees::TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    tests: &mut Vec<(typed_trees::expression::ExpressionHandle, SymbolHandle)>,
) {
    use typed_trees::expression::{BinaryOperator, ExpressionNode};

    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
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
    program: &typed_trees::TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<(typed_trees::expression::ExpressionHandle, SymbolHandle)> {
    use typed_trees::expression::{BinaryOperator, ExpressionNode};

    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
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
    program: &typed_trees::TypedTrees,
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
            return false;
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
    program.data_definitions().iter().any(|definition| {
        let variants = program
            .data_members(definition)
            .iter()
            .filter_map(|member| match member {
                typed_trees::data::DataMember::Variant(variant) => Some(variant.symbol),
                _ => None,
            })
            .collect::<Vec<_>>();
        variants.contains(first_variant)
            && variants.len() == covered.len()
            && variants.iter().all(|variant| covered.contains(variant))
    })
}

pub(crate) fn select_case_alternative_from_guard(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    subject: typed_trees::expression::ExpressionHandle,
    variant: SymbolHandle,
    places: &mut [LinearPlace],
) {
    let Some(place) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        subject,
    ) else {
        return;
    };
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return;
    };
    let mut selected_path = place.segments;
    selected_path.push(facts::PlaceSegment::Case { variant });
    select_static_case_alternative(symbol, &selected_path, places);
}

pub(crate) fn exclude_case_alternative(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    subject: typed_trees::expression::ExpressionHandle,
    variant: SymbolHandle,
    places: &mut [LinearPlace],
) {
    let Some(place) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        subject,
    ) else {
        return;
    };
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return;
    };
    let case_index = place.segments.len();
    for candidate in places.iter_mut().filter(|candidate| {
        candidate.symbol == symbol
            && candidate.path.get(..case_index) == Some(place.segments.as_slice())
            && candidate.path.get(case_index) == Some(&facts::PlaceSegment::Case { variant })
    }) {
        candidate.live = false;
        candidate.case_excluded = true;
    }
}

/// Join a state call's receiving establishment to the unique claim and
/// root-lineage provenance that the target state transferred through its
/// result.
///
/// Intra-state production can propagate provenance directly through source
/// expressions. A zero-argument state call has no caller-side source place,
/// though: without this join, binding a locally-created linear result in the
/// caller would mint a second identity and origin even when the target has one
/// unambiguous outgoing obligation. Direct aggregate construction and
/// path-aligned results publish structural output paths; opaque multi-output
/// calls remain conservative until they publish an explicit result mapping.
fn reconcile_state_call_result_origins(
    program: &typed_trees::TypedTrees,
    ownership: &checked_trees::FlowOwnershipFacts,
    permission_events: &mut [FlowPermissionEventFact],
) -> Vec<CheckedClaimOutcomeMap> {
    let segments = &ownership.segments;
    let iteration_limit = permission_events.len().saturating_add(1);
    for _ in 0..iteration_limit {
        let maps = derive_checked_claim_outcome_maps(program, ownership, permission_events);
        let rewrites = call_result_origin_rewrites(program, segments, permission_events, &maps);
        let origins_changed = apply_claim_origin_rewrites(permission_events, &rewrites);
        let liveness_changed =
            apply_statically_inactive_call_results(program, segments, permission_events, &maps);
        if !origins_changed && !liveness_changed {
            return derive_checked_claim_outcome_maps(program, ownership, permission_events);
        }
    }
    derive_checked_claim_outcome_maps(program, ownership, permission_events)
}

fn apply_statically_inactive_call_results(
    program: &typed_trees::TypedTrees,
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
            let PermissionEventSource::Statement { statement_index } = event.source else {
                return None;
            };
            let state = crate::semantic_calls::find_state(program, event.state_symbol)?;
            let statement = program
                .statement_table
                .statements(state.statement_nodes)
                .get(statement_index)?;
            let expression = match statement {
                StatementNode::LocalData(local) => local.initial_value,
                StatementNode::Assignment(assignment) => assignment.value,
                _ => return None,
            };
            let typed_trees::expression::ExpressionNode::Call(call) =
                program.expression_table.expression(expression)
            else {
                return None;
            };
            let target = crate::semantic_calls::find_state(program, call.target_symbol)?;
            let map = maps.iter().find(|map| map.state_symbol == target.symbol)?;
            let receiving_path = segments.span_or_empty(event.segments);
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
