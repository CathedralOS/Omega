//! Keep a saved reference's requirements attached to the call that produced it.
//! Lowering records the request; the selected implementation must prove it.

use checked_trees::{CheckedTrees, FlowReferenceForwardingSource};
use lowered_psi::LoweredPsi;
use terminal_psi::{BoundaryReferenceForwardingDemand, BoundaryReferenceSource};

use super::call_requests::{
    EmittedCallCatalog, EmittedCallLookupError, StatementCallKey, matching_requests,
};
use crate::expression_preparation::bindings::structural_paths::lower_typed_argument_path;
use crate::shared::lowering_error::LoweringError;

pub(super) fn retain(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    lowered: &mut LoweredPsi,
    emitted_calls: &EmittedCallCatalog,
) -> Result<(), LoweringError> {
    if !lowered
        .semantic_module
        .boundary_reference_forwarding
        .is_empty()
    {
        return invalid_plan("reference forwarding requests were already lowered");
    }
    let control = &checked.facts.flow.control;
    let mut requested = Vec::new();
    let mut seen_requests = std::collections::HashSet::new();
    for (_, state) in control.states.iter() {
        for call in control.calls.span_or_empty(state.calls) {
            for request in &call.reference_forwarding {
                if seen_requests.insert(request) {
                    requested.push(request);
                }
            }
        }
    }
    if requested.is_empty() {
        return Ok(());
    }
    let mut states = control
        .states
        .iter()
        .map(|(_, state)| state)
        .collect::<Vec<_>>();
    states.sort_by_key(|state| {
        (
            state.state_symbol.arena_index(),
            state.state_symbol.generation(),
        )
    });
    let mut producer_states = Vec::with_capacity(requested.len());
    let mut requested_calls = Vec::new();
    let mut unavailable_states = Vec::new();
    for (request_index, request) in requested.iter().enumerate() {
        let state_key = (
            request.state_symbol.arena_index(),
            request.state_symbol.generation(),
        );
        let first = states.partition_point(|state| {
            (
                state.state_symbol.arena_index(),
                state.state_symbol.generation(),
            ) < state_key
        });
        let after = states.partition_point(|state| {
            (
                state.state_symbol.arena_index(),
                state.state_symbol.generation(),
            ) <= state_key
        });
        let state = states.get(first).copied().filter(|_| first < after);
        let ambiguous = after - first > 1;
        producer_states.push((state, ambiguous));
        if let Some(state) = state.filter(|_| !ambiguous) {
            if let Some(key) = StatementCallKey::from_checked(
                checked,
                checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol: state.machine_symbol,
                    state_symbol: state.state_symbol,
                    statement_index: request.statement_index,
                    role: checked_trees::CheckedValueStatementRole::Expression,
                },
                request.call_ordinal,
            ) {
                requested_calls.push((key, request_index));
            }
        } else {
            // Keep malformed-state checks at the first same-ordinal occurrence,
            // before testing its source coordinate, just as the original walk did.
            unavailable_states.push((request.call_ordinal, request_index));
        }
    }
    requested_calls.sort_unstable();
    unavailable_states.sort_unstable();
    let mut recorded_calls = control
        .states
        .iter()
        .flat_map(|(_, state)| {
            control
                .calls
                .span_or_empty(state.calls)
                .iter()
                .map(move |call| {
                    (
                        (
                            StatementCallKey::new(
                                state.state_symbol,
                                call.statement_index,
                                call.call_ordinal,
                            ),
                            call.authored_expression.arena_index(),
                            call.authored_expression.generation(),
                        ),
                        call,
                    )
                })
        })
        .collect::<Vec<_>>();
    recorded_calls.sort_unstable_by_key(|(key, _)| *key);
    let mut rows = Vec::new();
    let mut visited = vec![false; requested.len()];
    for occurrence in &lowered.source_call_occurrences {
        let (Some(coordinate), Some(operation_id)) = (
            occurrence.origin.statement(),
            occurrence.terminal_site.operation(),
        ) else {
            continue;
        };
        let key = StatementCallKey::new(
            coordinate.state,
            coordinate.statement_index,
            coordinate.call_ordinal,
        );
        let mut matching = matching_requests(&requested_calls, key)
            .iter()
            .map(|(_, request_index)| *request_index)
            .peekable();
        let mut unavailable = matching_requests(&unavailable_states, coordinate.call_ordinal)
            .iter()
            .map(|(_, request_index)| *request_index)
            .peekable();
        // Merge the two already ordered groups without changing which original
        // request reports an error first or allocating per emitted occurrence.
        let matching = std::iter::from_fn(|| match (matching.peek(), unavailable.peek()) {
            (Some(found), Some(missing)) if found < missing => matching.next(),
            (_, Some(_)) => unavailable.next(),
            _ => matching.next(),
        });
        for request_index in matching {
            let request = requested[request_index];
            let (state, ambiguous) = producer_states[request_index];
            let state = state.ok_or(LoweringError::InvalidReferenceForwardingPlan(
                "the forwarding producer state is absent",
            ))?;
            if ambiguous {
                return invalid_plan("the forwarding producer state is ambiguous");
            }
            let call_key = (
                StatementCallKey::new(
                    state.state_symbol,
                    request.statement_index,
                    request.call_ordinal,
                ),
                request.invocation.arena_index(),
                request.invocation.generation(),
            );
            let call_position = recorded_calls.partition_point(|(found, _)| *found < call_key);
            let call = recorded_calls
                .get(call_position)
                .filter(|(found, _)| *found == call_key)
                .map(|(_, call)| *call)
                .ok_or(LoweringError::InvalidReferenceForwardingPlan(
                    "the forwarding producer has no exact recorded call",
                ))?;
            if recorded_calls
                .get(call_position + 1)
                .is_some_and(|(found, _)| *found == call_key)
            {
                return invalid_plan("the forwarding producer has ambiguous recorded calls");
            }
            if occurrence.source_target != call.target_symbol
                && occurrence.source_target != call.authored_target
            {
                return invalid_plan("the forwarding request joins another source target");
            }
            let (machine, boundary) = match emitted_calls.boundary_call(operation_id) {
                Ok(call) => call,
                Err(EmittedCallLookupError::MissingOrAmbiguousOperation) => {
                    return invalid_plan("the forwarding producer has no unique emitted operation");
                }
                Err(EmittedCallLookupError::NotBoundaryCall) => {
                    return invalid_plan(
                        "the forwarding producer did not lower to a boundary call",
                    );
                }
            };
            let declaration = lowered.semantic_module.boundary_machine(boundary).ok_or(
                LoweringError::InvalidReferenceForwardingPlan("the forwarding boundary is absent"),
            )?;
            let packed = |position| -> Result<
                (u32, &terminal_psi::StructuralParameterDeclaration),
                LoweringError,
            > {
                let mut rows = declaration
                    .structural_parameters
                    .iter()
                    .enumerate()
                    .filter(|(_, parameter)| parameter.position == position);
                let (Some((index, parameter)), None) = (rows.next(), rows.next()) else {
                    return invalid_plan("the forwarding formal has no unique packed lane");
                };
                Ok((
                    u32::try_from(index).map_err(|_| {
                        LoweringError::InvalidReferenceForwardingPlan(
                            "the forwarding lane exceeds u32",
                        )
                    })?,
                    parameter,
                ))
            };
            let (output_parameter, formal) = packed(request.output_parameter)?;
            let path = lower_typed_argument_path(
                identities,
                &request.output_path,
                formal.structural_type,
                &lowered.semantic_module.structural_types,
                None,
                &mut std::iter::empty(),
                None,
            )?;
            let mut outputs =
                declaration
                    .reference_outputs
                    .iter()
                    .enumerate()
                    .filter(|(_, output)| {
                        matches!(output.destination,
                    terminal_psi::BoundaryReferenceDestination::Parameter(argument_index)
                        if argument_index == output_parameter)
                            && output.path == path
                    });
            let (Some((output, _)), None) = (outputs.next(), outputs.next()) else {
                return invalid_plan("the forwarding request has no exact public reference output");
            };
            let mut sources = Vec::new();
            for source in &request.sources {
                sources.push(match source {
                    FlowReferenceForwardingSource::Reference { argument_position } => {
                        let (argument_index, _) = packed(*argument_position)?;
                        BoundaryReferenceSource::Reference { argument_index }
                    }
                    FlowReferenceForwardingSource::Stored {
                        argument_position,
                        path,
                    } => {
                        let (argument_index, formal) = packed(*argument_position)?;
                        BoundaryReferenceSource::Stored {
                            argument_index,
                            path: lower_typed_argument_path(
                                identities,
                                path,
                                formal.structural_type,
                                &lowered.semantic_module.structural_types,
                                None,
                                &mut std::iter::empty(),
                                None,
                            )?,
                        }
                    }
                });
            }
            sources.sort();
            sources.dedup();
            if sources.is_empty() {
                return invalid_plan("the forwarding request has no reference value choices");
            }
            rows.push(BoundaryReferenceForwardingDemand {
                machine,
                operation: operation_id,
                output: u32::try_from(output).map_err(|_| {
                    LoweringError::InvalidReferenceForwardingPlan(
                        "the forwarding output ordinal exceeds u32",
                    )
                })?,
                sources,
            });
            visited[request_index] = true;
        }
    }
    for ((request, (state, _)), visited) in requested.iter().zip(producer_states).zip(visited) {
        let state = state.ok_or(LoweringError::InvalidReferenceForwardingPlan(
            "the forwarding source state is absent",
        ))?;
        let key = StatementCallKey::from_checked(
            checked,
            checked_trees::CheckedValueOrigin::StateStatement {
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                statement_index: request.statement_index,
                role: checked_trees::CheckedValueStatementRole::Expression,
            },
            request.call_ordinal,
        );
        if key.is_some_and(|key| emitted_calls.state_was_emitted(key)) && !visited {
            return invalid_plan(
                "an emitted forwarding dependency has no producer call occurrence",
            );
        }
    }
    rows.sort();
    rows.dedup();
    if rows.windows(2).any(|pair| {
        (pair[0].machine, pair[0].operation, pair[0].output)
            == (pair[1].machine, pair[1].operation, pair[1].output)
    }) {
        return invalid_plan("one forwarding output has inconsistent requested choice sets");
    }
    lowered.semantic_module.boundary_reference_forwarding = rows;
    Ok(())
}

fn invalid_plan<T>(reason: &'static str) -> Result<T, LoweringError> {
    Err(LoweringError::InvalidReferenceForwardingPlan(reason))
}
