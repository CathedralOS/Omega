//! Transfer an admitted exact extent between addressable storage homes.
//! Permission, projection and ABI owners supply the two endpoints. This operation
//! preserves their complete memory footprints without expanding a large home into
//! a register and load/store pair for every eight bytes.
use super::runtime_address::{Written, footprint_at_origin};
use super::{Builder, SelectedInstructionKind, SelectedInstructionProvenance, transport_register};
use crate::SelectedInstructionError;
use crate::selection::extent_copy;
use crate::selection::scalar_call_abi::load_transfer;
use selected_instructions::{SelectedMemoryAccessOrigin, VirtualRegisterId};
use semantic_vocabulary::{IntegerValue, PlaceId};

/// An already projected pointer and its remaining static offset. Runtime
/// selectors stay attached to the same exact source/destination extent.
pub(super) struct CopyEnd<'a> {
    pub(super) place: PlaceId,
    pub(super) pointer: VirtualRegisterId,
    pub(super) byte_offset: u32,
    pub(super) elements: &'a [legalized_operations::LegalizedRuntimeIndexOperand],
}

pub(super) fn copy(
    builder: &mut Builder<'_>,
    origin: SelectedMemoryAccessOrigin,
    provenance: SelectedInstructionProvenance,
    from: CopyEnd<'_>,
    to: CopyEnd<'_>,
    byte_size: u32,
) -> Result<(), SelectedInstructionError> {
    // Keep the bounded inline transport for small homes. Larger extents use
    // the existing independently validated byte loop, with the exact constant
    // count and both complete access rosters. The choice changes neither copy
    // permission nor value/descriptor custody established by the caller.
    if byte_size > extent_copy::INLINE_MAX_BYTES {
        let source = address(builder, &from, provenance.clone())?;
        let destination = address(builder, &to, provenance.clone())?;
        let count = transport_register(builder, to.place, 0)?;
        builder.emit_materialize_i64(
            IntegerValue::Unsigned(u128::from(byte_size)),
            count,
            provenance.clone(),
        )?;
        let key = builder.constraints.keys.copy_bytes.ok_or_else(|| {
            SelectedInstructionError::custody_because(
                "exact extent copy has no admitted byte-copy form",
            )
        })?;
        let instruction = selected_instructions::SelectedInstructionId(
            builder
                .instructions
                .len()
                .try_into()
                .map_err(|_| SelectedInstructionError::custody())?,
        );
        let operands = extent_copy::bulk_operands(
            instruction,
            [source, destination, count],
            key,
            builder.catalog,
            &mut builder.registers,
        )?;
        access(builder, origin, &from, 0, byte_size, Written::None)?;
        access(builder, origin, &to, 0, byte_size, Written::Structural)?;
        return builder.emit(
            SelectedInstructionKind::CopyBytes,
            key,
            &operands,
            provenance,
        );
    }
    for (cursor, width) in extent_copy::chunks(byte_size) {
        let offset = from
            .byte_offset
            .checked_add(cursor)
            .ok_or_else(SelectedInstructionError::custody)?;
        let (input, displacement) = super::displaced_memory_pointer(
            builder,
            from.place,
            from.pointer,
            offset,
            u32::from(width),
            provenance.clone(),
        )?;
        let value = transport_register(builder, from.place, offset)?;
        access(
            builder,
            origin,
            &from,
            cursor,
            u32::from(width),
            Written::None,
        )?;
        let (kind, key) = load_transfer(&builder.constraints.keys, u32::from(width), displacement)
            .ok_or_else(SelectedInstructionError::custody)?;
        builder.emit(
            kind,
            key.ok_or_else(SelectedInstructionError::custody)?,
            &[input, value],
            provenance.clone(),
        )?;
        let offset = to
            .byte_offset
            .checked_add(cursor)
            .ok_or_else(SelectedInstructionError::custody)?;
        let (output, displacement) = super::displaced_memory_pointer(
            builder,
            to.place,
            to.pointer,
            offset,
            u32::from(width),
            provenance.clone(),
        )?;
        access(
            builder,
            origin,
            &to,
            cursor,
            u32::from(width),
            Written::Structural,
        )?;
        builder.emit_store(displacement, width, output, value, provenance.clone())?;
    }
    Ok(())
}

fn address(
    builder: &mut Builder<'_>,
    end: &CopyEnd<'_>,
    provenance: SelectedInstructionProvenance,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    if end.byte_offset == 0 {
        return Ok(end.pointer);
    }
    let address = transport_register(builder, end.place, end.byte_offset)?;
    builder.emit_address_offset(end.byte_offset, end.pointer, address, provenance)?;
    Ok(address)
}

fn access(
    builder: &mut Builder<'_>,
    origin: SelectedMemoryAccessOrigin,
    end: &CopyEnd<'_>,
    cursor: u32,
    byte_size: u32,
    written: Written,
) -> Result<(), SelectedInstructionError> {
    let offset = end
        .byte_offset
        .checked_add(cursor)
        .ok_or_else(SelectedInstructionError::custody)?;
    footprint_at_origin(
        builder,
        origin,
        end.place,
        offset,
        byte_size,
        end.elements,
        written,
    )
}
