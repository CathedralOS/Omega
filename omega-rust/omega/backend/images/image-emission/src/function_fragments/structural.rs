//! Publication of structural ABI facts from the same validated fragment graph.
mod validation;
use super::{Error, host, source};
use crate::{ObjectBoundarySettlement, ObjectFunction, ObjectUnitCallStack, ObjectUnitStack};
use machine_code::{
    InternalUnitCallArgumentRecord, InternalUnitCallRecord, InternalUnitCallSource,
    SemanticCodeAttribution, SemanticCodeSite, StructuralSourceLocation, UnitParameterHomeRecord,
    UnitParameterRecord,
};
use object_file::StagedOptimizedRelocationFreeObjectContainer;
use selected_instructions::{SelectedStructuralUnitCallSource, SelectedStructuralUnitFunction};
use semantic_vocabulary::MachineId;
use target_operations::CallSiteOwner;
pub(super) use validation::{admit, validate_function, validate_settlements};

fn selected(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
) -> Result<&SelectedStructuralUnitFunction, Error> {
    source
        .source()
        .source()
        .source()
        .selected_plan()
        .structural_unit_functions
        .iter()
        .find(|row| row.machine == machine)
        .ok_or(Error::Mismatch("structural function has no selected ABI"))
}
pub(super) fn populate(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    function: &mut ObjectFunction,
) -> Result<Vec<SemanticCodeAttribution>, Error> {
    let selected = selected(source, function.machine)?;
    let fragment = source::fragments(source)
        .structural_unit_functions
        .iter()
        .find(|row| row.machine == function.machine)
        .ok_or(Error::Mismatch("missing structural fragment"))?;
    for (parameter, binding) in selected
        .abi
        .parameters
        .iter()
        .zip(&selected.abi.layout.bindings)
    {
        let target = &parameter.target;
        function.unit_parameters.push(UnitParameterRecord {
            place: target.place,
            structural_type: target.structural_type,
            multiplicity: target.multiplicity,
            access: target.access,
            shape: target.shape,
        });
        function.unit_parameter_homes.push(UnitParameterHomeRecord {
            place: target.place,
            structural_type: target.structural_type,
            multiplicity: target.multiplicity,
            access: target.access,
            shape: target.shape,
            source: target.placement.clone(),
            location: StructuralSourceLocation::IncomingIndirectPointer {
                register: binding.pointer,
            },
            indirect: true,
        });
    }
    let mut rows = Vec::new();
    let (abstracted, _) = source::function(source, function.machine)?;
    for settlement in &selected.boundary_settlements {
        rows.push(SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(settlement.operation),
            operation_ordinal: operation_ordinal(abstracted, settlement.operation)?,
            code_offset: 0,
            byte_count: 0,
        });
    }
    function.unit_stack = Some(ObjectUnitStack {
        frame_bytes: 0,
        local_peak_bytes: 0,
        stack_alignment: 16,
    });
    if let (Some(call), Some(span)) = (&selected.call, &fragment.block.call) {
        let mut arguments = Vec::new();
        for (index, (argument, binding)) in
            call.arguments.iter().zip(&call.layout.bindings).enumerate()
        {
            let argument = &argument.target;
            // The existing selected ABI bundle copies each 16-byte root as two
            // adjacent eight-byte load/store pairs; pointer installation is
            // accounted by the enclosing exact call span.
            let code_offset = host(span.offset)?
                .checked_add(4 + index * 30)
                .ok_or(Error::Overflow)?;
            let bytes = fragment
                .bytes
                .get(code_offset..code_offset + 30)
                .ok_or(Error::Mismatch("structural copy extent"))?
                .to_vec();
            arguments.push(InternalUnitCallArgumentRecord {
                place: argument.place,
                access: argument.access,
                path: argument.path.clone(),
                root_structural_type: argument.root_structural_type,
                structural_type: argument.structural_type,
                shape: argument.shape,
                source_byte_offset: argument.source_byte_offset,
                source_location: StructuralSourceLocation::IncomingIndirectPointer {
                    register: binding.pointer,
                },
                call_stack_bytes: call.layout.outgoing_frame_byte_count,
                fixed_array_length: argument.fixed_array_length,
                element_stride: argument.element_stride,
                source: argument.source.clone(),
                destination: argument.destination.clone(),
                code_offset,
                byte_count: bytes.len(),
                bytes,
            });
        }
        let call_source = match &call.source {
            SelectedStructuralUnitCallSource::AuthoredCallUnit => InternalUnitCallSource::Authored,
            SelectedStructuralUnitCallSource::InstalledProvider {
                boundary,
                provider,
                completion_claim_sources,
                completion_receipts,
            } => InternalUnitCallSource::InstalledProvider {
                boundary: *boundary,
                provider: Box::new(provider.clone()),
                completion_claim_sources: completion_claim_sources.clone(),
                completion_receipts: completion_receipts.clone(),
            },
        };
        let ordinal = operation_ordinal(abstracted, call.operation)?;
        function.internal_unit_calls.push(InternalUnitCallRecord {
            source: call_source,
            owner: CallSiteOwner::Operation(call.operation),
            target: call.callee,
            result: None,
            semantic_result: None,
            structural_result: None,
            scalar_arguments: Vec::new(),
            arguments,
            claim_transfers: call.claim_transfers.clone(),
            operation_ordinal: ordinal,
            code_offset: host(span.offset)?,
            byte_count: span.bytes.len(),
        });
        rows.push(SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(call.operation),
            operation_ordinal: ordinal,
            code_offset: host(span.offset)?,
            byte_count: span.bytes.len(),
        });
        let call_site = source
            .source()
            .text_section()
            .resolved_internal_machine_calls
            .iter()
            .find(|row| row.caller == function.machine && row.operation == call.operation)
            .ok_or(Error::Mismatch("missing resolved structural call"))?;
        let transient_bytes = call
            .layout
            .outgoing_frame_byte_count
            .checked_add(8)
            .ok_or(Error::Overflow)?;
        function.unit_call_stacks.clear();
        function.unit_call_stacks.push(ObjectUnitCallStack {
            owner: CallSiteOwner::Operation(call.operation),
            target: call.callee,
            text_offset: host(call_site.field_section_offset)?,
            active_frame_bytes: 0,
            transient_bytes,
            caller_live_bytes: transient_bytes,
        });
        function
            .unit_stack
            .as_mut()
            .expect("Unit stack")
            .local_peak_bytes = transient_bytes;
    }
    rows.push(SemanticCodeAttribution {
        site: SemanticCodeSite::Edge(selected.terminator.psi_return_edge),
        operation_ordinal: abstracted
            .operations
            .len()
            .checked_sub(1)
            .ok_or(Error::Overflow)?,
        code_offset: host(fragment.block.return_instruction.offset)?,
        byte_count: fragment.block.return_instruction.bytes.len(),
    });
    rows.sort_by_key(|row| row.operation_ordinal);
    Ok(rows)
}
fn operation_ordinal(
    function: &abstract_operations::AbstractFunction,
    operation: semantic_vocabulary::OperationId,
) -> Result<usize, Error> {
    function.operations.iter().position(|row| matches!(row,
        abstract_operations::AbstractOperation::CallUnit { psi_operation, .. }
        | abstract_operations::AbstractOperation::BoundaryCall { psi_operation, .. } if *psi_operation == operation
    )).ok_or(Error::Mismatch("structural operation ordinal"))
}
pub(super) fn settlements(
    source: &StagedOptimizedRelocationFreeObjectContainer,
) -> Result<Vec<ObjectBoundarySettlement>, Error> {
    let mut result = Vec::new();
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
        for row in &function.boundary_settlements {
            let execution = machine_code::BoundaryExecutionRecord::AdmittedProvider(
                row.provider_execution.into(),
            );
            result.push(ObjectBoundarySettlement {
                machine: placed.machine,
                text_offset: host(placed.section_offset)?,
                settlement: machine_code::BoundarySettlementRecord {
                    psi_operation: row.operation,
                    boundary: row.boundary,
                    execution,
                    realization: target_operations::BoundaryRealization::ClaimCompletionOnly(
                        row.realization,
                    ),
                    scalar_arguments: Vec::new(),
                    runtime_scalar_arguments: Vec::new(),
                    arguments: row.arguments.clone(),
                    byte_sequence_arguments: Vec::new(),
                    completion_claim_sources: row.completion_claim_sources.clone(),
                    completion_receipts: row.completion_receipts.clone(),
                    completion_provider_custody: machine_code::derive_completion_provider_custody(
                        execution,
                        &row.completion_claim_sources,
                        &row.completion_receipts,
                    )
                    .ok_or(Error::Mismatch("completion custody"))?,
                    native_result: machine_code::BoundaryResultRecord::Unit,
                    operation_ordinal: operation_ordinal(abstracted, row.operation)?,
                    code_offset: 0,
                    byte_count: 0,
                },
            });
        }
    }
    Ok(result)
}
