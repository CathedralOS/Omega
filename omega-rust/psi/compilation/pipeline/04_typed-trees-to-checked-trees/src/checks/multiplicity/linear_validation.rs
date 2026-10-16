//! Validating linear permission events statement by statement: the claim
//! frontier, borrow events, permission production and affine cleanup.
//!
//! This file validates one machine. `claim_frontier.rs` builds the linear
//! claim frontier, `applied_events.rs` applies recorded entry, statement
//! and borrow events, `permission_production.rs` applies statement
//! permission production, `claim_provenance.rs` names claim identities and
//! provenance and `affine_cleanup.rs` derives and replays owner-authorized exit cleanup.

mod affine_cleanup;
mod applied_events;
mod claim_frontier;
mod claim_provenance;
pub(crate) mod loan_completions;
mod permission_production;

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
    apply_statement_permission_production, established_provenance, event_statement_index,
    permission_source, record_conditional_carrier_transfer, select_static_case_alternative,
};

use crate::checks::multiplicity::owned_selection;
use crate::checks::multiplicity::type_multiplicity::type_carries_linear_obligation;
use checked_trees::{CheckFacts, FlowClaimOutcomeSource, FlowPermissionEventFact};
use diagnostics::Diagnostic;
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource,
};

pub(crate) fn validate_linear_permission_events(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    super::claim_outcomes::validate_call_claim_joins(
        program,
        &facts.borrow,
        &facts.flow.ownership,
        &mut diagnostics,
    );
    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(state) = program.state_by_symbol(state_flow.state_symbol) else {
            continue;
        };
        let Some(machine) = program.machine_by_symbol(state_flow.machine_symbol) else {
            continue;
        };
        // A bodyless contract state declares the crossing as its parameter
        // disposition: custody transfers to the provider at the call edge, so
        // no in-body scope exit discharges its claims.
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
        // Anonymous custody comes from the producer's published roster, not a
        // second AST discovery. Replay checks the same identities through the
        // same establishment/transfer/consumption rules as named owners.
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

        let mut outcomes = Vec::new();
        super::permission_schedule::walk(
            program,
            state,
            (
                places.clone(),
                owned_selection::SelectionOccurrences::default(),
            ),
            |step, custody| {
                let (places, selections) = custody;
                super::permission_schedule::select_cases(program, state, step, places);
                match step {
                    super::permission_schedule::PermissionStep::Evaluate {
                        statement,
                        evaluation,
                    } => {
                        // The recording pass already decided which statements are
                        // owned selections and kept one receipt for each. Apply
                        // that receipt's availability exactly as recording did,
                        // instead of recording the selection a second time.
                        if !matches!(
                            evaluation,
                            checked_trees::FlowPermissionEvaluation::TransitionTarget(_)
                        ) && let Some((_, receipt)) = facts
                            .flow
                            .ownership
                            .owned_selections
                            .iter()
                            .find(|(_, receipt)| {
                                receipt.machine == machine.symbol
                                    && receipt.state == state.symbol
                                    && receipt.statement_ordinal as usize == statement
                            })
                        {
                            if events.iter().any(|event| {
                                permission_event_statement_index(event.source) == Some(statement)
                            }) {
                                diagnostics.push(Diagnostic::error("owned selection acquired unconditional transfer, establishment, or residual cleanup events"));
                            }
                            selections.consume(program, &facts.flow.ownership, receipt);
                            owned_selection::apply_availability(
                                program,
                                &facts.flow.ownership,
                                receipt,
                                places,
                            );
                            return;
                        }
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
                    }
                    super::permission_schedule::PermissionStep::Exit {
                        coordinate,
                        crash: false,
                    } => {
                        if let Err(diagnostic) = selections.check_events(
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
                        outcomes.push(places.clone())
                    }
                    _ => {}
                }
            },
        );
        let mut mixed_places = Vec::new();
        let has_ordinary_exit = !outcomes.is_empty();
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
                    mixed_places.push((places[place_index].root, places[place_index].path.clone()));
                } else {
                    places[place_index].live = live;
                    places[place_index].ever_established = outcomes
                        .iter()
                        .any(|outcome| outcome[place_index].ever_established);
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
                diagnostics.push(Diagnostic::error(format!(
                    "linear value `{}` reaches scope exit without being consumed or transferred",
                    place.name
                )));
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn append_unresolved_state_result_mapping_diagnostics(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    events: &[&FlowPermissionEventFact],
    ownership: &checked_trees::FlowOwnershipFacts,
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
                && receipt.target_symbol == call.target_symbol
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
        if !unresolved_statements
            .iter()
            .any(|(index, _)| *index == statement_index)
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
            unresolved_statements.push((statement_index, format!("{root}{path}")));
        }
    }

    let machine_name = validation::find_state(program, state.symbol)
        .map(|(machine, _)| machine.name.as_str())
        .unwrap_or("<unknown machine>");
    for (statement_index, path) in unresolved_statements {
        diagnostics.push(Diagnostic::error(format!(
            "linear state-call result {path} in {machine_name}::{} at statement {statement_index} has no unique conserved claim mapping; return a path-aligned source place or publish an explicit outcome mapping",
            program.symbols.name(state.symbol)
        )));
    }
}
