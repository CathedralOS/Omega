//! Selected byte-output custody rejoins source SSA, resolved scratch and decoded bytes.
use super::{Error, attribution, fragment, host, selected, source};
use crate::ObjectBoundarySettlement;
use calling_conventions::{ValueLocation, ValuePlacement, ValueShape};
use machine_code::{
    BoundaryExecutionRecord, BoundaryResultRecord, BoundarySettlementRecord,
    ForeignCallScalarArgumentRecord, InternalUnitScalarArgumentSourceRecord, SemanticCodeSite,
};
use object_file::StagedOptimizedRelocationFreeObjectContainer;
use selected_instructions::{
    LocalStorageSlotId, SelectedBoundarySettlement, SelectedBoundarySettlementPayload,
    SelectedInstructionKind,
};
use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, ScalarType};

pub(super) fn settlement(
    container: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
    located: &SelectedBoundarySettlement,
) -> Result<ObjectBoundarySettlement, Error> {
    let SelectedBoundarySettlementPayload::HostedWriteByteI32 {
        operation,
        boundary,
        source: value,
    } = located.settlement
    else {
        return Err(Error::Mismatch("selected byte output payload"));
    };
    let function = selected(container, machine)?;
    let fragment = fragment(container, machine)?;
    let (abstracted, _) = source::function(container, machine)?;
    let instruction = function
        .blocks
        .iter()
        .find(|block| block.id == located.block)
        .and_then(|block| block.instructions.get(located.instruction_index as usize))
        .ok_or(Error::Mismatch("selected byte output instruction"))?;
    let slot = LocalStorageSlotId::Boundary { operation };
    if instruction.kind != (SelectedInstructionKind::HostedWriteByteI32 { slot })
        || instruction.operands.len() != 1
        || instruction.provenance.operations.last() != Some(&operation)
        || instruction.provenance.values != [value]
    {
        return Err(Error::Mismatch(
            "byte output source or scratch substitution",
        ));
    }
    let span = fragment
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|span| span.instruction == instruction.id)
        .ok_or(Error::Mismatch("selected byte output span"))?;
    let native = container.source().text_section().target;
    let (register, offset) =
        crate::runtime_scalar_custody::decode_selected_byte_output(native, &span.bytes)
            .ok_or(Error::Mismatch("selected byte output encoding"))?;
    let frame = source::frame(container, machine)?.ok_or(Error::Mismatch("byte output frame"))?;
    let scratch = frame
        .local_storage_slots
        .iter()
        .find(|candidate| candidate.id == slot)
        .ok_or(Error::Mismatch("byte output scratch slot"))?;
    if scratch.frame_offset_bytes != u64::from(offset)
        || scratch.size_bytes != 1
        || scratch.alignment_bytes != 1
        || u64::from(offset) >= frame.frame_size_bytes
    {
        return Err(Error::Mismatch("byte output scratch geometry"));
    }
    let code_offset = host(span.offset)?;
    let scalar_type = ScalarType::Integer(
        IntegerType::new(IntegerSign::Signed, 32).map_err(|_| Error::Mismatch("i32"))?,
    );
    let (execution, realization) = target_settlement(container, machine, operation, boundary)?;
    let settlement = BoundarySettlementRecord {
        psi_operation: operation,
        boundary,
        execution: execution.into(),
        realization,
        scalar_arguments: Vec::new(),
        runtime_scalar_arguments: vec![ForeignCallScalarArgumentRecord {
            parameter_index: 0,
            source: InternalUnitScalarArgumentSourceRecord::SelectedBoundary {
                source_value: value,
                scalar_type,
                instruction: instruction.id,
                scratch_byte_offset: scratch.frame_offset_bytes,
            },
            placement: ValuePlacement {
                shape: ValueShape::integer(4, 4),
                locations: vec![ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 4,
                }],
            },
            code_offset,
            byte_count: span.bytes.len(),
        }],
        arguments: Vec::new(),
        byte_sequence_arguments: Vec::new(),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
        completion_provider_custody: Vec::new(),
        native_result: BoundaryResultRecord::Unit,
        operation_ordinal: attribution::ordinal(
            abstracted,
            SemanticCodeSite::Operation(operation),
        )?,
        code_offset,
        byte_count: span.bytes.len(),
    };
    let placed = container
        .source()
        .text_section()
        .functions
        .iter()
        .find(|placed| placed.machine == machine)
        .ok_or(Error::Mismatch("byte output placed function"))?;
    Ok(ObjectBoundarySettlement {
        machine,
        text_offset: host(placed.section_offset)?
            .checked_add(code_offset)
            .ok_or(Error::Overflow)?,
        settlement,
    })
}

pub(super) fn validate(
    container: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
    located: &SelectedBoundarySettlement,
    proposed: &ObjectBoundarySettlement,
) -> Result<(), Error> {
    let invalid = || Error::Mismatch("selected byte output publication custody");
    let SelectedBoundarySettlementPayload::HostedWriteByteI32 {
        operation,
        boundary,
        source: value,
    } = located.settlement
    else {
        return Err(invalid());
    };
    let function = selected(container, machine)?;
    let fragment = fragment(container, machine)?;
    let row = function
        .blocks
        .iter()
        .find(|block| block.id == located.block)
        .and_then(|block| block.instructions.get(located.instruction_index as usize))
        .ok_or_else(invalid)?;
    let slot = LocalStorageSlotId::Boundary { operation };
    if row.kind != (SelectedInstructionKind::HostedWriteByteI32 { slot })
        || row.operands.len() != 1
        || row.provenance.operations.last() != Some(&operation)
        || row.provenance.values != [value]
    {
        return Err(invalid());
    }
    let span = fragment
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|span| span.instruction == row.id)
        .ok_or_else(invalid)?;
    let frame = source::frame(container, machine)?.ok_or_else(invalid)?;
    let scratch = frame
        .local_storage_slots
        .iter()
        .find(|candidate| candidate.id == slot)
        .ok_or_else(invalid)?;
    let placed = container
        .source()
        .text_section()
        .functions
        .iter()
        .find(|placed| placed.machine == machine)
        .ok_or_else(invalid)?;
    let settlement = &proposed.settlement;
    let [argument] = settlement.runtime_scalar_arguments.as_slice() else {
        return Err(invalid());
    };
    let InternalUnitScalarArgumentSourceRecord::SelectedBoundary {
        source_value,
        scalar_type,
        instruction,
        scratch_byte_offset,
    } = argument.source
    else {
        return Err(invalid());
    };
    let offset = host(span.offset)?;
    let (abstracted, _) = source::function(container, machine)?;
    let (execution, realization) = target_settlement(container, machine, operation, boundary)?;
    if settlement.execution != BoundaryExecutionRecord::from(execution)
        || settlement.realization != realization
        || proposed.machine != machine
        || proposed.text_offset
            != host(placed.section_offset)?
                .checked_add(offset)
                .ok_or(Error::Overflow)?
        || settlement.psi_operation != operation
        || settlement.boundary != boundary
        || settlement.operation_ordinal
            != attribution::ordinal(abstracted, SemanticCodeSite::Operation(operation))?
        || source_value != value
        || instruction != row.id
        || !matches!(scalar_type, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Signed && integer.bits() == 32)
        || scratch_byte_offset != scratch.frame_offset_bytes
        || scratch.size_bytes != 1
        || scratch.alignment_bytes != 1
        || scratch_byte_offset >= frame.frame_size_bytes
        || settlement.code_offset != offset
        || settlement.byte_count != span.bytes.len()
        || !crate::runtime_scalar_custody::selected_byte_output_bytes_are_exact(
            container.source().text_section().target,
            settlement,
            &fragment.bytes,
        )
    {
        return Err(invalid());
    }
    Ok(())
}

// The selected instruction describes the physical mechanism, not the provider
// admitting it. Rejoin the exact retained target occurrence on construction and
// replay so an ordinary provider can never acquire compiler-builtin custody.
fn target_settlement(
    container: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
    operation: semantic_vocabulary::OperationId,
    boundary: semantic_vocabulary::BoundaryMachineId,
) -> Result<
    (
        target_operations::BoundaryExecutionBinding,
        target_operations::BoundaryRealization,
    ),
    Error,
> {
    use target_operations::{
        BoundaryExecutionBinding, BoundaryRealization, CompilerBuiltinExecution, TargetOperation,
        TargetUnitOperation,
    };
    let (_, target) = source::function(container, machine)?;
    let mut found = None;
    let mut inspect = |row: &TargetUnitOperation| -> Result<(), Error> {
        if let TargetUnitOperation::BoundarySettlement {
            psi_operation,
            boundary: source_boundary,
            execution,
            realization,
            ..
        } = row
            && *psi_operation == operation
        {
            if *source_boundary != boundary
                || found.is_some()
                || !matches!(
                    (execution, realization),
                    (
                        BoundaryExecutionBinding::AdmittedProvider(_),
                        BoundaryRealization::HostedWriteByteI32(_)
                    ) | (
                        BoundaryExecutionBinding::CompilerBuiltin(
                            CompilerBuiltinExecution::HostedWriteByteI32
                        ),
                        BoundaryRealization::HostedWriteByteI32(_)
                    )
                )
            {
                return Err(Error::Mismatch("byte output target settlement custody"));
            }
            found = Some((*execution, *realization));
        }
        Ok(())
    };
    match &target.operation {
        TargetOperation::UnitBody(body) => {
            for row in &body.operations {
                inspect(row)?;
            }
        }
        TargetOperation::ControlGraph(graph) => {
            for block in &graph.blocks {
                for row in &block.operations {
                    inspect(row)?;
                }
            }
        }
        _ => return Err(Error::Mismatch("byte output target function role")),
    }
    found.ok_or(Error::Mismatch("missing byte output target settlement"))
}
