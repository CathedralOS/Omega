//! Read and update an admitted fragmented scalar in its original containers.
//! A container is accessed as exact bytes, so unaligned and odd-sized storage
//! uses the same operation as naturally aligned storage. Writes preserve every
//! sibling bit by masked RMW; their physical reads and writes remain visible.
//! The envelope is only a bounds summary, never a contiguous scalar address.

use super::{
    Builder, LegalizedScalarInstruction, PlaceId, SelectedInstructionKind, VirtualRegisterId,
    invalid, provenance, transport_register,
};
use crate::SelectedInstructionError;
use legalized_operations::LegalizedRuntimeIndexOperand;
use selected_instructions::{LocalStorageSlotId, SelectedMemoryAccessRole};
use semantic_vocabulary::{
    AuthoredBitFragment, AuthoredBitFragments, IntegerValue, ScalarType, ValueId,
};

/// The same byte codec serves semantic storage and compiler-owned scratch.
/// A place anchors register provenance in both cases, but only Place accesses
/// describe that place's bytes. Scratch must retain its separate frame identity
/// so alias analysis cannot forward a program value into an internal counter.
#[derive(Clone, Copy)]
pub(super) enum StorageAccess<'indices> {
    Place(&'indices [LegalizedRuntimeIndexOperand]),
    Local(LocalStorageSlotId),
    Backing,
}

impl StorageAccess<'_> {
    pub(super) fn footprint(
        self,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        offset: u32,
        width: u32,
        written: super::runtime_address::Written,
    ) -> Result<(), SelectedInstructionError> {
        match self {
            Self::Place(indices) => super::runtime_address::footprint(
                builder, row, place, offset, width, indices, written,
            ),
            Self::Local(slot) => super::memory(
                builder,
                row,
                place,
                offset,
                width,
                match written {
                    super::runtime_address::Written::None => {
                        SelectedMemoryAccessRole::ReadLocal { slot }
                    }
                    _ => SelectedMemoryAccessRole::WriteLocal { slot },
                },
            ),
            Self::Backing => super::memory(
                builder,
                row,
                place,
                offset,
                width,
                match written {
                    super::runtime_address::Written::None => SelectedMemoryAccessRole::ReadBacking,
                    _ => SelectedMemoryAccessRole::WriteBacking,
                },
            ),
        }
    }
}

pub(super) fn constant(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    value: u64,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let output = transport_register(builder, place, 0)?;
    builder.emit_materialize_i64(
        IntegerValue::Unsigned(u128::from(value)),
        output,
        provenance(row),
    )?;
    Ok(output)
}

pub(super) fn binary(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    kind: SelectedInstructionKind,
    left: VirtualRegisterId,
    right: VirtualRegisterId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let constraint = match kind {
        SelectedInstructionKind::WrappingAddI64 => builder.constraints.keys.add_i64,
        SelectedInstructionKind::WrappingMultiplyI64 => builder.constraints.keys.multiply_i64,
        SelectedInstructionKind::WrappingSubtractI64
        | SelectedInstructionKind::BitwiseAndI64
        | SelectedInstructionKind::BitwiseOrI64
        | SelectedInstructionKind::BitwiseXorI64 => builder.constraints.keys.subtract_i64,
        _ => {
            return Err(SelectedInstructionError::custody_because(
                "binary register helper requires a supported arithmetic or bitwise operation",
            ));
        }
    };
    let output = transport_register(builder, place, 0)?;
    builder.emit(kind, constraint, &[left, right, output], provenance(row))?;
    Ok(output)
}

pub(super) fn shift(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    input: VirtualRegisterId,
    count: u64,
    left: bool,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    if count == 0 {
        return Ok(input);
    }
    if count >= 64 {
        return Err(invalid());
    }
    let count = constant(builder, row, place, count)?;
    let output = transport_register(builder, place, 0)?;
    builder.emit(
        if left {
            SelectedInstructionKind::WrappingShiftLeftI64
        } else {
            SelectedInstructionKind::WrappingShiftRightU64
        },
        builder.constraints.keys.shift_i64,
        &[input, count, output],
        provenance(row),
    )?;
    Ok(output)
}

pub(super) fn mask(width: u64) -> Result<u64, SelectedInstructionError> {
    match width {
        1..=63 => Ok((1_u64 << width) - 1),
        64 => Ok(u64::MAX),
        _ => Err(invalid()),
    }
}

fn container_width(fragment: &AuthoredBitFragment) -> Result<u32, SelectedInstructionError> {
    if !(8..=64).contains(&fragment.container_width_bits)
        || !fragment.container_width_bits.is_multiple_of(8)
        || fragment
            .destination_lsb
            .checked_add(fragment.width)
            .is_none_or(|end| end > fragment.container_width_bits)
        || fragment
            .source_lsb
            .checked_add(fragment.width)
            .is_none_or(|end| end > 64)
    {
        return Err(invalid());
    }
    u32::try_from(fragment.container_width_bits / 8).map_err(|_| invalid())
}

fn prefix_offset(
    envelope_offset: u32,
    fragments: &AuthoredBitFragments,
) -> Result<u32, SelectedInstructionError> {
    let (first, _) = fragments.byte_envelope().ok_or_else(invalid)?;
    envelope_offset
        .checked_sub(u32::try_from(first).map_err(|_| invalid())?)
        .ok_or_else(invalid)
}

fn container_offset(
    prefix: u32,
    fragment: &AuthoredBitFragment,
) -> Result<u32, SelectedInstructionError> {
    prefix
        .checked_add(u32::try_from(fragment.container_byte_offset).map_err(|_| invalid())?)
        .ok_or_else(invalid)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn load_container(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    pointer: VirtualRegisterId,
    offset: u32,
    width: u32,
    storage: StorageAccess<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    if width > 8 {
        return Err(SelectedInstructionError::custody_because(
            "scalar container exceeds its register carrier",
        ));
    }
    let mut container = constant(builder, row, place, 0)?;
    for position in 0..width {
        let byte_offset = offset.checked_add(position).ok_or_else(invalid)?;
        let (address, displacement) =
            super::memory_pointer(builder, row, place, pointer, byte_offset, 1)?;
        let byte = transport_register(builder, place, byte_offset)?;
        storage.footprint(
            builder,
            row,
            place,
            byte_offset,
            1,
            super::runtime_address::Written::None,
        )?;
        builder.emit(
            SelectedInstructionKind::Load8 {
                byte_offset: displacement,
            },
            builder.constraints.keys.load8.ok_or_else(invalid)?,
            &[address, byte],
            provenance(row),
        )?;
        let byte = shift(builder, row, place, byte, u64::from(position) * 8, true)?;
        container = binary(
            builder,
            row,
            place,
            SelectedInstructionKind::BitwiseOrI64,
            container,
            byte,
        )?;
    }
    Ok(container)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn read(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    pointer: VirtualRegisterId,
    envelope_offset: u32,
    fragments: &AuthoredBitFragments,
    indices: &[LegalizedRuntimeIndexOperand],
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let definition = row.result.ok_or_else(invalid)?;
    if !matches!(
        definition.scalar_type,
        ScalarType::Boolean | ScalarType::Integer(_)
    ) {
        return Err(invalid());
    }
    let value = read_register(
        builder,
        row,
        place,
        pointer,
        envelope_offset,
        fragments,
        StorageAccess::Place(indices),
    )?;
    let output = builder.register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
    builder.emit(
        crate::selection::scalar_call_abi::integer_carrier_normalization(definition.scalar_type),
        builder.constraints.keys.copy_i64,
        &[value, output],
        provenance(row),
    )?;
    Ok(output)
}

/// Gather the same admitted fragments for a scalar result or an owned image.
/// The latter transfers bits directly into aggregate storage, without an SSA
/// scalar definition or a fabricated logical source value.
#[allow(clippy::too_many_arguments)]
pub(super) fn read_register(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    pointer: VirtualRegisterId,
    envelope_offset: u32,
    fragments: &AuthoredBitFragments,
    storage: StorageAccess<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let prefix = prefix_offset(envelope_offset, fragments)?;
    let mut value = constant(builder, row, place, 0)?;
    for fragment in fragments.fragments() {
        let width = container_width(fragment)?;
        let offset = container_offset(prefix, fragment)?;
        let container = load_container(builder, row, place, pointer, offset, width, storage)?;
        let payload = shift(
            builder,
            row,
            place,
            container,
            fragment.destination_lsb,
            false,
        )?;
        let source_mask = constant(builder, row, place, mask(fragment.width)?)?;
        let payload = binary(
            builder,
            row,
            place,
            SelectedInstructionKind::BitwiseAndI64,
            payload,
            source_mask,
        )?;
        let payload = shift(builder, row, place, payload, fragment.source_lsb, true)?;
        value = binary(
            builder,
            row,
            place,
            SelectedInstructionKind::BitwiseOrI64,
            value,
            payload,
        )?;
    }
    Ok(value)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn write(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    pointer: VirtualRegisterId,
    envelope_offset: u32,
    fragments: &AuthoredBitFragments,
    indices: &[LegalizedRuntimeIndexOperand],
    value: ValueId,
) -> Result<(), SelectedInstructionError> {
    let (_, value_register, _, scalar) = builder.resolve(value).ok_or_else(invalid)?;
    if !matches!(scalar, ScalarType::Boolean | ScalarType::Integer(_)) {
        return Err(invalid());
    }
    write_register(
        builder,
        row,
        place,
        pointer,
        envelope_offset,
        fragments,
        StorageAccess::Place(indices),
        value_register,
        Some(value),
    )
}

/// Store transported scalar bits without inventing an SSA value. Constructors
/// and whole-value decoders own structural writes; ordinary scalar stores keep
/// the source value and runtime-index proof in their footprint.
#[allow(clippy::too_many_arguments)]
pub(super) fn write_register(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    pointer: VirtualRegisterId,
    envelope_offset: u32,
    fragments: &AuthoredBitFragments,
    storage: StorageAccess<'_>,
    value_register: VirtualRegisterId,
    value: Option<ValueId>,
) -> Result<(), SelectedInstructionError> {
    let prefix = prefix_offset(envelope_offset, fragments)?;
    for fragment in fragments.fragments() {
        let width = container_width(fragment)?;
        let offset = container_offset(prefix, fragment)?;
        let container = load_container(builder, row, place, pointer, offset, width, storage)?;
        let source_mask_bits = mask(fragment.width)?;
        let destination_mask = source_mask_bits
            .checked_shl(u32::try_from(fragment.destination_lsb).map_err(|_| invalid())?)
            .ok_or_else(invalid)?;
        let keep = constant(builder, row, place, !destination_mask)?;
        let preserved = binary(
            builder,
            row,
            place,
            SelectedInstructionKind::BitwiseAndI64,
            container,
            keep,
        )?;
        let payload = shift(
            builder,
            row,
            place,
            value_register,
            fragment.source_lsb,
            false,
        )?;
        let source_mask = constant(builder, row, place, source_mask_bits)?;
        let payload = binary(
            builder,
            row,
            place,
            SelectedInstructionKind::BitwiseAndI64,
            payload,
            source_mask,
        )?;
        let payload = shift(builder, row, place, payload, fragment.destination_lsb, true)?;
        let merged = binary(
            builder,
            row,
            place,
            SelectedInstructionKind::BitwiseOrI64,
            preserved,
            payload,
        )?;
        store_container(
            builder, row, place, pointer, offset, width, storage, merged, value,
        )?;
    }
    Ok(())
}

/// Write exact bytes using the same owner and indexing as container loads.
#[allow(clippy::too_many_arguments)]
pub(super) fn store_container(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    pointer: VirtualRegisterId,
    offset: u32,
    width: u32,
    storage: StorageAccess<'_>,
    bits: VirtualRegisterId,
    value: Option<ValueId>,
) -> Result<(), SelectedInstructionError> {
    if width > 8 {
        return Err(SelectedInstructionError::custody_because(
            "scalar container exceeds its register carrier",
        ));
    }
    for position in 0..width {
        let byte_offset = offset.checked_add(position).ok_or_else(invalid)?;
        let payload = shift(builder, row, place, bits, u64::from(position) * 8, false)?;
        let (address, displacement) =
            super::memory_pointer(builder, row, place, pointer, byte_offset, 1)?;
        storage.footprint(
            builder,
            row,
            place,
            byte_offset,
            1,
            value.map_or(
                super::runtime_address::Written::Structural,
                super::runtime_address::Written::Scalar,
            ),
        )?;
        let mut operation_provenance = provenance(row);
        if let Some(value) = value
            && !operation_provenance.values.contains(&value)
        {
            operation_provenance.values.push(value);
        }
        builder.emit_store(displacement, 1, address, payload, operation_provenance)?;
    }
    Ok(())
}
