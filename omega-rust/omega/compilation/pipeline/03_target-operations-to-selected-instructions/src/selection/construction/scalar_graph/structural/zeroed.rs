//! Establish a whole zero-constructible structural value in one home. A
//! zero-constructible home is exactly all-zero bytes, so either spelling
//! below establishes it: a small home stores one zero register into every
//! scalar-leaf range; a large home, or a small one with a leaf inside a case
//! payload member, is written by one `ZeroBytes` over its whole byte extent,
//! padding and unused payload bytes included.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, SelectedInstructionKind,
    SelectedMemoryAccessRole, memory, transport_register,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::local_storage;
use crate::selection::construction::scalar_graph::structural::provenance;
use selected_instructions::VirtualRegisterId;
use semantic_vocabulary::{IntegerValue, PlaceId};

/// The largest home established by per-leaf stores. Up to one 4 KiB page
/// every leaf store addresses the home from its base pointer with an
/// encodable displacement on both ISAs (AArch64's unsigned scaled
/// immediate reaches byte 4095), and the straight-line sequence stays
/// bounded by the home's size. Above it the per-leaf sequence grows with
/// the leaf count — the product lexer's home has about 230k leaves, about
/// 1 MB of stores per establishment — and its stores start paying address
/// arithmetic, so one fixed loop over the whole extent replaces it.
const PER_LEAF_ZEROING_MAX_BYTES: u32 = 4096;

pub(super) fn establish(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let (established, shape) = crate::selection::zeroed_structural_input::home(source, row)
        .ok_or(SelectedInstructionError::custody())?;
    // Small homes keep their exact per-leaf store sequence; only they expand
    // leaves, so the leaf budget never bounds a whole-extent establishment.
    // `home` already admitted the value as zero-constructible, so a small home
    // whose leaves the store grammar cannot address directly (a member of a
    // case payload) takes the whole-extent fill like a large one.
    let leaf_stores = (shape.byte_size <= PER_LEAF_ZEROING_MAX_BYTES)
        .then(|| crate::selection::zeroed_structural_input::storage(source, established))
        .flatten();
    let slot = builder.transport.push_structural_local_slot(
        row.operation,
        established.place,
        shape.byte_size.max(1),
        shape.alignment,
    );
    // The address instruction charges construction once; the payload stores
    // carry the row's provenance without introducing additional semantic work.
    // Empty values still lend an address. Match record construction's anchor
    // without adding semantic payload or changing the owned argument ABI.
    let pointer = local_storage::address(builder, row, slot, 0, shape.byte_size.max(1), true)?;
    if shape.byte_size != 0 {
        match leaf_stores {
            Some(stores) => store_each_leaf(builder, row, established.place, pointer, &stores)?,
            None => zero_whole_home(builder, row, established.place, pointer, shape.byte_size)?,
        }
    }
    // Publish the whole home to edge transport only after it is established.
    builder
        .transport
        .pointers
        .push((established.place, pointer));
    Ok(())
}

/// Store one zero register into every scalar-leaf range of the home.
fn store_each_leaf(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    pointer: VirtualRegisterId,
    stores: &[(u32, u8)],
) -> Result<(), SelectedInstructionError> {
    // Every scalar zero spelling is the all-zero bit pattern, so one
    // materialization serves every leaf width.
    let zero = transport_register(builder, place, 0)?;
    builder.emit_materialize_i64(IntegerValue::Unsigned(0), zero, provenance(row))?;
    for &(offset, width) in stores {
        let (pointer, displacement) =
            super::memory_pointer(builder, row, place, pointer, offset, u32::from(width))?;
        memory(
            builder,
            row,
            place,
            offset,
            u32::from(width),
            SelectedMemoryAccessRole::WritePlace,
        )?;
        builder.emit_store(displacement, width, pointer, zero, provenance(row))?;
    }
    Ok(())
}

/// Write zero over the home's whole byte extent `[0, byte_size)` from its
/// base pointer. The count is a materialized constant equal to the
/// `WritePlace` row's byte count, so the row is the fill's exact reach.
fn zero_whole_home(
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
