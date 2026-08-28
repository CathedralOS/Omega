use omega_abstract_operations::{
    AbstractBoundaryEdge, AbstractBoundaryLink, AbstractBoundarySummary,
    AbstractHostCallNativeArgument, AbstractHostCallOccurrence, AbstractHostCallSourceSite,
    AbstractSourceBoundaryEdge,
};
use omega_control_flow::ControlFlowPlan;
use omega_platform_interface::HostCallPlan;

pub(super) fn build_abstract_boundary_summary(
    control_flow: &ControlFlowPlan,
    host_calls: &HostCallPlan,
) -> Result<AbstractBoundarySummary, psi_diagnostics::Diagnostic> {
    let mut summary = AbstractBoundarySummary::with_source_and_host_capacity(
        control_flow.semantics.boundaries.edges.len(),
        host_calls.operations.len(),
    );

    for (_, state) in control_flow.states.iter() {
        for edge in control_flow
            .semantics
            .boundaries
            .edges
            .span_or_empty(state.boundaries.edges)
        {
            summary.source_edges.insert(AbstractSourceBoundaryEdge {
                source_key: state.key,
                statement_index: edge.statement_index,
                call_ordinal: edge.call_ordinal,
                receiver_symbol: edge.receiver_symbol,
                target_symbol: edge.target_symbol,
                boundary_trait_symbol: edge.boundary_trait_symbol,
                boundary_signature_symbol: edge.boundary_signature_symbol,
            });
        }
    }

    summary.host_calls.reserve(host_calls.calls.len());
    summary
        .host_call_arguments
        .reserve(host_calls.arguments.len());

    for (source_call, call) in host_calls.calls.iter() {
        let source_site = match call.source_site.ok_or_else(|| {
            psi_diagnostics::Diagnostic::error("host call is missing its exact source use site")
        })? {
            psi_checked_trees::NominalMachineUseSite::Statement(handle) => {
                AbstractHostCallSourceSite::Statement(handle)
            }
            psi_checked_trees::NominalMachineUseSite::Expression(handle) => {
                AbstractHostCallSourceSite::Expression(handle)
            }
        };
        if !call.registration_operation.is_valid() || call.requirement_identity.is_empty() {
            return Err(psi_diagnostics::Diagnostic::error(
                "host call is missing its exact registrar overload identity",
            ));
        }
        if !call.lowering.is_valid() {
            return Err(psi_diagnostics::Diagnostic::error(
                "host call is missing its exact platform lowering identity",
            ));
        }
        let mut arguments = psi_arena::HandleSpan::empty();
        let source_arguments = host_calls.arguments.span(call.arguments).ok_or_else(|| {
            psi_diagnostics::Diagnostic::error("host call retained an invalid argument span")
        })?;
        if call.has_result
            && source_arguments
                .first()
                .is_none_or(|argument| argument.formal.is_some())
        {
            return Err(psi_diagnostics::Diagnostic::error(
                "result-bearing host call is missing its non-formal result operand",
            ));
        }
        for (argument_index, argument) in source_arguments.iter().enumerate() {
            let expected_formal_index = argument_index.saturating_sub(usize::from(call.has_result));
            let Some(formal) = argument.formal else {
                if call.has_result && argument_index == 0 {
                    continue;
                }
                return Err(psi_diagnostics::Diagnostic::error(
                    "host call argument is missing its exact native formal identity",
                ));
            };
            let expected_ordinal = u32::try_from(expected_formal_index).map_err(|_| {
                psi_diagnostics::Diagnostic::error("host-call formal ordinal exceeds u32")
            })?;
            if formal.formal_ordinal != expected_ordinal
                || formal.native_parameter
                    != omega_calling_conventions::callback_native_parameter_id(
                        &call.requirement_identity,
                        expected_ordinal,
                    )
            {
                return Err(psi_diagnostics::Diagnostic::error(
                    "host call argument native identity does not match its exact overload and order",
                ));
            }
            summary.host_call_arguments.append_to_span(
                &mut arguments,
                AbstractHostCallNativeArgument {
                    formal_ordinal: formal.formal_ordinal,
                    native_parameter: Some(formal.native_parameter),
                },
            );
        }
        let host_call = summary.host_calls.insert(AbstractHostCallOccurrence {
            source_call_index: source_call.arena_index(),
            source_call_generation: source_call.generation(),
            source_site,
            registration_operation: call.registration_operation,
            requirement_identity: call.requirement_identity.clone(),
            source_key: call.source_key,
            statement_index: call.statement_index,
            call_ordinal: call.call_ordinal,
            lowering_index: call.lowering.arena_index(),
            lowering_generation: call.lowering.generation(),
            arguments,
        });
        for (operation_ordinal, operation) in host_calls
            .operations
            .span_or_empty(call.operations)
            .iter()
            .enumerate()
        {
            let lowered_edge = summary.edges.insert(AbstractBoundaryEdge {
                host_call,
                source_key: call.source_key,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                operation_ordinal,
                operation_key: operation.operation_key,
            });
            append_boundary_links(
                &mut summary,
                call.source_key,
                call.statement_index,
                call.call_ordinal,
                call.registration_operation,
                lowered_edge,
            );
        }
    }

    validate_abstract_boundary_summary(host_calls, &summary)?;
    Ok(summary)
}

fn validate_abstract_boundary_summary(
    host_calls: &HostCallPlan,
    summary: &AbstractBoundarySummary,
) -> Result<(), psi_diagnostics::Diagnostic> {
    if summary.host_calls.len() != host_calls.calls.len() {
        return Err(boundary_identity_error(
            "host-call occurrence cardinality drift",
        ));
    }

    let expected_edge_count = host_calls
        .calls
        .iter()
        .map(|(_, call)| usize::try_from(call.operations.count()).unwrap_or(usize::MAX))
        .try_fold(0usize, usize::checked_add)
        .ok_or_else(|| boundary_identity_error("host-call operation cardinality overflow"))?;
    if summary.edges.len() != expected_edge_count {
        return Err(boundary_identity_error(
            "host-call operation cardinality drift",
        ));
    }

    let expected_argument_count = host_calls
        .arguments
        .iter()
        .filter(|(_, argument)| argument.formal.is_some())
        .count();
    if summary.host_call_arguments.len() != expected_argument_count {
        return Err(boundary_identity_error(
            "host-call native argument catalog cardinality drift",
        ));
    }

    let expected_link_count = summary
        .edges
        .iter()
        .map(|(_, edge)| {
            let occurrence = summary.host_calls.get(edge.host_call);
            summary
                .source_edges
                .iter()
                .filter(|(_, source)| {
                    source.source_key == edge.source_key
                        && source.statement_index == edge.statement_index
                        && source.call_ordinal == edge.call_ordinal
                        && source.target_symbol == occurrence.registration_operation
                })
                .count()
        })
        .try_fold(0usize, usize::checked_add)
        .ok_or_else(|| boundary_identity_error("host-call link cardinality overflow"))?;
    if summary.links.len() != expected_link_count {
        return Err(boundary_identity_error(
            "host-call target-aware link cardinality drift",
        ));
    }

    for (source_call, call) in host_calls.calls.iter() {
        let matching_occurrences: Vec<_> = summary
            .host_calls
            .iter()
            .filter(|(_, occurrence)| {
                occurrence.source_call_index == source_call.arena_index()
                    && occurrence.source_call_generation == source_call.generation()
            })
            .collect();
        let [(occurrence_handle, occurrence)] = matching_occurrences.as_slice() else {
            return Err(boundary_identity_error(
                "host call does not have exactly one abstract occurrence",
            ));
        };
        let source_site = match call.source_site {
            Some(psi_checked_trees::NominalMachineUseSite::Statement(handle))
                if handle.is_valid() =>
            {
                AbstractHostCallSourceSite::Statement(handle)
            }
            Some(psi_checked_trees::NominalMachineUseSite::Expression(handle))
                if handle.is_valid() =>
            {
                AbstractHostCallSourceSite::Expression(handle)
            }
            _ => {
                return Err(boundary_identity_error(
                    "host call source use site is invalid",
                ));
            }
        };
        if occurrence.source_site != source_site
            || occurrence.registration_operation != call.registration_operation
            || occurrence.requirement_identity != call.requirement_identity
            || occurrence.source_key != call.source_key
            || occurrence.statement_index != call.statement_index
            || occurrence.call_ordinal != call.call_ordinal
            || occurrence.lowering_index != call.lowering.arena_index()
            || occurrence.lowering_generation != call.lowering.generation()
        {
            return Err(boundary_identity_error(
                "host-call occurrence identity drift",
            ));
        }

        let source_arguments = host_calls.arguments.span(call.arguments).ok_or_else(|| {
            boundary_identity_error("host call retained an invalid argument span")
        })?;
        let expected_arguments: Vec<_> = source_arguments
            .iter()
            .enumerate()
            .filter_map(|(index, argument)| argument.formal.map(|formal| (index, formal)))
            .collect();
        let retained_arguments = summary
            .host_call_arguments
            .span(occurrence.arguments)
            .ok_or_else(|| {
                boundary_identity_error("abstract host call retained an invalid argument span")
            })?;
        if retained_arguments.len() != expected_arguments.len() {
            return Err(boundary_identity_error(
                "host-call native argument cardinality drift",
            ));
        }
        for ((source_index, expected), retained) in
            expected_arguments.iter().zip(retained_arguments)
        {
            let expected_source_index = usize::try_from(expected.formal_ordinal)
                .unwrap_or(usize::MAX)
                .saturating_add(usize::from(call.has_result));
            if *source_index != expected_source_index
                || retained.formal_ordinal != expected.formal_ordinal
                || retained.native_parameter != Some(expected.native_parameter)
                || expected.native_parameter
                    != omega_calling_conventions::callback_native_parameter_id(
                        &call.requirement_identity,
                        expected.formal_ordinal,
                    )
            {
                return Err(boundary_identity_error(
                    "host-call native argument identity or order drift",
                ));
            }
        }

        let source_operations = host_calls.operations.span(call.operations).ok_or_else(|| {
            boundary_identity_error("host call retained an invalid operation span")
        })?;
        for (operation_ordinal, operation) in source_operations.iter().enumerate() {
            let matching_edges: Vec<_> = summary
                .edges
                .iter()
                .filter(|(_, edge)| {
                    edge.host_call == *occurrence_handle
                        && edge.operation_ordinal == operation_ordinal
                })
                .collect();
            let [(edge_handle, edge)] = matching_edges.as_slice() else {
                return Err(boundary_identity_error(
                    "host operation does not have exactly one abstract edge",
                ));
            };
            if edge.source_key != call.source_key
                || edge.statement_index != call.statement_index
                || edge.call_ordinal != call.call_ordinal
                || edge.operation_key != operation.operation_key
            {
                return Err(boundary_identity_error(
                    "abstract host operation identity drift",
                ));
            }

            let expected_sources: Vec<_> = summary
                .source_edges
                .iter()
                .filter(|(_, source)| {
                    source.source_key == call.source_key
                        && source.statement_index == call.statement_index
                        && source.call_ordinal == call.call_ordinal
                        && source.target_symbol == call.registration_operation
                })
                .map(|(handle, _)| handle)
                .collect();
            let actual_sources: Vec<_> = summary
                .links
                .iter()
                .filter(|(_, link)| link.lowered_edge == *edge_handle)
                .map(|(_, link)| link.source_edge)
                .collect();
            if actual_sources != expected_sources {
                return Err(boundary_identity_error(
                    "abstract boundary link is not exact and target-aware",
                ));
            }
        }
    }

    Ok(())
}

fn boundary_identity_error(message: &str) -> psi_diagnostics::Diagnostic {
    psi_diagnostics::Diagnostic::error(message)
}

fn append_boundary_links(
    summary: &mut AbstractBoundarySummary,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    call_ordinal: usize,
    registration_operation: psi_symbols::SymbolHandle,
    lowered_edge: psi_arena::Handle<AbstractBoundaryEdge>,
) {
    let source_edges: Vec<_> = summary
        .source_edges
        .iter()
        .filter_map(|(source_edge, edge)| {
            (edge.source_key == source_key
                && edge.statement_index == statement_index
                && edge.call_ordinal == call_ordinal
                && edge.target_symbol == registration_operation)
                .then_some(source_edge)
        })
        .collect();

    for source_edge in source_edges {
        summary.links.insert(AbstractBoundaryLink {
            source_edge,
            lowered_edge,
        });
    }
}

#[cfg(test)]
mod tests;
