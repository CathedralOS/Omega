//! Validating linear permission events statement by statement: the claim
//! frontier, borrow events, permission production and affine cleanup.
//!
//! This file validates one machine. `claim_frontier.rs` builds the linear
//! claim frontier, `recorded_events.rs` replays recorded entry, statement
//! and borrow events, `permission_production.rs` applies statement
//! permission production, `claim_provenance.rs` names claim identities and
//! provenance and `affine_cleanup.rs` appends affine cleanup events.

mod affine_cleanup;
mod claim_frontier;
mod claim_provenance;
mod permission_production;
mod recorded_events;

pub(crate) use affine_cleanup::append_affine_cleanup_permission_events;
pub(crate) use claim_frontier::{initial_linear_places, linear_claim_frontier};
pub(crate) use permission_production::{
    apply_statement_permission_production, established_provenance, event_statement_index,
    permission_kind_for_move, permission_source, select_static_case_alternative,
};
pub(crate) use recorded_events::{
    append_borrow_permission_events, apply_recorded_state_entry_events,
    apply_recorded_statement_events, permission_event_statement_index,
};

use crate::checks::multiplicity::owned_selection;
use crate::checks::multiplicity::permission_events::{
    case_transition_run_is_exhaustive, collect_positive_case_tests, exact_positive_case_test,
    exclude_case_alternative, select_case_alternative_from_guard,
};
use crate::checks::multiplicity::type_multiplicity::type_carries_linear_obligation;
use checked_trees::{CheckFacts, FlowClaimOutcomeSource, FlowFacts, FlowPermissionEventFact};
use diagnostics::Diagnostic;
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource,
};
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;

pub(crate) fn validate_linear_permission_events(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    // `record_statement` consults the borrow ledger and the statement-entry
    // constraint sets to decide whether a once-borrowed source may join a
    // selection. Those are inputs recorded by earlier passes, so the replay
    // seeds them unchanged while the ownership arenas it fills stay fresh.
    let mut selected_replay = CheckFacts {
        borrow: facts.borrow.clone(),
        flow: FlowFacts {
            contexts: facts.flow.contexts.clone(),
            control: facts.flow.control.clone(),
            ..Default::default()
        },
        ..Default::default()
    };

    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(state) = crate::semantic_calls::find_state(program, state_flow.state_symbol)
        else {
            continue;
        };
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == state_flow.machine_symbol)
        else {
            continue;
        };
        let statements = program.statement_table.statements(state.statement_nodes);
        let mut places =
            initial_linear_places(program, state, state_flow.machine_symbol, state.symbol);
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
        // Replay entry establishment so a parameter source's minted claim
        // identity reaches the receipt roster exactly as the recording pass
        // observed it.
        apply_recorded_state_entry_events(&events, &facts.flow.ownership.segments, &mut places);
        append_unresolved_state_result_mapping_diagnostics(
            program,
            state,
            &events,
            &facts.flow.ownership,
            &mut diagnostics,
        );

        let first_transition = statements
            .iter()
            .position(|statement| matches!(statement, StatementNode::Transition(_)));
        let prefix_end = first_transition.unwrap_or(statements.len());
        for (statement_index, statement) in statements[..prefix_end].iter().enumerate() {
            match owned_selection::record_statement(
                program,
                &mut selected_replay,
                machine,
                state,
                statement_index,
                statement,
                &mut places,
            ) {
                Ok(true) => continue,
                Ok(false) => {}
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            }
            apply_recorded_statement_events(
                statement_index,
                &events,
                &facts.flow.ownership.segments,
                &mut places,
                &mut diagnostics,
            );
        }

        let mut mixed_places = Vec::new();
        let mut has_ordinary_exit = first_transition.is_none();
        if let Some(first_transition) = first_transition {
            let entry = places.clone();
            let arm_indices = (first_transition..statements.len())
                .filter(|index| matches!(statements[*index], StatementNode::Transition(_)))
                .collect::<Vec<_>>();
            let mut outcomes = Vec::new();
            let mut excluded_case_tests = Vec::new();
            for statement_index in arm_indices.iter().copied() {
                let mut outcome = entry.clone();
                if let Err(diagnostic) = owned_selection::record_statement(
                    program,
                    &mut selected_replay,
                    machine,
                    state,
                    statement_index,
                    &statements[statement_index],
                    &mut outcome,
                ) {
                    diagnostics.push(diagnostic);
                    continue;
                }
                for &(subject, variant) in &excluded_case_tests {
                    exclude_case_alternative(
                        program,
                        state.symbol,
                        statement_index,
                        subject,
                        variant,
                        &mut outcome,
                    );
                }
                let StatementNode::Transition(transition) = &statements[statement_index] else {
                    unreachable!("transition indices contain only transitions")
                };
                if let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
                    let mut selected = Vec::new();
                    collect_positive_case_tests(program, guard, &mut selected);
                    for (subject, variant) in selected {
                        select_case_alternative_from_guard(
                            program,
                            state.symbol,
                            statement_index,
                            subject,
                            variant,
                            &mut outcome,
                        );
                    }
                }
                apply_recorded_statement_events(
                    statement_index,
                    &events,
                    &facts.flow.ownership.segments,
                    &mut outcome,
                    &mut diagnostics,
                );
                if transition.exit == typed_trees::statement::TransitionExit::Ordinary {
                    outcomes.push(outcome);
                }
                if let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard
                    && let Some(case_test) = exact_positive_case_test(program, guard)
                {
                    excluded_case_tests.push(case_test);
                }
            }
            let exhaustive =
                arm_indices.last().is_some_and(|index| {
                    matches!(
                        statements[*index],
                        StatementNode::Transition(typed_trees::statement::TableTransition {
                            guard: typed_trees::statement::TransitionGuardNode::Always,
                            ..
                        })
                    )
                }) || case_transition_run_is_exhaustive(program, statements, &arm_indices);
            if !exhaustive {
                outcomes.push(entry);
            }
            has_ordinary_exit = !outcomes.is_empty();

            if let Some(first) = outcomes.first() {
                for place_index in 0..places.len() {
                    let live = first[place_index].live;
                    if places[place_index].multiplicity == Multiplicity::Linear
                        && outcomes
                            .iter()
                            .skip(1)
                            .any(|outcome| outcome[place_index].live != live)
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "linear value `{}` has inconsistent treatment across transition arms; every path must consume/transfer it or every path must preserve the same live obligation",
                            places[place_index].name
                        )));
                        mixed_places
                            .push((places[place_index].symbol, places[place_index].path.clone()));
                    } else {
                        places[place_index].live = live;
                        places[place_index].ever_established = outcomes
                            .iter()
                            .any(|outcome| outcome[place_index].ever_established);
                    }
                }
            }
        }

        if has_ordinary_exit {
            for place in places.iter().filter(|place| {
                place.multiplicity == Multiplicity::Linear
                    && place.live
                    && !mixed_places
                        .iter()
                        .any(|(symbol, path)| *symbol == place.symbol && *path == place.path)
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "linear value `{}` reaches scope exit without being consumed or transferred",
                    place.name
                )));
            }
        }
    }

    if !owned_selections_match(&selected_replay.flow.ownership, &facts.flow.ownership) {
        diagnostics.push(Diagnostic::error(
            "owned selection receipts differ from source ownership replay",
        ));
    }
    for (_, receipt) in selected_replay.flow.ownership.owned_selections.iter() {
        let sources = selected_replay
            .flow
            .ownership
            .selection_sources
            .span_or_empty(receipt.sources);
        // A linear receipt consumes only the exact claim each transfer names;
        // residual siblings of a source root remain live custody with their
        // own later events. Only an event overlapping a consumed path on a
        // source root is a second life for the same claim.
        let consumed_places: Vec<(SymbolHandle, Vec<facts::PlaceSegment>)> =
            if program.type_multiplicity(receipt.type_reference) == Multiplicity::Linear {
                selected_replay
                    .flow
                    .ownership
                    .selection_transfers
                    .span_or_empty(receipt.transfers)
                    .iter()
                    .filter(|transfer| transfer.source.is_valid())
                    .flat_map(|transfer| {
                        let symbol = selected_replay
                            .flow
                            .ownership
                            .selection_sources
                            .get(transfer.source)
                            .symbol;
                        let claims = selected_replay
                            .flow
                            .ownership
                            .selection_transfer_claims
                            .span_or_empty(transfer.claims);
                        if claims.is_empty() {
                            vec![(
                                symbol,
                                selected_replay
                                    .flow
                                    .ownership
                                    .segments
                                    .span_or_empty(transfer.path)
                                    .to_vec(),
                            )]
                        } else {
                            claims
                                .iter()
                                .map(|claim| {
                                    (
                                        symbol,
                                        selected_replay
                                            .flow
                                            .ownership
                                            .segments
                                            .span_or_empty(claim.path)
                                            .to_vec(),
                                    )
                                })
                                .collect()
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };
        if facts.flow.ownership.permissions.iter().any(|(_, event)| {
            if event.machine_symbol != receipt.machine
                || event.state_symbol != receipt.state
                || event.access != PermissionAccess::Owned
            {
                return false;
            }
            let statement = permission_event_statement_index(event.source);
            if statement == Some(receipt.statement_ordinal as usize) {
                return true;
            }
            if event.source != PermissionEventSource::StateExit
                && !statement
                    .is_some_and(|statement| statement > receipt.statement_ordinal as usize)
            {
                return false;
            }
            if consumed_places.is_empty() {
                return sources
                    .iter()
                    .any(|source| event.root == facts::PlaceRoot::Symbol(source.symbol));
            }
            let event_path = facts.flow.ownership.segments.span_or_empty(event.segments);
            consumed_places.iter().any(|(symbol, path)| {
                event.root == facts::PlaceRoot::Symbol(*symbol)
                    && owned_selection::place_paths_overlap(event_path, path)
            })
        }) {
            diagnostics.push(Diagnostic::error("owned selection acquired unconditional transfer, establishment, or residual cleanup events"));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// The replay populates a fresh ownership arena, so span offsets into
/// `segments`, `selection_sources`, and `selection_transfers` are positional
/// rather than semantic. Compare each recorded receipt field-for-field,
/// resolving spans through the side that produced them.
fn owned_selections_match(
    replay: &checked_trees::FlowOwnershipFacts,
    recorded: &checked_trees::FlowOwnershipFacts,
) -> bool {
    let replay_receipts = replay
        .owned_selections
        .iter()
        .map(|(_, receipt)| receipt)
        .collect::<Vec<_>>();
    let recorded_receipts = recorded
        .owned_selections
        .iter()
        .map(|(_, receipt)| receipt)
        .collect::<Vec<_>>();
    if replay_receipts.len() != recorded_receipts.len() {
        return false;
    }
    for (replayed_receipt, recorded_receipt) in replay_receipts.into_iter().zip(recorded_receipts) {
        if replayed_receipt.machine != recorded_receipt.machine
            || replayed_receipt.state != recorded_receipt.state
            || replayed_receipt.statement_ordinal != recorded_receipt.statement_ordinal
            || replayed_receipt.expression != recorded_receipt.expression
            || replayed_receipt.destination != recorded_receipt.destination
            || replayed_receipt.type_reference != recorded_receipt.type_reference
            || replayed_receipt.death != recorded_receipt.death
            || replay
                .selection_sources
                .span_or_empty(replayed_receipt.sources)
                != recorded
                    .selection_sources
                    .span_or_empty(recorded_receipt.sources)
        {
            return false;
        }
        let replayed_transfers = replay
            .selection_transfers
            .span_or_empty(replayed_receipt.transfers);
        let recorded_transfers = recorded
            .selection_transfers
            .span_or_empty(recorded_receipt.transfers);
        if replayed_transfers.len() != recorded_transfers.len() {
            return false;
        }
        for (replayed_transfer, recorded_transfer) in
            replayed_transfers.iter().zip(recorded_transfers)
        {
            if replayed_transfer.expression != recorded_transfer.expression
                || replayed_transfer.source_arm != recorded_transfer.source_arm
                || replayed_transfer.source != recorded_transfer.source
                || replay.segments.span_or_empty(replayed_transfer.path)
                    != recorded.segments.span_or_empty(recorded_transfer.path)
            {
                return false;
            }
            // The consumed claim set is part of the transfer's identity: a
            // whole carrier's replay must reproduce the same ordered claim
            // rows, each with the consumed place's own identity and
            // provenance, or the recorded set was tampered with.
            let replayed_claims = replay
                .selection_transfer_claims
                .span_or_empty(replayed_transfer.claims);
            let recorded_claims = recorded
                .selection_transfer_claims
                .span_or_empty(recorded_transfer.claims);
            if replayed_claims.len() != recorded_claims.len() {
                return false;
            }
            for (replayed_claim, recorded_claim) in replayed_claims.iter().zip(recorded_claims) {
                if replay.segments.span_or_empty(replayed_claim.path)
                    != recorded.segments.span_or_empty(recorded_claim.path)
                    || replayed_claim.claim_identity != recorded_claim.claim_identity
                    || replayed_claim.provenance != recorded_claim.provenance
                {
                    return false;
                }
            }
        }
    }
    true
}

fn append_unresolved_state_result_mapping_diagnostics(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    events: &[&FlowPermissionEventFact],
    ownership: &checked_trees::FlowOwnershipFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut unresolved_statements = Vec::new();
    for event in events {
        if event.kind != PermissionEventKind::Establish
            || event.access != PermissionAccess::Owned
            || !event.obligation_live
        {
            continue;
        }
        let PermissionEventSource::Statement { statement_index } = event.source else {
            continue;
        };
        let Some(statement) = statements.get(statement_index) else {
            continue;
        };
        let result_expression = match statement {
            StatementNode::LocalData(local) => local.initial_value,
            StatementNode::Assignment(assignment) => assignment.value,
            _ => continue,
        };
        let typed_trees::expression::ExpressionNode::Call(call) =
            program.expression_table.expression(result_expression)
        else {
            continue;
        };
        let Some(target_state) = crate::semantic_calls::find_state(program, call.target_symbol)
        else {
            continue;
        };
        if program
            .statement_table
            .statements(target_state.statement_nodes)
            .is_empty()
            || !type_carries_linear_obligation(program, target_state.return_type)
        {
            continue;
        }
        let mapped_from_call = ownership.permissions.iter().any(|(_, candidate)| {
            candidate.kind == PermissionEventKind::Transfer
                && candidate.access == PermissionAccess::Owned
                && candidate.obligation_live
                && candidate.claim_identity != PermissionClaimIdentity::Unknown
                && candidate.claim_identity == event.claim_identity
                && (candidate.state_symbol == target_state.symbol
                    || (candidate.state_symbol == state.symbol
                        && permission_event_statement_index(candidate.source)
                            == Some(statement_index)))
        });
        let receiving_path = ownership.segments.span_or_empty(event.segments);
        let mapped_by_checked_outcome = ownership
            .claim_outcome_maps
            .iter()
            .filter(|(_, map)| map.state_symbol == target_state.symbol)
            .flat_map(|(_, map)| ownership.claim_outcome_entries.span_or_empty(map.entries))
            .any(|entry| {
                ownership.segments.span_or_empty(entry.output_segments) == receiving_path
                    && matches!(
                        entry.source,
                        FlowClaimOutcomeSource::Established {
                            claim_identity,
                            provenance,
                        } if claim_identity == event.claim_identity
                            && provenance == event.provenance
                    )
            });
        if mapped_from_call || mapped_by_checked_outcome {
            continue;
        }
        if !unresolved_statements.contains(&statement_index) {
            unresolved_statements.push(statement_index);
        }
    }

    for statement_index in unresolved_statements {
        diagnostics.push(Diagnostic::error(format!(
            "linear state-call result at statement {statement_index} has no unique conserved claim mapping; return a path-aligned source place or publish an explicit outcome mapping"
        )));
    }
}
