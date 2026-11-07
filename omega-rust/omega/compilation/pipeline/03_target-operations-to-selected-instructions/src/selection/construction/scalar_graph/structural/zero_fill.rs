//! Emit an exact whole-home zero fill after the caller admits its own home.
//! This writes bytes; it grants no value, domain, ownership or pointer facts.
use super::{
    Builder, LegalizedScalarInstruction, SelectedInstructionKind, SelectedMemoryAccessRole, memory,
    transport_register,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::provenance;
use selected_instructions::VirtualRegisterId;
use semantic_vocabulary::{IntegerValue, PlaceId};

pub(super) use crate::selection::extent_copy::INLINE_MAX_BYTES as INLINE_HOME_MAX_BYTES;

/// Write zero over the home's whole byte extent `[0, byte_size)` from its
/// base pointer. The count is a materialized constant equal to the
/// `WritePlace` row's byte count, so the row is the fill's exact reach.
pub(super) fn whole_home(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    pointer: VirtualRegisterId,
    byte_size: u32,
) -> Result<(), SelectedInstructionError> {
    let count = transport_register(builder, place, 0)?;
    builder.emit_materialize_i64(
        IntegerValue::Unsigned(u128::from(byte_size)),
        count,
        provenance(row),
    )?;
    let cursor = super::byte_field_store::scratch(builder, 2)?;
    memory(
        builder,
        row,
        place,
        0,
        byte_size,
        SelectedMemoryAccessRole::WritePlace,
    )?;
    builder.emit(
        SelectedInstructionKind::ZeroBytes,
        builder
            .constraints
            .keys
            .zero_bytes
            .ok_or(SelectedInstructionError::custody())?,
        &[pointer, count, cursor],
        provenance(row),
    )
}
