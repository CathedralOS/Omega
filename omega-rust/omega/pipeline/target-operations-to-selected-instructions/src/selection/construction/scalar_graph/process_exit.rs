//! Collapse an admitted final boundary and its nominal source return into native exit.
use super::*;
use legalized_operations::{
    LegalizedScalarBlock, LegalizedScalarReturnValue, LegalizedScalarTerminator,
};
use selected_instructions::{SelectedBoundarySettlement, SelectedBoundarySettlementPayload};

pub(super) fn build(
    block: &LegalizedScalarBlock,
    block_id: SelectedBlockId,
    start: usize,
    builder: &mut Builder<'_>,
) -> Result<Option<SelectedTerminator>, SelectedInstructionError> {
    let Some(row) = block.instructions.last() else {
        return Ok(None);
    };
    let LegalizedScalarInstructionKind::HostedExitProcessI32 { boundary, source } = row.kind else {
        return Ok(None);
    };
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let LegalizedScalarTerminator::Return(returned) = &block.terminator else {
        return Err(invalid());
    };
    let (_, input, _, scalar_type) = builder.resolve(source).ok_or_else(invalid)?;
    if returned.value != LegalizedScalarReturnValue::Unit
        || !matches!(returned.ownership.as_slice(), [optimization_unit::OwnershipEvent::Cleanup(actions)] if actions.is_empty())
        || row.result.is_some()
        || !matches!(row.ownership.as_slice(), [optimization_unit::OwnershipEvent::ClaimCompletion(claims)] if claims.is_empty())
        || !matches!(scalar_type, ScalarType::Integer(integer) if integer.bits() == 32 && integer.sign() == IntegerSign::Signed)
    {
        return Err(invalid());
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
                    .ok_or_else(invalid)?,
            )
            .map_err(|_| invalid())?,
            settlement: SelectedBoundarySettlementPayload::HostedExitProcessI32 {
                operation: row.operation,
                boundary,
                source,
            },
        });
    builder.emit(
        SelectedInstructionKind::HostedExitProcessI32,
        builder
            .constraints
            .keys
            .hosted_exit_process_i32
            .ok_or_else(invalid)?,
        &[input],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![source],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(Some(SelectedTerminator::HostedExitProcess {
        instruction: builder.instructions.last().cloned().ok_or_else(invalid)?,
        nominal_return_edge: returned.edge,
    }))
}
