//! Realize one admitted byte-output occurrence in operation-owned frame storage.
use super::*;
use legalized_operations::LegalizedScalarInstruction;
use selected_instructions::{
    LocalStorageSlotId, SelectedBoundarySettlement, SelectedBoundarySettlementPayload,
    SelectedLocalStorageSlot,
};

pub(super) fn emit(
    row: &LegalizedScalarInstruction,
    block: SelectedBlockId,
    block_start: usize,
    builder: &mut Builder<'_>,
) -> Result<bool, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::LinuxWriteByteI32 { boundary, source } = row.kind else {
        return Ok(false);
    };
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let (_, input, _, scalar_type) = builder.resolve(source).ok_or_else(invalid)?;
    if row.result.is_some()
        || !matches!(row.ownership.as_slice(), [optimization_unit::OwnershipEvent::ClaimCompletion(claims)] if claims.is_empty())
        || !matches!(scalar_type, ScalarType::Integer(integer)
            if integer.bits() == 32 && integer.sign() == IntegerSign::Signed)
    {
        return Err(invalid());
    }
    let slot = LocalStorageSlotId::Boundary {
        operation: row.operation,
    };
    builder
        .transport
        .local_slots
        .push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: 1,
            alignment: 1,
        });
    let instruction_index = builder
        .instructions
        .len()
        .checked_sub(block_start)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(invalid)?;
    builder
        .transport
        .settlements
        .push(SelectedBoundarySettlement {
            block,
            instruction_index,
            settlement: SelectedBoundarySettlementPayload::LinuxWriteByteI32 {
                operation: row.operation,
                boundary,
                source,
            },
        });
    builder.emit(
        SelectedInstructionKind::LinuxWriteByteI32 { slot },
        builder
            .constraints
            .keys
            .linux_write_byte_i32
            .ok_or_else(invalid)?,
        &[input],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![source],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(true)
}
