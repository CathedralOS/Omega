//! Select exact aggregate fragment accesses without widening storage.
//! Packed expansions own an instruction-local temporary, never a source value.
//! Callers stage an encodable pointer before recording the exact semantic
//! place/offset/width access, then pass its native displacement here. Address
//! staging must precede the access record so its instruction remains the load
//! or store. Physical constraints keep scratch and early-written outputs
//! disjoint from live inputs.
use super::integer_arithmetic::instruction_scratch;
use super::{Builder, IntegerSign, SelectedInstructionKind, VirtualRegisterId};
use crate::SelectedInstructionError;
use crate::selection::scalar_call_abi::load_transfer;

pub(super) fn load(
    builder: &mut Builder<'_>,
    pointer: VirtualRegisterId,
    output: VirtualRegisterId,
    byte_offset: u32,
    byte_size: u32,
) -> Result<(), SelectedInstructionError> {
    let mut operands = vec![pointer, output];
    let (kind, key) = match load_transfer(&builder.constraints.keys, byte_size, byte_offset) {
        Some(pair) => pair,
        None => {
            let width = selected_instructions::PackedByteWidth::from_byte_size(
                byte_size
                    .try_into()
                    .map_err(|_| SelectedInstructionError::custody())?,
            )
            .ok_or_else(|| SelectedInstructionError::custody())?;
            operands.push(scratch(builder)?);
            (
                SelectedInstructionKind::LoadPacked { byte_offset, width },
                builder.constraints.keys.load_packed,
            )
        }
    };
    builder.emit(
        kind,
        key.ok_or_else(|| SelectedInstructionError::custody())?,
        &operands,
        Default::default(),
    )
}

pub(super) fn store(
    builder: &mut Builder<'_>,
    pointer: VirtualRegisterId,
    value: VirtualRegisterId,
    byte_offset: u32,
    byte_size: u32,
) -> Result<(), SelectedInstructionError> {
    let byte_size = u8::try_from(byte_size).map_err(|_| SelectedInstructionError::custody())?;
    let mut operands = vec![pointer, value];
    let (kind, key) =
        if let Some(width) = selected_instructions::PackedByteWidth::from_byte_size(byte_size) {
            operands.push(scratch(builder)?);
            (
                SelectedInstructionKind::StorePacked { byte_offset, width },
                builder.constraints.keys.store_packed,
            )
        } else if matches!(byte_size, 1 | 2 | 4 | 8) {
            (
                SelectedInstructionKind::Store {
                    byte_offset,
                    byte_size,
                },
                builder.constraints.keys.store,
            )
        } else {
            return Err(SelectedInstructionError::custody());
        };
    builder.emit(
        kind,
        key.ok_or_else(|| SelectedInstructionError::custody())?,
        &operands,
        Default::default(),
    )
}

fn scratch(builder: &mut Builder<'_>) -> Result<VirtualRegisterId, SelectedInstructionError> {
    instruction_scratch(builder, 2, IntegerSign::Unsigned)
}
