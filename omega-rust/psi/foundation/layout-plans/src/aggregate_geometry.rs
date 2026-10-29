//! Numeric packing shared by layout observations and native ABI placement.
//! Callers supply carrier geometry; this owner knows no source or ABI classes.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageGeometry {
    pub byte_size: u64,
    pub byte_alignment: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalSumGeometry {
    pub storage: StorageGeometry,
    pub common_field_offsets: Vec<u64>,
    pub payload_byte_offset: u64,
    pub case_field_offsets: Vec<Vec<u64>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateGeometryError {
    EmptyCaseSet,
    ZeroAlignment,
    SizeOverflow,
}

/// Place the supplied tag at zero, followed by common fields and an overlaid
/// payload region aligned for every case. Field order is supplied order. Both
/// field-sequence ends and the final extent include their required padding.
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
