//! Independent structural publication replay over retained current ABI/fragment data.
use super::*;
use calling_conventions::{IndirectPointerLocation, ValueLocation};
pub(in crate::function_fragments) fn admit(
    source: &StagedOptimizedRelocationFreeObjectContainer,
) -> Result<(), Error> {
    let fragments = source::fragments(source);
    let plan = source.source().source().source().selected_plan();
    if fragments.target != target::NativeTarget::uefi_x64()
        || !fragments.functions.is_empty()
        || plan.structural_unit_functions.len() != fragments.structural_unit_functions.len()
        || source.source().source().source().frame_layout().is_some()
    {
        return Err(Error::Unsupported(
            "structural publication requires the existing Microsoft owned-indirect ABI",
        ));
    }
    for fragment in &fragments.structural_unit_functions {
        let row = selected(source, fragment.machine)?;
        if row.abi.parameters.len() != 2
            || row.call.is_some() != fragment.block.call.is_some()
            || row.provenance != fragment.provenance
            || row.attachment != fragment.attachment
        {
            return Err(Error::Mismatch("structural ABI roster"));
        }
    }
    Ok(())
}
pub(in crate::function_fragments) fn validate_function(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    function: &ObjectFunction,
    rows: &[SemanticCodeAttribution],
) -> Result<(), Error> {
    let invalid = || Error::Mismatch("structural object differs from current ABI or call evidence");
    let selected = selected(source, function.machine)?;
    let fragment = source::fragments(source)
        .structural_unit_functions
        .iter()
        .find(|row| row.machine == function.machine)
        .ok_or_else(invalid)?;
    let (abstracted, _) = source::function(source, function.machine)?;
    if function.unit_parameters.len() != selected.abi.parameters.len()
        || function.unit_parameter_homes.len() != selected.abi.parameters.len()
        || function.unit_affine_cleanup.is_some()
        || function.internal_unit_calls.len() != usize::from(selected.call.is_some())
        || function.unit_call_stacks.len() != usize::from(selected.call.is_some())
        || function.scalar_stack.is_some()
    {
        return Err(invalid());
    }
    for (((parameter, home), source), binding) in function
        .unit_parameters
        .iter()
        .zip(&function.unit_parameter_homes)
        .zip(&selected.abi.parameters)
        .zip(&selected.abi.layout.bindings)
    {
        let source = &source.target;
        if parameter.place != source.place
            || parameter.structural_type != source.structural_type
            || parameter.multiplicity != source.multiplicity
            || parameter.access != source.access
            || parameter.shape != source.shape
            || home.place != source.place
            || home.structural_type != source.structural_type
            || home.multiplicity != source.multiplicity
            || home.access != source.access
            || home.shape != source.shape
            || home.source != source.placement
            || !home.indirect
            || home.location
                != (StructuralSourceLocation::IncomingIndirectPointer {
                    register: binding.pointer,
                })
            || !matches!(home.source.locations.as_slice(), [ValueLocation::Indirect { pointer: IndirectPointerLocation::Register(register), byte_size:16, .. }] if *register==binding.pointer)
        {
            return Err(invalid());
        }
    }
    let mut expected_sites = Vec::new();
    for settlement in &selected.boundary_settlements {
        expected_sites.push((
            SemanticCodeSite::Operation(settlement.operation),
            operation_ordinal(abstracted, settlement.operation)?,
            0,
            0,
        ));
    }
    let local_peak = if let (Some(call), Some(span)) = (&selected.call, &fragment.block.call) {
        let actual = &function.internal_unit_calls[0];
        if actual.owner != CallSiteOwner::Operation(call.operation)
            || actual.target != call.callee
            || actual.result.is_some()
            || actual.semantic_result.is_some()
            || actual.structural_result.is_some()
            || !actual.scalar_arguments.is_empty()
            || actual.claim_transfers != call.claim_transfers
            || actual.arguments.len() != call.arguments.len()
            || actual.operation_ordinal != operation_ordinal(abstracted, call.operation)?
            || actual.code_offset != host(span.offset)?
            || actual.byte_count != span.bytes.len()
        {
            return Err(invalid());
        }
        match (&actual.source, &call.source) {
            (
                InternalUnitCallSource::Authored,
                SelectedStructuralUnitCallSource::AuthoredCallUnit,
            ) => {}
            (
                InternalUnitCallSource::InstalledProvider {
                    boundary,
                    provider,
                    completion_claim_sources,
                    completion_receipts,
                },
                SelectedStructuralUnitCallSource::InstalledProvider {
                    boundary: expected_boundary,
                    provider: expected_provider,
                    completion_claim_sources: expected_sources,
                    completion_receipts: expected_receipts,
                },
            ) if boundary == expected_boundary
                && provider.as_ref() == expected_provider
                && completion_claim_sources == expected_sources
                && completion_receipts == expected_receipts => {}
            _ => return Err(invalid()),
        }
        for (index, ((argument, source), binding)) in actual
            .arguments
            .iter()
            .zip(&call.arguments)
            .zip(&call.layout.bindings)
            .enumerate()
        {
            let source = &source.target;
            let offset = host(span.offset)? + 4 + index * 30;
            if argument.place != source.place
                || argument.access != source.access
                || argument.path != source.path
                || argument.root_structural_type != source.root_structural_type
                || argument.structural_type != source.structural_type
                || argument.shape != source.shape
                || argument.source_byte_offset != source.source_byte_offset
                || argument.source_location
                    != (StructuralSourceLocation::IncomingIndirectPointer {
                        register: binding.pointer,
                    })
                || argument.call_stack_bytes != call.layout.outgoing_frame_byte_count
                || argument.fixed_array_length != source.fixed_array_length
                || argument.element_stride != source.element_stride
                || argument.source != source.source
                || argument.destination != source.destination
                || argument.code_offset != offset
                || argument.byte_count != 30
                || fragment.bytes.get(offset..offset + 30) != Some(argument.bytes.as_slice())
            {
                return Err(invalid());
            }
        }
        let stack = &function.unit_call_stacks[0];
        let resolved = source
            .source()
            .text_section()
            .resolved_internal_machine_calls
            .iter()
            .find(|row| row.caller == function.machine && row.operation == call.operation)
            .ok_or_else(invalid)?;
        let peak = call
            .layout
            .outgoing_frame_byte_count
            .checked_add(8)
            .ok_or(Error::Overflow)?;
        if stack.owner != actual.owner
            || stack.target != call.callee
            || stack.text_offset != host(resolved.field_section_offset)?
            || stack.active_frame_bytes != 0
            || stack.transient_bytes != peak
            || stack.caller_live_bytes != peak
        {
            return Err(invalid());
        }
        expected_sites.push((
            SemanticCodeSite::Operation(call.operation),
            actual.operation_ordinal,
            host(span.offset)?,
            span.bytes.len(),
        ));
        peak
    } else {
        0
    };
    if function.unit_stack
        != Some(ObjectUnitStack {
            frame_bytes: 0,
            local_peak_bytes: local_peak,
            stack_alignment: 16,
        })
    {
        return Err(invalid());
    }
    expected_sites.push((
        SemanticCodeSite::Edge(selected.terminator.psi_return_edge),
        abstracted
            .operations
            .len()
            .checked_sub(1)
            .ok_or(Error::Overflow)?,
        host(fragment.block.return_instruction.offset)?,
        fragment.block.return_instruction.bytes.len(),
    ));
    expected_sites.sort_by_key(|row| row.1);
    if rows.len() != expected_sites.len()
        || rows
            .iter()
            .zip(expected_sites)
            .any(|(row, (site, ordinal, offset, length))| {
                row.site != site
                    || row.operation_ordinal != ordinal
                    || row.code_offset != offset
                    || row.byte_count != length
            })
    {
        return Err(invalid());
    }
    Ok(())
}
pub(in crate::function_fragments) fn validate_settlements(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    rows: &[ObjectBoundarySettlement],
) -> Result<(), Error> {
    let mut cursor = 0;
    for placed in &source.source().text_section().functions {
        let Some(function) = source
            .source()
            .source()
            .source()
            .selected_plan()
            .structural_unit_functions
            .iter()
            .find(|row| row.machine == placed.machine)
        else {
            continue;
        };
        let (abstracted, _) = source::function(source, placed.machine)?;
        for expected in &function.boundary_settlements {
            let actual = rows
                .get(cursor)
                .ok_or(Error::Mismatch("missing structural settlement"))?;
            cursor += 1;
            let row = &actual.settlement;
            if actual.machine != placed.machine
                || actual.text_offset != host(placed.section_offset)?
                || row.psi_operation != expected.operation
                || row.boundary != expected.boundary
                || row.execution
                    != machine_code::BoundaryExecutionRecord::AdmittedProvider(
                        expected.provider_execution.into(),
                    )
                || row.realization
                    != target_operations::BoundaryRealization::ClaimCompletionOnly(
                        expected.realization,
                    )
                || row.arguments != expected.arguments
                || row.completion_claim_sources != expected.completion_claim_sources
                || row.completion_receipts != expected.completion_receipts
                || !row.scalar_arguments.is_empty()
                || !row.runtime_scalar_arguments.is_empty()
                || !row.byte_sequence_arguments.is_empty()
                || !row.native_result.is_unit()
                || row.operation_ordinal != operation_ordinal(abstracted, expected.operation)?
                || row.code_offset != 0
                || row.byte_count != 0
                || row.completion_provider_custody.len() != expected.completion_receipts.len()
            {
                return Err(Error::Mismatch("structural settlement custody"));
            }
            for (custody, receipt) in row
                .completion_provider_custody
                .iter()
                .zip(&expected.completion_receipts)
            {
                let expected_source = expected
                    .completion_claim_sources
                    .iter()
                    .find(|source| source.claim() == receipt.claim)
                    .ok_or(Error::Mismatch("missing completion source"))?;
                if custody.source != *expected_source
                    || custody.receipt != *receipt
                    || custody.provider_execution != expected.provider_execution.into()
                {
                    return Err(Error::Mismatch("completion provider custody"));
                }
            }
            crate::completion_receipts::validate_completion_custody(row)
                .map_err(|_| Error::Mismatch("completion receipts"))?;
        }
    }
    if cursor != rows.len() {
        return Err(Error::Mismatch("foreign structural settlement"));
    }
    Ok(())
}
