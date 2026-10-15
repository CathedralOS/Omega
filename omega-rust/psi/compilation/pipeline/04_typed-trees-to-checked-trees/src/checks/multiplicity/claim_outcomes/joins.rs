//! Bind returned custody to an exact call occurrence, separately from lineage.
//! A callee-local establishment needs this correspondence even with one exit:
//! its static identity cannot identify two simultaneously live invocations.
//! Input forwarding keeps the caller's held identity; differing exit sources
//! retain every alternative rather than manufacturing an unconditional lineage.

use super::{
    ClaimAbsence, argument_for_parameter, bind_claim_outcome_source_at_arguments,
    bind_claim_outcome_source_at_call, claim_outcomes_for_expression,
    claim_paths_are_case_alternatives, derive_checked_claim_outcome_maps, parameter_for_symbol,
};
use crate::checks::multiplicity::linear_obligations::{
    CheckedClaimOutcomeEntry, CheckedClaimOutcomeMap, CheckedClaimOutcomeSource,
};
use crate::checks::multiplicity::linear_validation::linear_claim_frontier;
use checked_trees::{
    CheckFacts, FlowClaimJoinAlternative, FlowClaimJoinAlternativeSource, FlowClaimJoinExit,
    FlowClaimJoinExitKind, FlowClaimJoinReceipt, FlowClaimOutcomeSource, FlowOwnershipFacts,
    FlowPermissionEventFact,
};
use diagnostics::Diagnostic;
use facts::{PlaceRoot, PlaceSegment};
use language_semantics::{
    PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    PermissionProvenance,
};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExitFrontier {
    exits: Vec<FlowClaimJoinExit>,
    entries: Vec<CheckedClaimOutcomeEntry>,
    inactive_paths: Vec<Vec<PlaceSegment>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Alternative {
    exits: Vec<FlowClaimJoinExit>,
    source: Option<(CheckedClaimOutcomeSource, CheckedClaimOutcomeSource)>,
    excluded_input: Option<ExcludedInput>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExcludedInput {
    source: CheckedClaimOutcomeSource,
    root: PlaceRoot,
    path: Vec<PlaceSegment>,
    established_at: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Receipt {
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    ordinal: usize,
    target: SymbolHandle,
    expression: ExpressionHandle,
    root: PlaceRoot,
    path: Vec<PlaceSegment>,
    identity: PermissionClaimIdentity,
    alternatives: Vec<Alternative>,
}

fn known_source(source: &CheckedClaimOutcomeSource) -> bool {
    match source {
        CheckedClaimOutcomeSource::Input {
            parameter_symbol,
            path,
        } => {
            parameter_symbol.is_valid()
                && path.iter().all(|segment| {
                    matches!(segment,
                PlaceSegment::Field { symbol } if symbol.is_valid())
                        || matches!(segment, PlaceSegment::Case { variant } if variant.is_valid())
                        || matches!(segment, PlaceSegment::FixedIndex { .. })
                })
        }
        CheckedClaimOutcomeSource::Established {
            claim_identity,
            provenance,
        } => {
            *claim_identity != PermissionClaimIdentity::Unknown
                && *provenance != PermissionProvenance::Unknown
        }
    }
}

/// No exit may disappear through filter_map. Missing facts, unknown sources,
/// and recursive expansion all make the complete correspondence unavailable.
fn state_frontiers(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    state: &typed_trees::state::State,
    ownership: &FlowOwnershipFacts,
    events: &[FlowPermissionEventFact],
    maps: &[CheckedClaimOutcomeMap],
    active: &mut Vec<SymbolHandle>,
) -> Option<Vec<ExitFrontier>> {
    if active.contains(&state.symbol) {
        return None;
    }
    active.push(state.symbol);
    let result = (|| {
        let statements = typed_trees::statement::reachable_state_statements(
            program.statement_table.statements(state.statement_nodes),
        );
        let transition_indices = statements
            .iter()
            .enumerate()
            .filter_map(|(index, statement)| {
                matches!(statement, StatementNode::Transition(_)).then_some(index)
            })
            .collect::<Vec<_>>();
        if let Some(last) = transition_indices.last() {
            let StatementNode::Transition(transition) = &statements[*last] else {
                return None;
            };
            if !matches!(transition.guard, typed_trees::statement::TransitionGuardNode::Always)
                && !transition.continuation.is_valid()
                && !crate::checks::multiplicity::permission_events::case_transition_run_is_exhaustive(
                    program, statements, &transition_indices)
            { return None; }
        }
        let mut frontiers = Vec::new();
        for (statement_index, statement) in statements.iter().enumerate() {
            match statement {
                StatementNode::Expression(expression)
                    if statement_index + 1 == statements.len() =>
                {
                    frontiers.extend(expression_frontiers(
                        program,
                        borrow,
                        state,
                        statement_index,
                        *expression,
                        ownership,
                        events,
                        maps,
                        active,
                        FlowClaimJoinExitKind::FinalExpression,
                    )?);
                }
                StatementNode::Transition(transition) => {
                    if transition.exit != TransitionExit::Ordinary {
                        continue;
                    }
                    for (target, kind) in [
                        (transition.target, FlowClaimJoinExitKind::TransitionTarget),
                        (
                            transition.continuation,
                            FlowClaimJoinExitKind::TransitionContinuation,
                        ),
                    ] {
                        if !target.is_valid() {
                            continue;
                        }
                        match program.statement_table.transition_target(target) {
                            TransitionTargetNode::Value(expression) => {
                                frontiers.extend(expression_frontiers(
                                    program,
                                    borrow,
                                    state,
                                    statement_index,
                                    *expression,
                                    ownership,
                                    events,
                                    maps,
                                    active,
                                    kind,
                                )?);
                            }
                            TransitionTargetNode::Named {
                                path, arguments, ..
                            } => {
                                let target_state =
                                    validation::call_target_state(program, path.symbol)?;
                                let mut children = state_frontiers(
                                    program,
                                    borrow,
                                    target_state,
                                    ownership,
                                    events,
                                    maps,
                                    active,
                                )?;
                                for child in &mut children {
                                    for entry in &mut child.entries {
                                        entry.source = bind_claim_outcome_source_at_arguments(
                                            program,
                                            state,
                                            statement_index,
                                            program.statement_table.expression_handles(*arguments),
                                            target_state,
                                            &entry.source,
                                            &ownership.segments,
                                            events,
                                        )?;
                                    }
                                    child.exits.insert(
                                        0,
                                        FlowClaimJoinExit {
                                            state_symbol: state.symbol,
                                            statement_index,
                                            kind,
                                        },
                                    );
                                    if !complete_frontier(
                                        program,
                                        state.return_type,
                                        &child.entries,
                                        &child.inactive_paths,
                                    ) {
                                        return None;
                                    }
                                }
                                frontiers.extend(children);
                            }
                            _ => return None,
                        }
                    }
                }
                _ => {}
            }
        }
        (!frontiers.is_empty()).then_some(frontiers)
    })();
    active.pop();
    result
}

#[allow(clippy::too_many_arguments)]
fn expression_frontiers(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    state: &typed_trees::state::State,
    statement: usize,
    expression: ExpressionHandle,
    ownership: &FlowOwnershipFacts,
    events: &[FlowPermissionEventFact],
    maps: &[CheckedClaimOutcomeMap],
    active: &mut Vec<SymbolHandle>,
    kind: FlowClaimJoinExitKind,
) -> Option<Vec<ExitFrontier>> {
    let edge = FlowClaimJoinExit {
        state_symbol: state.symbol,
        statement_index: statement,
        kind,
    };
    let entries = claim_outcomes_for_expression(
        program,
        state,
        statement,
        expression,
        ownership,
        events,
        maps,
        &[],
    );
    let inactive_paths = linear_claim_frontier(program, state.return_type)
        .into_iter()
        .filter(|claim| {
            claim.conditional
                && ClaimAbsence {
                    program,
                    borrow,
                    state,
                    ownership,
                    events,
                    maps,
                }
                .expression(statement, expression, &claim.path)
        })
        .map(|claim| claim.path)
        .collect::<Vec<_>>();
    if complete_frontier(program, state.return_type, &entries, &inactive_paths) {
        return expand_joined_frontier(
            program,
            borrow,
            state,
            ownership,
            events,
            maps,
            active,
            &[],
            ExitFrontier {
                exits: vec![edge],
                entries,
                inactive_paths,
            },
        );
    }
    let call = program.expression_table.call_node(expression)?;
    let target = validation::call_target_state(program, call.target_symbol)?;
    let mut frontiers = state_frontiers(program, borrow, target, ownership, events, maps, active)?;
    for frontier in &mut frontiers {
        for entry in &mut frontier.entries {
            entry.source = bind_claim_outcome_source_at_call(
                program,
                state,
                statement,
                call,
                target,
                &entry.source,
                &ownership.segments,
                events,
            )?;
        }
        frontier.exits.insert(0, edge);
        if !complete_frontier(
            program,
            state.return_type,
            &frontier.entries,
            &frontier.inactive_paths,
        ) {
            return None;
        }
    }
    Some(frontiers)
}

/// A wrapper cannot reuse its callee's local joined occurrence as a global
/// resource id. Reconstruct the producing call and substitute its complete
/// alternatives together, preserving correlations between returned fields.
#[allow(clippy::too_many_arguments)]
fn expand_joined_frontier(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    state: &typed_trees::state::State,
    ownership: &FlowOwnershipFacts,
    events: &[FlowPermissionEventFact],
    maps: &[CheckedClaimOutcomeMap],
    active: &mut Vec<SymbolHandle>,
    visited: &[(SymbolHandle, usize, usize)],
    frontier: ExitFrontier,
) -> Option<Vec<ExitFrontier>> {
    let joined = frontier
        .entries
        .iter()
        .find_map(|entry| match entry.source {
            CheckedClaimOutcomeSource::Established {
                provenance:
                    PermissionProvenance::Joined {
                        machine_symbol,
                        state_symbol,
                        source:
                            PermissionEventSource::Call {
                                statement_index,
                                call_ordinal,
                                target_symbol,
                            },
                        ..
                    },
                ..
            } => Some((
                machine_symbol,
                state_symbol,
                statement_index,
                call_ordinal,
                target_symbol,
            )),
            _ => None,
        });
    let Some((machine, owner, statement, ordinal, target_symbol)) = joined else {
        return Some(vec![frontier]);
    };
    if owner != state.symbol {
        return None;
    }
    let invocation = (owner, statement, ordinal);
    if visited.contains(&invocation) {
        return None;
    }
    let mut visited = visited.to_vec();
    visited.push(invocation);
    let validation::CallSite::Expression {
        call,
        expression: call_expression,
        ..
    } = validation::find_call_site(program, machine, owner, statement, ordinal)?
    else {
        return None;
    };
    if call.target_symbol != target_symbol {
        return None;
    }
    let target = validation::call_target_state(program, target_symbol)?;
    let children = state_frontiers(program, borrow, target, ownership, events, maps, active)?;
    let mut expanded = Vec::new();
    for child in children {
        let mut next = frontier.clone();
        next.exits.push(FlowClaimJoinExit {
            state_symbol: owner,
            statement_index: statement,
            kind: FlowClaimJoinExitKind::CallResult {
                call_ordinal: ordinal,
            },
        });
        next.exits.extend(child.exits);
        let mut entries = Vec::new();
        for mut entry in next.entries {
            let CheckedClaimOutcomeSource::Established {
                claim_identity,
                provenance:
                    PermissionProvenance::Joined {
                        machine_symbol,
                        state_symbol,
                        source:
                            PermissionEventSource::Call {
                                statement_index,
                                call_ordinal,
                                target_symbol: source_target,
                            },
                        ..
                    },
            } = entry.source
            else {
                entries.push(entry);
                continue;
            };
            if (
                machine_symbol,
                state_symbol,
                statement_index,
                call_ordinal,
                source_target,
            ) != (machine, owner, statement, ordinal, target_symbol)
            {
                entries.push(entry);
                continue;
            }
            let mut origins = events
                .iter()
                .filter(|event| {
                    event.kind == PermissionEventKind::Establish
                        && event.state_symbol == owner
                        && event.claim_identity == claim_identity
                })
                .filter_map(|event| {
                    let (established_at, expression, path) =
                        super::receiving_value(program, &ownership.segments, event)?;
                    (established_at == statement && expression == call_expression).then_some(path)
                });
            let path = origins.next()?;
            if origins.next().is_some() {
                return None;
            }
            // The receiving home may contain this call beneath constructor
            // fields or indexes. Only its result-relative suffix can select
            // the callee's return frontier; storage prefixes are not lineage.
            if child.inactive_paths.iter().any(|inactive| inactive == path) {
                next.inactive_paths.push(entry.output_path);
                continue;
            }
            let mut matching = child
                .entries
                .iter()
                .filter(|candidate| candidate.output_path == path);
            let source = &matching.next()?.source;
            if matching.next().is_some() {
                return None;
            }
            entry.source = bind_claim_outcome_source_at_call(
                program,
                state,
                statement,
                call,
                target,
                source,
                &ownership.segments,
                events,
            )?;
            // Expansion explains lineage; it must not replace the exact
            // caller occurrence with a shared callee establishment template.
            // The child frontier already rejects duplicate return sources.
            // Input forwarding still binds the caller's actual held claim.
            if matches!(source, CheckedClaimOutcomeSource::Established { .. })
                && let CheckedClaimOutcomeSource::Established {
                    claim_identity: expanded_identity,
                    ..
                } = &mut entry.source
            {
                *expanded_identity = claim_identity;
            }
            if matches!(entry.source, CheckedClaimOutcomeSource::Established { provenance:
                PermissionProvenance::Joined { machine_symbol: rebound_machine, state_symbol: rebound_state,
                    source: PermissionEventSource::Call { statement_index: rebound_statement, call_ordinal: rebound_ordinal, .. }, .. }, .. }
                if (rebound_machine, rebound_state, rebound_statement, rebound_ordinal) == (machine, owner, statement, ordinal))
            {
                return None;
            }
            entries.push(entry);
        }
        next.entries = entries;
        if !complete_frontier(
            program,
            state.return_type,
            &next.entries,
            &next.inactive_paths,
        ) {
            return None;
        }
        expanded.extend(expand_joined_frontier(
            program, borrow, state, ownership, events, maps, active, &visited, next,
        )?);
    }
    Some(expanded)
}

fn complete_frontier(
    program: &dyn ::validation::CheckingInput,
    result_type: typed_trees::types::TypeReferenceHandle,
    entries: &[CheckedClaimOutcomeEntry],
    inactive_paths: &[Vec<PlaceSegment>],
) -> bool {
    let expected = linear_claim_frontier(program, result_type);
    if expected.is_empty()
        || entries.iter().any(|entry| {
            !known_source(&entry.source)
                || !expected.iter().any(|claim| claim.path == entry.output_path)
                || inactive_paths.contains(&entry.output_path)
        })
        || inactive_paths
            .iter()
            .any(|path| !expected.iter().any(|claim| claim.path == *path))
    {
        return false;
    }
    for claim in expected {
        let matching = entries
            .iter()
            .filter(|entry| entry.output_path == claim.path)
            .count();
        if matching != 1 && !(matching == 0 && inactive_paths.contains(&claim.path)) {
            return false;
        }
    }
    !entries.iter().enumerate().any(|(index, entry)| {
        entries[index + 1..].iter().any(|other| {
            entry.source == other.source
                && !claim_paths_are_case_alternatives(&entry.output_path, &other.output_path)
        })
    })
}

/// Reuse source-replayed absence rather than treating a failed binding as an
/// empty case. A historical establishment alone is insufficient: assignments
/// and call write frames must preserve that exact place through this call's
/// entire statement, including earlier argument evaluation.
#[allow(clippy::too_many_arguments)]
fn excluded_input_at_call(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    state: &typed_trees::state::State,
    statement_index: usize,
    call: &typed_trees::expression::TableCallExpression,
    target: &typed_trees::state::State,
    source: &CheckedClaimOutcomeSource,
    ownership: &FlowOwnershipFacts,
    events: &[FlowPermissionEventFact],
) -> Option<ExcludedInput> {
    let CheckedClaimOutcomeSource::Input {
        parameter_symbol,
        path,
    } = source
    else {
        return None;
    };
    let argument = argument_for_parameter(
        program,
        program.expression_table.expression_handles(call.arguments),
        target,
        *parameter_symbol,
    )?;
    let parameter = parameter_for_symbol(program, target, *parameter_symbol)?;
    let mut projections = crate::flow::literal_expression_projections(
        program,
        argument,
        parameter.type_reference,
        path,
        false,
    )?;
    let projection = projections.pop()?;
    if !projections.is_empty() {
        return None;
    }
    let mut place = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        projection.expression,
    )?;
    place.segments.extend_from_slice(&projection.remaining);
    let established_at = preserved_inactive_place(
        program,
        borrow,
        state,
        statement_index,
        &place,
        ownership,
        events,
    )?;
    Some(ExcludedInput {
        source: source.clone(),
        root: place.root,
        path: place.segments,
        established_at,
    })
}

/// An absent conditional payload remains absent only while the exact storage
/// path is preserved through the entire consuming statement. Unknown identity
/// alone, and an inactive unguarded allocation, establish no such fact.
#[allow(clippy::too_many_arguments)]
pub(super) fn preserved_inactive_place(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    state: &typed_trees::state::State,
    statement_index: usize,
    place: &validation::CanonicalPlace,
    ownership: &FlowOwnershipFacts,
    events: &[FlowPermissionEventFact],
) -> Option<usize> {
    if !place
        .segments
        .iter()
        .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
    {
        return None;
    }
    let establishment = events.iter().filter(|event| {
        event.state_symbol == state.symbol && event.kind == PermissionEventKind::Establish
            && event.access == PermissionAccess::Owned && !event.obligation_live
            && event.claim_identity == PermissionClaimIdentity::Unknown
            && event.root == place.root
            && ownership.segments.span_or_empty(event.segments) == place.segments
            && matches!(event.source, PermissionEventSource::Statement { statement_index: index }
                if index < statement_index)
    }).max_by_key(|event| match event.source {
        PermissionEventSource::Statement { statement_index } => statement_index,
        _ => 0,
    })?;
    let PermissionEventSource::Statement {
        statement_index: established_at,
    } = establishment.source
    else {
        return None;
    };
    let machine = program.machine_by_symbol(establishment.machine_symbol)?;
    let frames = validation::CallFrameResolver::new(program)?;
    let borrowed_state = borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|candidate| {
            candidate.machine_symbol == machine.symbol && candidate.state_symbol == state.symbol
        })?;
    let statements = program.statement_table.statements(state.statement_nodes);
    for (index, statement) in statements
        .iter()
        .enumerate()
        .take(statement_index + 1)
        .skip(established_at + 1)
    {
        let mut writes = crate::flow::statement_storage_writes(
            program,
            machine.symbol,
            state.symbol,
            index,
            statement,
            Some(&frames),
        )?;
        writes.extend(crate::flow::frame_storage_writes(
            program,
            machine.symbol,
            state.symbol,
            index,
            &frames.statement_value_write_frame(machine, statement),
            Some(&frames),
        )?);
        if let Some(call) = statement.as_call() {
            writes.extend(crate::flow::frame_storage_writes(
                program,
                machine.symbol,
                state.symbol,
                index,
                &frames.may_write_frame(machine, call),
                Some(&frames),
            )?);
        }
        // Selected operators can have incomplete body frames. The independently
        // checked signature ceiling also covers every call in this statement.
        for call in borrow
            .calls
            .span_or_empty(borrowed_state.calls)
            .iter()
            .filter(|call| call.statement_index == index)
        {
            writes.extend(crate::flow::signature_ceiling_places(
                program,
                machine.symbol,
                state.symbol,
                None,
                call,
                Some(&frames),
            )?);
        }
        if writes.iter().any(|write| {
            (write.root) == (place.root)
                && crate::flow::canonical_place_segments_may_overlap(
                    program,
                    &write.segments,
                    &place.segments,
                )
        }) {
            return None;
        }
    }
    Some(established_at)
}

fn derive_receipts(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    ownership: &FlowOwnershipFacts,
    events: &[FlowPermissionEventFact],
) -> Vec<Receipt> {
    let maps = derive_checked_claim_outcome_maps(program, borrow, ownership, events);
    let mut receipts = Vec::new();
    for event in events {
        if event.kind != PermissionEventKind::Establish
            || event.access != PermissionAccess::Owned
            || !event.obligation_live
        {
            continue;
        }
        let Some((statement_index, expression, received_path)) =
            super::receiving_value(program, &ownership.segments, event)
        else {
            continue;
        };
        let Some(state) = program.state_by_symbol(event.state_symbol) else {
            continue;
        };
        let Some(call) = program.expression_table.call_node(expression) else {
            continue;
        };
        let Some(target) = validation::call_target_state(program, call.target_symbol) else {
            continue;
        };
        let mut ordinal = 0;
        let exact_call = loop {
            let Some(site) = validation::find_call_site(
                program,
                event.machine_symbol,
                event.state_symbol,
                statement_index,
                ordinal,
            ) else {
                break false;
            };
            if matches!(site, validation::CallSite::Expression { expression: found, .. } if found == expression)
            {
                break true;
            }
            ordinal += 1;
        };
        if !exact_call {
            continue;
        }
        let Some(frontiers) = state_frontiers(
            program,
            borrow,
            target,
            ownership,
            events,
            &maps,
            &mut Vec::new(),
        ) else {
            continue;
        };
        let path = ownership.segments.span_or_empty(event.segments);
        let mut alternatives = Vec::new();
        let mut complete = true;
        for frontier in frontiers {
            let mut entries = frontier
                .entries
                .iter()
                .filter(|entry| entry.output_path == received_path);
            let Some(entry) = entries.next() else {
                if frontier
                    .inactive_paths
                    .iter()
                    .any(|inactive| inactive == received_path)
                {
                    alternatives.push(Alternative {
                        exits: frontier.exits,
                        source: None,
                        excluded_input: None,
                    });
                    continue;
                }
                complete = false;
                break;
            };
            if entries.next().is_some() {
                complete = false;
                break;
            }
            let bound_source = bind_claim_outcome_source_at_call(
                program,
                state,
                statement_index,
                call,
                target,
                &entry.source,
                &ownership.segments,
                events,
            )
            .filter(known_source);
            let excluded_input = if bound_source.is_none() {
                excluded_input_at_call(
                    program,
                    borrow,
                    state,
                    statement_index,
                    call,
                    target,
                    &entry.source,
                    ownership,
                    events,
                )
            } else {
                None
            };
            if bound_source.is_none() && excluded_input.is_none() {
                complete = false;
                break;
            }
            alternatives.push(Alternative {
                exits: frontier.exits,
                source: bound_source.map(|bound| (entry.source.clone(), bound)),
                excluded_input,
            });
        }
        if !complete || alternatives.is_empty() {
            continue;
        }
        let first = &alternatives[0];
        let shared_source = alternatives.iter().all(|alternative| {
            alternative.source.as_ref().map(|(source, _)| source)
                == first.source.as_ref().map(|(source, _)| source)
                && alternative.excluded_input == first.excluded_input
        });
        // An invariant input source forwards the existing caller claim.
        // A callee-local source instead needs an occurrence-local binding,
        // even when every exit agrees. Active versus excluded custody also
        // keeps its complete frontier, not just the one active source.
        let callee_establishment = first.source.as_ref().is_some_and(|(source, _)| {
            matches!(source, CheckedClaimOutcomeSource::Established { .. })
        });
        // Authenticated absence is a disposition, never an active claim an
        // ordinary origin rewrite could forward, even on a uniform exit set.
        let carries_absence = alternatives
            .iter()
            .any(|alternative| alternative.source.is_none());
        if shared_source && !callee_establishment && !carries_absence {
            continue;
        }
        // Only the result occurrence allocated at this call can name its join.
        // Rewritten identities from another call cannot borrow this receipt.
        let PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source,
            ..
        } = event.claim_identity
        else {
            continue;
        };
        if machine_symbol != event.machine_symbol
            || state_symbol != state.symbol
            || source != event.source
        {
            continue;
        }
        receipts.push(Receipt {
            machine: event.machine_symbol,
            state: state.symbol,
            statement: statement_index,
            ordinal,
            target: call.target_symbol,
            expression,
            root: event.root,
            path: path.to_vec(),
            identity: event.claim_identity,
            alternatives,
        });
    }
    receipts
}

fn join_provenance(receipt: &Receipt) -> Option<PermissionProvenance> {
    let PermissionClaimIdentity::Established { ordinal, .. } = receipt.identity else {
        return None;
    };
    Some(PermissionProvenance::Joined {
        machine_symbol: receipt.machine,
        state_symbol: receipt.state,
        source: PermissionEventSource::Call {
            statement_index: receipt.statement,
            call_ordinal: receipt.ordinal,
            target_symbol: receipt.target,
        },
        ordinal,
    })
}

fn store_source(
    ownership: &mut FlowOwnershipFacts,
    source: CheckedClaimOutcomeSource,
) -> FlowClaimOutcomeSource {
    match source {
        CheckedClaimOutcomeSource::Input {
            parameter_symbol,
            path,
        } => FlowClaimOutcomeSource::Input {
            parameter_symbol,
            segments: ownership.segments.insert_many(path),
        },
        CheckedClaimOutcomeSource::Established {
            claim_identity,
            provenance,
        } => FlowClaimOutcomeSource::Established {
            claim_identity,
            provenance,
        },
    }
}

pub(crate) fn publish_call_claim_joins(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
) {
    let ownership = &mut facts.flow.ownership;
    let mut events = ownership
        .permissions
        .iter()
        .map(|(_, event)| event.clone())
        .collect::<Vec<_>>();
    // Earlier call joins can feed later calls, but no receipt cites itself.
    for _ in 0..=events.len() {
        let receipts = derive_receipts(program, &facts.borrow, ownership, &events);
        let mut changed = false;
        for receipt in &receipts {
            let Some(provenance) = join_provenance(receipt) else {
                continue;
            };
            for event in &mut events {
                if event.claim_identity == receipt.identity && event.provenance != provenance {
                    event.provenance = provenance;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let receipts = derive_receipts(program, &facts.borrow, ownership, &events);
    ownership.permissions = arena::Arena::default();
    ownership.permissions.insert_many(events);
    ownership.claim_join_receipts = arena::Arena::default();
    ownership.claim_join_alternatives = arena::Arena::default();
    ownership.claim_join_exits = arena::Arena::default();
    for receipt in receipts {
        let mut alternatives = Vec::new();
        for alternative in receipt.alternatives {
            let exits = ownership.claim_join_exits.insert_many(alternative.exits);
            let source = match (alternative.source, alternative.excluded_input) {
                (None, None) => FlowClaimJoinAlternativeSource::Inactive,
                (_, Some(excluded)) => FlowClaimJoinAlternativeSource::ExcludedInput {
                    source: store_source(ownership, excluded.source),
                    root: excluded.root,
                    segments: ownership.segments.insert_many(excluded.path),
                    established_at: excluded.established_at,
                },
                (Some((source, bound_source)), None) => FlowClaimJoinAlternativeSource::Claim {
                    source: store_source(ownership, source),
                    bound_source: store_source(ownership, bound_source),
                },
            };
            alternatives.push(FlowClaimJoinAlternative { exits, source });
        }
        let alternatives = ownership.claim_join_alternatives.insert_many(alternatives);
        let result_segments = ownership.segments.insert_many(receipt.path);
        ownership.claim_join_receipts.insert(FlowClaimJoinReceipt {
            machine_symbol: receipt.machine,
            state_symbol: receipt.state,
            statement_index: receipt.statement,
            call_ordinal: receipt.ordinal,
            target_symbol: receipt.target,
            expression: receipt.expression,
            result_root: receipt.root,
            result_segments,
            claim_identity: receipt.identity,
            alternatives,
        });
    }
}

fn read_source(
    ownership: &FlowOwnershipFacts,
    source: FlowClaimOutcomeSource,
) -> Option<CheckedClaimOutcomeSource> {
    Some(match source {
        FlowClaimOutcomeSource::Unknown => return None,
        FlowClaimOutcomeSource::Input {
            parameter_symbol,
            segments,
        } => CheckedClaimOutcomeSource::Input {
            parameter_symbol,
            path: ownership.segments.span(segments)?.to_vec(),
        },
        FlowClaimOutcomeSource::Established {
            claim_identity,
            provenance,
        } => CheckedClaimOutcomeSource::Established {
            claim_identity,
            provenance,
        },
    })
}

pub(crate) fn validate_call_claim_joins(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    ownership: &FlowOwnershipFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let events = ownership
        .permissions
        .iter()
        .map(|(_, event)| event.clone())
        .collect::<Vec<_>>();
    let expected = derive_receipts(program, borrow, ownership, &events);
    let recorded = ownership
        .claim_join_receipts
        .iter()
        .map(|(_, receipt)| {
            let alternatives = ownership
                .claim_join_alternatives
                .span(receipt.alternatives)?
                .iter()
                .map(|alternative| {
                    Some(Alternative {
                        exits: ownership.claim_join_exits.span(alternative.exits)?.to_vec(),
                        source: match alternative.source {
                            FlowClaimJoinAlternativeSource::Inactive
                            | FlowClaimJoinAlternativeSource::ExcludedInput { .. } => None,
                            FlowClaimJoinAlternativeSource::Claim {
                                source,
                                bound_source,
                            } => Some((
                                read_source(ownership, source)?,
                                read_source(ownership, bound_source)?,
                            )),
                        },
                        excluded_input: match alternative.source {
                            FlowClaimJoinAlternativeSource::ExcludedInput {
                                source,
                                root,
                                segments,
                                established_at,
                            } => Some(ExcludedInput {
                                source: read_source(ownership, source)?,
                                root,
                                path: ownership.segments.span(segments)?.to_vec(),
                                established_at,
                            }),
                            _ => None,
                        },
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Receipt {
                machine: receipt.machine_symbol,
                state: receipt.state_symbol,
                statement: receipt.statement_index,
                ordinal: receipt.call_ordinal,
                target: receipt.target_symbol,
                expression: receipt.expression,
                root: receipt.result_root,
                path: ownership.segments.span(receipt.result_segments)?.to_vec(),
                identity: receipt.claim_identity,
                alternatives,
            })
        })
        .collect::<Option<Vec<_>>>();
    let valid = recorded.as_ref() == Some(&expected)
        && events.iter().all(|event| {
            if matches!(event.provenance, PermissionProvenance::Joined { .. }) {
                expected.iter().any(|receipt| {
                    receipt.identity == event.claim_identity
                        && join_provenance(receipt) == Some(event.provenance)
                })
            } else {
                !expected
                    .iter()
                    .any(|receipt| receipt.identity == event.claim_identity)
            }
        });
    if !valid {
        diagnostics.push(Diagnostic::error(
            "call claim joins differ from exact source exit and call replay",
        ));
    }
}
