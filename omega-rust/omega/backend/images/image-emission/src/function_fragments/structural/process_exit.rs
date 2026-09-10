//! Nonreturning boundary custody: selected SSA and terminal span, without scratch.
use super::{Error, attribution, fragment, host, selected, source};
use crate::ObjectBoundarySettlement;
use calling_conventions::{ValueLocation, ValuePlacement, ValueShape};
use machine_code::{
    BoundaryExecutionRecord, BoundaryResultRecord, BoundarySettlementRecord,
    ForeignCallScalarArgumentRecord, InternalUnitScalarArgumentSourceRecord, SemanticCodeSite,
};
use object_file::StagedOptimizedRelocationFreeObjectContainer;
use selected_instructions::{
    SelectedBoundarySettlement, SelectedBoundarySettlementPayload, SelectedInstructionKind,
    SelectedTerminator,
};
use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, ScalarType};

pub(super) fn settlement(
    container: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
    located: &SelectedBoundarySettlement,
) -> Result<ObjectBoundarySettlement, Error> {
    let SelectedBoundarySettlementPayload::HostedExitProcessI32 {
        operation,
        boundary,
        source: value,
    } = located.settlement
    else {
        return Err(Error::Mismatch("selected process exit payload"));
    };
    let function = selected(container, machine)?;
    let fragment = fragment(container, machine)?;
    let (abstracted, _) = source::function(container, machine)?;
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == located.block)
        .ok_or(Error::Mismatch("process exit block"))?;
    let SelectedTerminator::HostedExitProcess {
        instruction,
        nominal_return_edge,
    } = &block.terminator
    else {
        return Err(Error::Mismatch("process exit terminal"));
    };
    if located.instruction_index as usize != block.instructions.len()
        || instruction.kind != SelectedInstructionKind::HostedExitProcessI32
        || instruction.operands.len() != 1
        || instruction.provenance.operations.last() != Some(&operation)
        || instruction.provenance.values != [value]
    {
        return Err(Error::Mismatch("process exit source substitution"));
    }
    let span = fragment
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|span| span.instruction == instruction.id)
        .ok_or(Error::Mismatch("process exit span"))?;
    if span.control
        != (machine_code::FunctionFragmentControlProvenance::HostedExitProcess {
            nominal_return_edge: *nominal_return_edge,
        })
    {
        return Err(Error::Mismatch("process exit nominal edge"));
    }
    let register = crate::runtime_scalar_custody::process_exit::decode(
        container.source().text_section().target,
        &span.bytes,
    )
    .ok_or(Error::Mismatch("process exit encoding"))?;
    let code_offset = host(span.offset)?;
    let scalar_type = ScalarType::Integer(
        IntegerType::new(IntegerSign::Signed, 32).map_err(|_| Error::Mismatch("i32"))?,
    );
    let settlement = BoundarySettlementRecord {
        psi_operation: operation,
        boundary,
        execution: BoundaryExecutionRecord::CompilerBuiltin(
            target_operations::CompilerBuiltinExecution::HostedExitProcessI32,
        ),
        realization: target_operations::BoundaryRealization::HostedExitProcessI32(
            Default::default(),
        ),
        scalar_arguments: Vec::new(),
        runtime_scalar_arguments: vec![ForeignCallScalarArgumentRecord {
            parameter_index: 0,
            source: InternalUnitScalarArgumentSourceRecord::SelectedProcessExit {
                source_value: value,
                scalar_type,
                instruction: instruction.id,
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
        .find(|row| row.machine == machine)
        .ok_or(Error::Mismatch("process exit placed function"))?;
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
    let invalid = || Error::Mismatch("process exit publication custody");
    let SelectedBoundarySettlementPayload::HostedExitProcessI32 {
        operation,
        boundary,
        source: value,
    } = located.settlement
    else {
        return Err(invalid());
    };
    let function = selected(container, machine)?;
    let fragment = fragment(container, machine)?;
    let block = function
        .blocks
        .iter()
        .find(|row| row.id == located.block)
        .ok_or_else(invalid)?;
    let SelectedTerminator::HostedExitProcess {
        instruction,
        nominal_return_edge,
    } = &block.terminator
    else {
        return Err(invalid());
    };
    let span = fragment
        .blocks
        .iter()
        .flat_map(|row| &row.instructions)
        .find(|row| row.instruction == instruction.id)
        .ok_or_else(invalid)?;
    let [argument] = proposed.settlement.runtime_scalar_arguments.as_slice() else {
        return Err(invalid());
    };
    let InternalUnitScalarArgumentSourceRecord::SelectedProcessExit {
        source_value,
        instruction: source_instruction,
        ..
    } = argument.source
    else {
        return Err(invalid());
    };
    let placed = container
        .source()
        .text_section()
        .functions
        .iter()
        .find(|row| row.machine == machine)
        .ok_or_else(invalid)?;
    let (abstracted, _) = source::function(container, machine)?;
    if located.instruction_index as usize != block.instructions.len()
        || instruction.kind != SelectedInstructionKind::HostedExitProcessI32
        || instruction.operands.len() != 1
        || instruction.provenance.operations.last() != Some(&operation)
        || instruction.provenance.values != [value]
        || source_value != value
        || source_instruction != instruction.id
        || span.control
            != (machine_code::FunctionFragmentControlProvenance::HostedExitProcess {
                nominal_return_edge: *nominal_return_edge,
            })
        || proposed.machine != machine
        || proposed.text_offset
            != host(placed.section_offset)?
                .checked_add(host(span.offset)?)
                .ok_or(Error::Overflow)?
        || proposed.settlement.psi_operation != operation
        || proposed.settlement.boundary != boundary
        || proposed.settlement.operation_ordinal
            != attribution::ordinal(abstracted, SemanticCodeSite::Operation(operation))?
        || proposed.settlement.code_offset != host(span.offset)?
        || proposed.settlement.byte_count != span.bytes.len()
        || !crate::runtime_scalar_custody::process_exit::bytes_are_exact(
            container.source().text_section().target,
            &proposed.settlement,
            &fragment.bytes,
        )
    {
        return Err(invalid());
    }
    Ok(())
}
