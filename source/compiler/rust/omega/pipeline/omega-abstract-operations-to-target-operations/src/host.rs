use omega_abstract_operations::{
    AbstractHostOperationProvenance, AbstractOperation, AbstractOperationKind,
    AbstractOperationPlan,
};
use omega_calling_conventions::{HostAbiPlan, HostOperationKey};
use omega_platform_interface::HostCallPlan;
use omega_target_operations::{
    RuntimeTextReadSource, TargetHostBinding, TargetHostFormalOperandBinding,
    TargetHostOperationProvenance, TargetOperationCode, TargetOperationKind,
};
use psi_arena::{Handle, HandleSpan};
use psi_diagnostics::Diagnostic;

use crate::remap;

pub(crate) fn copy_runtime_text_host_bindings(
    host_abi: &HostAbiPlan,
    abstract_operations: &AbstractOperationPlan,
    code: &mut TargetOperationCode,
) {
    for (instruction_key, instruction) in abstract_operations.code.instructions.iter() {
        if !matches!(
            &instruction.kind,
            AbstractOperationKind::ReadRuntimeTextLine { .. }
                | AbstractOperationKind::ReadRuntimeByte { .. }
                | AbstractOperationKind::WriteRuntimeByte { .. }
        ) {
            continue;
        }

        let (TargetOperationKind::ReadRuntimeTextLine {
            source: RuntimeTextReadSource::HostOperation { operation_key },
            ..
        }
        | TargetOperationKind::ReadRuntimeByte {
            source: RuntimeTextReadSource::HostOperation { operation_key },
            ..
        }
        | TargetOperationKind::WriteRuntimeByte {
            source: RuntimeTextReadSource::HostOperation { operation_key },
            ..
        }) = &code
            .instructions
            .get(remap::instruction_handle(instruction_key))
            .kind
        else {
            continue;
        };

        if target_host_binding(code, *operation_key).is_some() {
            continue;
        }

        if let Some((_, binding)) = host_abi
            .bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == *operation_key)
        {
            code.host_bindings.insert(binding.clone());
        }
    }
}

fn target_host_binding(
    code: &TargetOperationCode,
    operation_key: HostOperationKey,
) -> Option<&TargetHostBinding> {
    code.host_bindings
        .iter()
        .find(|(_, binding)| binding.operation_key == operation_key)
        .map(|(_, binding)| binding)
}

pub(crate) fn resolve_operation(
    host_calls: &HostCallPlan,
    abstract_operations: &AbstractOperationPlan,
    instruction: &AbstractOperation,
    operation_ordinal: u16,
    provenance: Option<&AbstractHostOperationProvenance>,
) -> Result<(HostOperationKey, Option<TargetHostOperationProvenance>), Diagnostic> {
    let host_call = if let Some(provenance) = provenance {
        host_calls
            .calls
            .iter()
            .find(|(handle, _)| {
                handle.arena_index() == provenance.source_call_index
                    && handle.generation() == provenance.source_call_generation
            })
            .map(|(_, host_call)| host_call)
            .ok_or_else(|| provenance_error("source host-call handle is missing or stale"))?
    } else {
        host_calls
            .calls
            .iter()
            .find(|(_, host_call)| {
                host_call.source_key == instruction.source_key
                    && host_call.statement_index == instruction.source_statement
            })
            .map(|(_, host_call)| host_call)
            .ok_or_else(|| provenance_error("host operation has no matching source call"))?
    };
    if host_call.source_key != instruction.source_key
        || host_call.statement_index != instruction.source_statement
        || provenance.is_some_and(|row| row.call_ordinal != host_call.call_ordinal)
    {
        return Err(provenance_error(
            "source host-call coordinates or call ordinal drifted",
        ));
    }

    let operations = host_calls
        .operations
        .span(host_call.operations)
        .ok_or_else(|| provenance_error("source host call retained an invalid operation span"))?;

    let ordinal = usize::from(operation_ordinal);
    let operation = operations
        .get(ordinal)
        .ok_or_else(|| provenance_error("host operation ordinal is out of range"))?;
    let Some(provenance) = provenance else {
        return Ok((operation.operation_key, None));
    };
    if provenance.operation_ordinal != operation_ordinal {
        return Err(provenance_error(
            "retained host operation ordinal disagrees with the selected operation",
        ));
    }

    let matching_occurrences = abstract_operations
        .semantics
        .boundaries
        .host_calls
        .iter()
        .filter(|(_, occurrence)| {
            occurrence.source_call_index == provenance.source_call_index
                && occurrence.source_call_generation == provenance.source_call_generation
                && occurrence.source_key == host_call.source_key
                && occurrence.statement_index == host_call.statement_index
                && occurrence.call_ordinal == host_call.call_ordinal
                && occurrence.registration_operation == host_call.registration_operation
                && occurrence.requirement_identity == host_call.requirement_identity
                && occurrence.lowering_index == host_call.lowering.arena_index()
                && occurrence.lowering_generation == host_call.lowering.generation()
        })
        .collect::<Vec<_>>();
    let [(occurrence_handle, occurrence)] = matching_occurrences.as_slice() else {
        return Err(provenance_error(
            "opted-in host operation does not resolve to exactly one source occurrence",
        ));
    };
    let matching_edges = abstract_operations
        .semantics
        .boundaries
        .edges
        .iter()
        .filter(|(_, edge)| {
            edge.host_call == *occurrence_handle
                && edge.source_key == host_call.source_key
                && edge.statement_index == host_call.statement_index
                && edge.call_ordinal == host_call.call_ordinal
                && edge.operation_ordinal == ordinal
                && edge.operation_key == operation.operation_key
        })
        .collect::<Vec<_>>();
    let [(edge_handle, _)] = matching_edges.as_slice() else {
        return Err(provenance_error(
            "opted-in host operation does not resolve to exactly one boundary edge",
        ));
    };

    let source_formals = host_calls
        .arguments
        .span(host_call.arguments)
        .ok_or_else(|| provenance_error("source host call retained an invalid argument span"))?
        .iter()
        .filter_map(|argument| argument.formal)
        .collect::<Vec<_>>();
    let native_arguments = abstract_operations
        .semantics
        .boundaries
        .host_call_arguments
        .span(occurrence.arguments)
        .ok_or_else(|| provenance_error("source occurrence retained an invalid argument span"))?;
    if provenance.formal_operands.len() != source_formals.len()
        || native_arguments.len() != source_formals.len()
    {
        return Err(provenance_error(
            "opted-in host operation formal-operand cardinality drifted",
        ));
    }
    let operation_operands = match &instruction.kind {
        AbstractOperationKind::HostOperation { operands, .. } => *operands,
        _ => {
            return Err(provenance_error(
                "provenance is attached to a non-host operation",
            ));
        }
    };
    let mut target_formals = Vec::with_capacity(source_formals.len());
    let mut retained_operands = Vec::with_capacity(source_formals.len());
    for (index, ((retained, source), native)) in provenance
        .formal_operands
        .iter()
        .zip(&source_formals)
        .zip(native_arguments)
        .enumerate()
    {
        if retained.formal_ordinal != source.formal_ordinal
            || retained.native_parameter != source.native_parameter
            || native.formal_ordinal != retained.formal_ordinal
            || native.native_parameter != Some(retained.native_parameter)
            || !handle_is_in_span(retained.operand, operation_operands)
            || abstract_operations
                .code
                .operands
                .iter()
                .all(|(handle, _)| handle != retained.operand)
            || retained_operands.contains(&retained.operand)
        {
            return Err(provenance_error(&format!(
                "opted-in host operation formal operand {index} drifted"
            )));
        }
        retained_operands.push(retained.operand);
        let native_argument =
            Handle::from_parts(
                occurrence
                    .arguments
                    .start()
                    .arena_index()
                    .checked_add(u32::try_from(index).map_err(|_| {
                        provenance_error("host formal index exceeds the arena domain")
                    })?)
                    .ok_or_else(|| provenance_error("host formal handle overflowed"))?,
                occurrence.arguments.start().generation(),
            );
        target_formals.push(TargetHostFormalOperandBinding {
            native_argument,
            formal_ordinal: retained.formal_ordinal,
            native_parameter: retained.native_parameter,
            abstract_operand: retained.operand,
            abstract_operand_kind: abstract_operations
                .code
                .operands
                .iter()
                .find(|(handle, _)| *handle == retained.operand)
                .map(|(_, operand)| operand.kind.clone())
                .ok_or_else(|| provenance_error("retained abstract operand is missing"))?,
            operand: remap::operand_handle(retained.operand),
        });
    }

    Ok((
        operation.operation_key,
        Some(TargetHostOperationProvenance {
            occurrence: *occurrence_handle,
            boundary_edge: *edge_handle,
            call_ordinal: provenance.call_ordinal,
            operation_ordinal,
            formal_operands: target_formals.into(),
        }),
    ))
}

fn handle_is_in_span<T>(handle: Handle<T>, span: HandleSpan<T>) -> bool {
    if span.is_empty() || handle.generation() != span.start().generation() {
        return false;
    }
    let start = span.start().arena_index();
    handle.arena_index() >= start && handle.arena_index() < start.saturating_add(span.count())
}

fn provenance_error(message: &str) -> Diagnostic {
    Diagnostic::error(format!("host-operation operand provenance: {message}"))
}
