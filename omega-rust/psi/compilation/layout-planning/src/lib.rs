//! Default integer, record and sum layout realization.
//! Source and native planners request complete geometry before publishing it.
#![forbid(unsafe_code)]
use layout_plans::{AggregateGeometryError, ConventionalSumGeometry, StorageGeometry};
use semantic_vocabulary::ClosedIntegerType;

// Schema reflection and native placement must report the same chosen
// integer storage. These are the current default realizations, not a
// language promise about the alignment of every representation.
pub fn standalone_integer_geometry(integer: ClosedIntegerType) -> StorageGeometry {
    integer_geometry(integer, 8)
}

pub fn field_integer_geometry(integer: ClosedIntegerType) -> StorageGeometry {
    integer_geometry(integer, 16)
}

fn integer_geometry(integer: ClosedIntegerType, alignment_cap: u16) -> StorageGeometry {
    let bytes = integer.bits().div_ceil(8);
    StorageGeometry {
        byte_size: u64::from(bytes),
        byte_alignment: u64::from(bytes.next_power_of_two().min(alignment_cap)),
    }
}

pub fn conventional_sum_geometry(
    tag: StorageGeometry,
    common_fields: impl IntoIterator<Item = StorageGeometry>,
    cases: impl IntoIterator<Item = impl IntoIterator<Item = StorageGeometry>>,
) -> Result<ConventionalSumGeometry, AggregateGeometryError> {
    let cases: Vec<Vec<StorageGeometry>> = cases
        .into_iter()
        .map(|fields| fields.into_iter().collect())
        .collect();
    if cases.is_empty() {
        return Err(AggregateGeometryError::EmptyCaseSet);
    }
    valid_alignment(tag.byte_alignment)?;
    let (common_field_offsets, common) = pack_fields_at(common_fields, tag.byte_size)?;
    let mut payload_alignment = 1;
    for field in cases.iter().flatten() {
        valid_alignment(field.byte_alignment)?;
        payload_alignment = payload_alignment.max(field.byte_alignment);
    }
    let payload_byte_offset = align_up(common.byte_size, payload_alignment)?;
    let mut end = common.byte_size;
    let mut case_field_offsets = Vec::with_capacity(cases.len());
    for fields in cases {
        let (offsets, geometry) = pack_fields_at(fields, payload_byte_offset)?;
        end = end.max(geometry.byte_size);
        case_field_offsets.push(offsets);
    }
    let byte_alignment = tag
        .byte_alignment
        .max(common.byte_alignment)
        .max(payload_alignment);
    Ok(ConventionalSumGeometry {
        tag_byte_offset: 0,
        tag,
        storage: StorageGeometry {
            byte_size: align_up(end, byte_alignment)?,
            byte_alignment,
        },
        common_field_offsets,
        payload_byte_offset,
        case_field_offsets,
    })
}

fn pack_fields_at(
    fields: impl IntoIterator<Item = StorageGeometry>,
    base: u64,
) -> Result<(Vec<u64>, StorageGeometry), AggregateGeometryError> {
    let mut end = base;
    let mut alignment = 1;
    let mut offsets = Vec::new();
    for field in fields {
        valid_alignment(field.byte_alignment)?;
        end = align_up(end, field.byte_alignment)?;
        offsets.push(end);
        end = end
            .checked_add(field.byte_size)
            .ok_or(AggregateGeometryError::SizeOverflow)?;
        alignment = alignment.max(field.byte_alignment);
    }
    Ok((
        offsets,
        StorageGeometry {
            byte_size: align_up(end, alignment)?,
            byte_alignment: alignment,
        },
    ))
}

/// Default records place each complete field at its alignment and round
/// the final extent to the largest field alignment.
pub fn conventional_record_geometry(
    fields: impl IntoIterator<Item = StorageGeometry>,
) -> Result<(Vec<u64>, StorageGeometry), AggregateGeometryError> {
    pack_fields_at(fields, 0)
}

fn valid_alignment(alignment: u64) -> Result<(), AggregateGeometryError> {
    if alignment == 0 {
        Err(AggregateGeometryError::ZeroAlignment)
    } else {
        Ok(())
    }
}

fn align_up(value: u64, alignment: u64) -> Result<u64, AggregateGeometryError> {
    valid_alignment(alignment)?;
    let remainder = value % alignment;
    if remainder == 0 {
        Ok(value)
    } else {
        value
            .checked_add(alignment - remainder)
            .ok_or(AggregateGeometryError::SizeOverflow)
    }
}
