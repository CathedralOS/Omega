//! Translate a proof-used producer dependency to its emitted BoundaryCall.
//! This transports the request; the chosen provider independently proves it.

use checked_trees::{CheckedTrees, FlowReferenceForwardingSource};
use lowered_psi::LoweredPsi;
use terminal_psi::{BoundaryReferenceForwardingDemand, BoundaryReferenceSource, OperationKind};

use crate::expression_preparation::bindings::structural_paths::lower_typed_argument_path;
use crate::shared::lowering_error::{LoweringError, unsupported};

pub(super) fn retain(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    lowered: &mut LoweredPsi,
) -> Result<(), LoweringError> {
    if !lowered
        .semantic_module
        .boundary_reference_forwarding
        .is_empty()
    {
        return unsupported("reference forwarding requests were already lowered");
    }
    let control = &checked.facts.flow.control;
    let mut requested = Vec::new();
    for (_, state) in control.states.iter() {
        for call in control.calls.span_or_empty(state.calls) {
            for request in &call.reference_forwarding {
                if !requested.contains(request) {
                    requested.push(request.clone());
                }
            }
        }
    }
    let mut rows = Vec::new();
    let mut visited = vec![false; requested.len()];
    for occurrence in &lowered.source_call_occurrences {
        let (Some(coordinate), Some(operation_id)) = (
            occurrence.origin.statement(),
            occurrence.terminal_site.operation(),
        ) else {
            continue;
        };
        for (index, request) in requested.iter().enumerate() {
            if coordinate.call_ordinal != request.call_ordinal {
                continue;
            }
            let mut states = control
                .states
                .iter()
                .map(|(_, state)| state)
                .filter(|state| state.state_symbol == request.state_symbol);
            let state = states.next().ok_or(LoweringError::Unsupported(
                "the forwarding producer state is absent",
            ))?;
            if states.next().is_some() {
                return unsupported("the forwarding producer state is ambiguous");
            }
            if !occurrence.matches_checked_origin(
                checked,
                checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol: state.machine_symbol,
                    state_symbol: state.state_symbol,
                    statement_index: request.statement_index,
                    role: checked_trees::CheckedValueStatementRole::Expression,
                },
            ) {
                continue;
            }
            let mut calls = control
                .calls
                .span_or_empty(state.calls)
                .iter()
                .filter(|call| {
                    call.statement_index == request.statement_index
                        && call.call_ordinal == request.call_ordinal
                        && call.authored_expression == request.invocation
                });
            let call = calls.next().ok_or(LoweringError::Unsupported(
                "the forwarding producer has no exact recorded call",
            ))?;
            if calls.next().is_some() {
                return unsupported("the forwarding producer has ambiguous recorded calls");
            }
            if occurrence.source_target != call.target_symbol
                && occurrence.source_target != call.authored_target
            {
                return unsupported("the forwarding request joins another source target");
            }
            let mut operations = lowered
                .semantic_module
                .machines
                .iter()
                .flat_map(|machine| {
                    machine
                        .operations()
                        .map(move |operation| (machine.id, operation))
                })
                .filter(|(_, operation)| operation.id == operation_id);
            let (Some((machine, operation)), None) = (operations.next(), operations.next()) else {
                return unsupported("the forwarding producer has no unique emitted operation");
            };
            let OperationKind::BoundaryCall { boundary, .. } = operation.kind else {
                return unsupported("the forwarding producer did not lower to a boundary call");
            };
            let declaration = lowered.semantic_module.boundary_machine(boundary).ok_or(
                LoweringError::Unsupported("the forwarding boundary is absent"),
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
                    return unsupported("the forwarding formal has no unique packed lane");
                };
                Ok((
                    u32::try_from(index).map_err(|_| {
                        LoweringError::Unsupported("the forwarding lane exceeds u32")
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
                return unsupported("the forwarding request has no exact public reference output");
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
                return unsupported("the forwarding request has no reference value choices");
            }
            rows.push(BoundaryReferenceForwardingDemand {
                machine,
                operation: operation_id,
                output: u32::try_from(output).map_err(|_| {
                    LoweringError::Unsupported("the forwarding output ordinal exceeds u32")
                })?,
                sources,
            });
            visited[index] = true;
        }
    }
    for (request, visited) in requested.iter().zip(visited) {
        let state = control
            .states
            .iter()
            .map(|(_, state)| state)
            .find(|state| state.state_symbol == request.state_symbol)
            .ok_or(LoweringError::Unsupported(
                "the forwarding source state is absent",
            ))?;
        let origin =
            checked.consistent_terminal_origin(checked_trees::CheckedValueOrigin::StateStatement {
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                statement_index: request.statement_index,
                role: checked_trees::CheckedValueStatementRole::Expression,
            });
        let emitted = match origin {
            Some(checked_trees::CheckedValueOrigin::StateStatement { state_symbol, .. }) => lowered
                .state_blocks
                .iter()
                .any(|block| block.state == state_symbol),
            _ => false,
        };
        if emitted && !visited {
            return unsupported("an emitted forwarding dependency has no producer call occurrence");
        }
    }
    rows.sort();
    rows.dedup();
    if rows.windows(2).any(|pair| {
        (pair[0].machine, pair[0].operation, pair[0].output)
            == (pair[1].machine, pair[1].operation, pair[1].output)
    }) {
        return unsupported("one forwarding output has inconsistent requested choice sets");
    }
    lowered.semantic_module.boundary_reference_forwarding = rows;
    Ok(())
}
