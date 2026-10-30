//! Nonreturning boundary custody: selected SSA and terminal span, without scratch.
use super::{Error, attribution, fragment, host, placed_text_offset, selected, source};
use crate::ObjectBoundarySettlement;
use crate::object_artifact::replay::boundary::runtime_scalar_custody::hosted_i32_argument;
use machine_code::{
    BoundaryResultRecord, BoundarySettlementRecord, InternalUnitScalarArgumentSourceRecord,
    SemanticCodeSite,
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
        edge,
        execution,
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
        .block(located.block)
        .ok_or(Error::Mismatch("process exit block"))?;
    let SelectedTerminator::HostedExitProcess {
        instruction,
        completion_edge,
    } = &block.terminator
    else {
        return Err(Error::Mismatch("process exit terminal"));
    };
    if located.instruction_index as usize != block.instructions.len()
        || edge != *completion_edge
        || instruction.kind != SelectedInstructionKind::HostedExitProcessI32
        || instruction.operands.len() != 1
        || instruction.provenance.edges.last() != Some(&edge)
        || instruction.provenance.values != [value]
    {
        return Err(Error::Mismatch("process exit source substitution"));
    }
    let span = fragment
        .instruction_span(instruction.id)
        .ok_or(Error::Mismatch("process exit span"))?;
    if span.control
        != (machine_code::FunctionFragmentControlProvenance::HostedExitProcess {
            completion_edge: *completion_edge,
        })
    {
        return Err(Error::Mismatch("process exit completion edge"));
    }
    let register =
        crate::object_artifact::replay::boundary::runtime_scalar_custody::process_exit::decode(
            container.source().text_section().target,
            &span.bytes,
        )
        .ok_or(Error::Mismatch("process exit encoding"))?;
    let code_offset = host(span.offset)?;
    let scalar_type = ScalarType::Integer(
        IntegerType::new(IntegerSign::Signed, 32).map_err(|_| Error::Mismatch("i32"))?,
    );
    let settlement = BoundarySettlementRecord {
        invocation_site: semantic_vocabulary::InvocationSite::ExternalCompletion(edge),
        boundary,
        execution: execution.into(),
        realization: target_operations::BoundaryRealization::HostedExitProcessI32(
            Default::default(),
        ),
        scalar_arguments: Vec::new(),
        runtime_scalar_arguments: vec![hosted_i32_argument(
            InternalUnitScalarArgumentSourceRecord::SelectedProcessExit {
                source_value: value,
                scalar_type,
                instruction: instruction.id,
            },
            register,
            code_offset,
            span.bytes.len(),
        )],
        arguments: Vec::new(),
        byte_sequence_arguments: Vec::new(),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
        completion_provider_custody: Vec::new(),
        native_result: BoundaryResultRecord::Unit,
        operation_ordinal: attribution::ordinal(abstracted, SemanticCodeSite::Edge(edge))?,
        code_offset,
        byte_count: span.bytes.len(),
    };
    let placed = container
        .source()
        .text_section()
        .function(machine)
        .ok_or(Error::Mismatch("process exit placed function"))?;
    Ok(ObjectBoundarySettlement {
        machine,
        text_offset: placed_text_offset(placed.section_offset, code_offset)?,
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
        edge,
        execution,
        boundary,
        source: value,
    } = located.settlement
    else {
        return Err(invalid());
    };
    let function = selected(container, machine)?;
    let fragment = fragment(container, machine)?;
    let block = function.block(located.block).ok_or_else(invalid)?;
    let SelectedTerminator::HostedExitProcess {
        instruction,
        completion_edge,
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
        .function(machine)
        .ok_or_else(invalid)?;
    let (abstracted, _) = source::function(container, machine)?;
    let offset = host(span.offset)?;
    if located.instruction_index as usize != block.instructions.len()
        || edge != *completion_edge
        || instruction.kind != SelectedInstructionKind::HostedExitProcessI32
        || instruction.operands.len() != 1
        || instruction.provenance.edges.last() != Some(&edge)
        || instruction.provenance.values != [value]
        || source_value != value
        || source_instruction != instruction.id
        || span.control
            != (machine_code::FunctionFragmentControlProvenance::HostedExitProcess {
                completion_edge: *completion_edge,
            })
        || proposed.machine != machine
        || proposed.text_offset != placed_text_offset(placed.section_offset, offset)?
        || proposed.settlement.invocation_site != semantic_vocabulary::InvocationSite::ExternalCompletion(edge)
        || proposed.settlement.execution != execution.into()
        || proposed.settlement.boundary != boundary
        || proposed.settlement.operation_ordinal
            != attribution::ordinal(abstracted, SemanticCodeSite::Edge(edge))?
        || proposed.settlement.code_offset != offset
        || proposed.settlement.byte_count != span.bytes.len()
        || !crate::object_artifact::replay::boundary::runtime_scalar_custody::process_exit::bytes_are_exact(
            container.source().text_section().target,
            &proposed.settlement,
            &fragment.bytes,
        )
    {
        return Err(invalid());
    }
    Ok(())
}
