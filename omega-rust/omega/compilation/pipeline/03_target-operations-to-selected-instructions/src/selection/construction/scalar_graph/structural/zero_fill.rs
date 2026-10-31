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

/// The largest home expanded into per-leaf or byte-chunk stores. Up to one 4 KiB page
/// every leaf store addresses the home from its base pointer with an
/// encodable displacement on both ISAs (AArch64's unsigned scaled
/// immediate reaches byte 4095), and the straight-line sequence stays
/// bounded by the home's size. Above it the per-leaf sequence grows with
/// the leaf count — the product lexer's home has about 230k leaves, about
/// 1 MB of stores per establishment — and its stores start paying address
/// arithmetic, so one fixed loop over the whole extent replaces it.
pub(super) const INLINE_HOME_MAX_BYTES: u32 = 4096;

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
