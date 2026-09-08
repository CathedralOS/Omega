//! Independently join a selected terminal to the final boundary/source return pair.
use super::*;
use legalized_operations::{
    LegalizedScalarBlock, LegalizedScalarReturnValue, LegalizedScalarTerminator,
};
use selected_instructions::{SelectedBoundarySettlement, SelectedBoundarySettlementPayload};

pub(super) fn validate(
    block: &LegalizedScalarBlock,
    replay: &mut Replay<'_>,
) -> Result<bool, SelectedInstructionError> {
    let Some(row) = block.instructions.last() else {
        return Ok(false);
    };
    let LegalizedScalarInstructionKind::HostedExitProcessI32 { boundary, source } = row.kind else {
        return Ok(false);
    };
    let LegalizedScalarTerminator::Return(returned) = &block.terminator else {
        return Err(replay.invalid());
    };
    let SelectedTerminator::HostedExitProcess {
        instruction,
        nominal_return_edge,
    } = &replay.block.terminator
    else {
        return Err(replay.invalid());
    };
    let (_, input, _, scalar_type) = replay.resolve(source).ok_or_else(|| replay.invalid())?;
    if returned.value != LegalizedScalarReturnValue::Unit
        || !matches!(returned.ownership.as_slice(), [optimization_unit::OwnershipEvent::Cleanup(actions)] if actions.is_empty())
        || row.result.is_some()
        || !matches!(row.ownership.as_slice(), [optimization_unit::OwnershipEvent::ClaimCompletion(claims)] if claims.is_empty())
        || !matches!(scalar_type, ScalarType::Integer(integer) if integer.bits() == 32 && integer.sign() == IntegerSign::Signed)
        || *nominal_return_edge != returned.edge
        || instruction.id.0 as usize != replay.instruction_cursor
        || instruction.kind != SelectedInstructionKind::HostedExitProcessI32
        || Some(instruction.constraint) != replay.constraints.keys.hosted_exit_process_i32
        || instruction
            .operands
            .iter()
            .map(|operand| operand.virtual_register)
            .ne([input])
        || instruction.provenance
            != (SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![source],
                fuel: row.fuel.clone(),
                ..Default::default()
            })
    {
        return Err(replay.invalid());
    }
    replay
        .transport
        .settlements
        .push(SelectedBoundarySettlement {
            block: replay.block.id,
            instruction_index: u32::try_from(replay.block_cursor).map_err(|_| replay.invalid())?,
            settlement: SelectedBoundarySettlementPayload::HostedExitProcessI32 {
                operation: row.operation,
                boundary,
                source,
            },
        });
    Ok(true)
}
