//! Realize an admitted external completion at its exact terminal edge.
use super::{
    Builder, IntegerSign, ScalarType, SelectedBlockId, SelectedInstructionKind,
    SelectedInstructionProvenance,
};
use crate::SelectedInstructionError;
use legalized_operations::{LegalizedScalarBlock, LegalizedScalarTerminator};
use selected_instructions::{
    SelectedBoundarySettlement, SelectedBoundarySettlementPayload, SelectedTerminator,
};

pub(super) fn build(
    block: &LegalizedScalarBlock,
    block_id: SelectedBlockId,
    start: usize,
    builder: &mut Builder<'_>,
) -> Result<Option<SelectedTerminator>, SelectedInstructionError> {
    let LegalizedScalarTerminator::ExternalCompletion {
        psi_edge,
        boundary,
        kind,
        arguments,
        binding,
        structural_arguments,
        fuel,
        ..
    } = &block.terminator
    else {
        return Ok(None);
    };
    let [argument] = arguments.as_slice() else {
        return Err(SelectedInstructionError::custody());
    };
    let source = argument.source.source_value();
    let (_, input, _, scalar_type) = builder
        .resolve(source)
        .ok_or_else(SelectedInstructionError::custody)?;
    if *kind != semantic_vocabulary::ExternalCompletionKind::ProcessExit
        || binding.boundary != *boundary
        || !matches!(
            binding.execution,
            target_operations::BoundaryExecutionBinding::AdmittedProvider(_)
                | target_operations::BoundaryExecutionBinding::CompilerBuiltin(
                    target_operations::CompilerBuiltinExecution::HostedExitProcessI32
                )
        )
        || !matches!(
            binding.realization,
            target_operations::BoundarySettlementRealization::Builtin(
                target_operations::BoundaryRealization::HostedExitProcessI32(_)
            )
        )
        || !structural_arguments.is_empty()
        || !matches!(scalar_type, ScalarType::Integer(integer) if integer.bits() == 32 && integer.sign() == IntegerSign::Signed)
    {
        return Err(SelectedInstructionError::custody());
    }
    builder
        .transport
        .settlements
        .push(SelectedBoundarySettlement {
            block: block_id,
            instruction_index: u32::try_from(
                builder
                    .instructions
                    .len()
                    .checked_sub(start)
                    .ok_or_else(SelectedInstructionError::custody)?,
            )
            .map_err(|_| SelectedInstructionError::custody())?,
            settlement: SelectedBoundarySettlementPayload::HostedExitProcessI32 {
                edge: *psi_edge,
                execution: binding.execution,
                boundary: *boundary,
                source,
            },
        });
    builder.emit(
        SelectedInstructionKind::HostedExitProcessI32,
        builder
            .constraints
            .keys
            .hosted_exit_process_i32
            .ok_or_else(SelectedInstructionError::custody)?,
        &[input],
        SelectedInstructionProvenance {
            edges: vec![*psi_edge],
            values: vec![source],
            fuel: fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(Some(SelectedTerminator::HostedExitProcess {
        instruction: builder
            .instructions
            .last()
            .cloned()
            .ok_or_else(SelectedInstructionError::custody)?,
        completion_edge: *psi_edge,
    }))
}
