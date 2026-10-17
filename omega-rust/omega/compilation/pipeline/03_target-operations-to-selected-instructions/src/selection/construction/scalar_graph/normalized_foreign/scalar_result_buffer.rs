//! A scalar's semantic result is independent of its admitted ABI result pointer.
//! The call owns a separate result slot; floating controls retain their own
//! slot. Rematerialize the address after the call so its volatile ABI pointer
//! need not survive counterparty clobbers. Loads retain the exact scalar width.
use super::super::{
    Builder, SelectedInstructionKind, SelectedInstructionProvenance, VirtualRegisterId,
};
use crate::SelectedInstructionError;
use calling_conventions::ValueLocation;
use legalized_operations::{LegalizedNormalizedForeignCall, LegalizedScalarInstruction};
use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId, SelectedLocalStorageSlot};

pub(super) fn is_indirect(call: &LegalizedNormalizedForeignCall) -> bool {
    call.result.scalar_home().is_some()
        && call
            .binding
            .boundary_entry_plan
            .call
            .result
            .as_ref()
            .is_some_and(|placement| {
                matches!(
                    placement.locations.as_slice(),
                    [ValueLocation::Indirect { .. }]
                )
            })
}

fn address(
    operation: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let result = operation
        .result
        .ok_or_else(SelectedInstructionError::custody)?;
    let address = super::super::scalar_stack::address_register(
        builder,
        result.value,
        result.definition_site,
    )?;
    builder.emit_frame_address(
        FrameStorageSlotId::Local(LocalStorageSlotId::BoundaryResult {
            operation: operation.operation,
        }),
        address,
        SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: vec![result.value],
            ..Default::default()
        },
    )?;
    Ok(address)
}

pub(super) fn prepare(
    call: &LegalizedNormalizedForeignCall,
    operation: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let placement = call
        .binding
        .boundary_entry_plan
        .call
        .result
        .as_ref()
        .ok_or_else(SelectedInstructionError::custody)?;
    if !placement.is_complete_scalar_result() {
        return Err(SelectedInstructionError::custody());
    }
    builder
        .transport
        .local_slots
        .push(SelectedLocalStorageSlot {
            id: LocalStorageSlotId::BoundaryResult {
                operation: operation.operation,
            },
            byte_size: placement.shape.byte_size,
            alignment: placement.shape.alignment,
        });
    address(operation, builder)
}

pub(super) fn load(
    call: &LegalizedNormalizedForeignCall,
    operation: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let result = operation
        .result
        .ok_or_else(SelectedInstructionError::custody)?;
    let home = call
        .result
        .scalar_home()
        .ok_or_else(SelectedInstructionError::custody)?;
    let pointer = address(operation, builder)?;
    let output = builder.register(result.value, result.definition_site, result.scalar_type)?;
    let (kind, key) = match home.shape.byte_size {
        1 => (
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            builder.constraints.keys.load8,
        ),
        2 => (
            SelectedInstructionKind::Load16 { byte_offset: 0 },
            builder.constraints.keys.load16,
        ),
        4 => (
            SelectedInstructionKind::Load32 { byte_offset: 0 },
            builder.constraints.keys.load32,
        ),
        8 => (
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            builder.constraints.keys.load64,
        ),
        _ => return Err(SelectedInstructionError::custody()),
    };
    builder.emit(
        kind,
        key.ok_or_else(SelectedInstructionError::custody)?,
        &[pointer, output],
        SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: vec![result.value],
            ..Default::default()
        },
    )?;
    Ok(output)
}
