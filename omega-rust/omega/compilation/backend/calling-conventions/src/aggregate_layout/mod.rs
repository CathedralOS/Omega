use crate::ValueShape;

/// One field placed in declaration order within an aggregate region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackedFieldLayout {
    pub shape: ValueShape,
    pub byte_offset: u32,
}

/// One case payload overlaid at a conventional sum's shared payload base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalSumCaseLayout {
    pub fields: Vec<PackedFieldLayout>,
}

/// Canonical target layout for a closed sum or mixed record/sum.
///
/// The signed 32-bit case tag is always at byte zero. Common fields follow
/// the tag in declaration order. Every case payload begins at one shared,
/// maximally aligned base and overlays every other case payload. The complete
/// value covers the largest payload and is padded to its strictest alignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalSumLayout {
    pub shape: ValueShape,
    pub tag_byte_offset: u32,
    pub tag_shape: ValueShape,
    pub common_fields: Vec<PackedFieldLayout>,
    pub payload_byte_offset: u32,
    pub cases: Vec<ConventionalSumCaseLayout>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateLayoutError {
    EmptyCaseSet,
    ZeroAlignment,
    SizeOverflow,
}

/// Derive the one conventional tag-prefixed overlay used by Omega native
/// targets. The caller supplies already-derived field shapes; semantic field
/// and case identities remain in the source structural declaration.
pub fn evaluate_conventional_sum_layout(
    common_field_shapes: &[ValueShape],
    case_field_shapes: &[Vec<ValueShape>],
) -> Result<ConventionalSumLayout, AggregateLayoutError> {
    let tag = layout_plans::StorageGeometry {
        byte_size: runtime_abi::CASE_TAG_BYTE_SIZE,
        byte_alignment: runtime_abi::CASE_TAG_BYTE_ALIGNMENT,
    };
    let geometry = layout_plans::conventional_sum_geometry(
        tag,
        common_field_shapes.iter().map(storage_geometry),
        case_field_shapes
            .iter()
            .map(|fields| fields.iter().map(storage_geometry)),
    )
    .map_err(|error| match error {
        layout_plans::AggregateGeometryError::EmptyCaseSet => AggregateLayoutError::EmptyCaseSet,
        layout_plans::AggregateGeometryError::ZeroAlignment => AggregateLayoutError::ZeroAlignment,
        layout_plans::AggregateGeometryError::SizeOverflow => AggregateLayoutError::SizeOverflow,
    })?;
    let common_fields = native_fields(common_field_shapes, geometry.common_field_offsets)?;
    let cases = case_field_shapes
        .iter()
        .zip(geometry.case_field_offsets)
        .map(|(shapes, offsets)| {
            Ok(ConventionalSumCaseLayout {
                fields: native_fields(shapes, offsets)?,
            })
        })
        .collect::<Result<Vec<_>, AggregateLayoutError>>()?;
    Ok(ConventionalSumLayout {
        shape: native_geometry(geometry.storage)?,
        tag_byte_offset: 0,
        tag_shape: native_geometry(tag)?,
        common_fields,
        payload_byte_offset: u32::try_from(geometry.payload_byte_offset)
            .map_err(|_| AggregateLayoutError::SizeOverflow)?,
        cases,
    })
}

fn storage_geometry(shape: &ValueShape) -> layout_plans::StorageGeometry {
    layout_plans::StorageGeometry {
        byte_size: u64::from(shape.byte_size),
        byte_alignment: u64::from(shape.alignment),
    }
}

fn native_geometry(
    geometry: layout_plans::StorageGeometry,
) -> Result<ValueShape, AggregateLayoutError> {
    Ok(ValueShape::integer(
        u32::try_from(geometry.byte_size).map_err(|_| AggregateLayoutError::SizeOverflow)?,
        u16::try_from(geometry.byte_alignment).map_err(|_| AggregateLayoutError::SizeOverflow)?,
    ))
}

fn native_fields(
    shapes: &[ValueShape],
    offsets: Vec<u64>,
) -> Result<Vec<PackedFieldLayout>, AggregateLayoutError> {
    shapes
        .iter()
        .zip(offsets)
        .map(|(shape, offset)| {
            Ok(PackedFieldLayout {
                // Preserve float/integer ABI classification; geometry owns only offsets.
                shape: *shape,
                byte_offset: u32::try_from(offset)
                    .map_err(|_| AggregateLayoutError::SizeOverflow)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        AggregateLayoutError, PackedFieldLayout, ValueShape, evaluate_conventional_sum_layout,
    };

    #[test]
    fn payloadless_sum_is_one_i32_tag() {
        let layout = evaluate_conventional_sum_layout(&[], &[vec![], vec![]])
            .expect("payloadless sum layout");

        assert_eq!(layout.shape, ValueShape::integer(4, 4));
        assert_eq!(layout.tag_byte_offset, 0);
        assert_eq!(layout.payload_byte_offset, 4);
        assert_eq!(layout.cases.len(), 2);
    }

    #[test]
    fn byte_read_overlays_its_optional_i32_payload_after_the_tag() {
        let layout =
            evaluate_conventional_sum_layout(&[], &[vec![], vec![ValueShape::integer(4, 4)]])
                .expect("ByteRead layout");

        assert_eq!(layout.shape, ValueShape::integer(8, 4));
        assert_eq!(layout.payload_byte_offset, 4);
        assert!(layout.cases[0].fields.is_empty());
        assert_eq!(
            layout.cases[1].fields,
            vec![PackedFieldLayout {
                shape: ValueShape::integer(4, 4),
                byte_offset: 4,
            }]
        );
    }

    #[test]
    fn mixed_layout_aligns_one_shared_payload_base() {
        let layout = evaluate_conventional_sum_layout(
            &[ValueShape::integer(1, 1)],
            &[
                vec![ValueShape::integer(2, 2)],
                vec![ValueShape::integer(8, 8)],
            ],
        )
        .expect("mixed layout");

        assert_eq!(layout.common_fields[0].byte_offset, 4);
        assert_eq!(layout.payload_byte_offset, 8);
        assert_eq!(layout.cases[0].fields[0].byte_offset, 8);
        assert_eq!(layout.cases[1].fields[0].byte_offset, 8);
        assert_eq!(layout.shape, ValueShape::integer(16, 8));
    }

    #[test]
    fn large_common_fields_keep_full_payload_offsets() {
        let layout = evaluate_conventional_sum_layout(
            &[
                ValueShape::integer(32_768, 8),
                ValueShape::integer(32_768, 8),
            ],
            &[vec![ValueShape::integer(8, 8)]],
        )
        .expect("large fields are ordinary storage, not register fragments");
        assert_eq!(layout.common_fields[1].byte_offset, 32_776);
        assert_eq!(layout.payload_byte_offset, 65_544);
        assert_eq!(layout.cases[0].fields[0].byte_offset, 65_544);
        assert_eq!(layout.shape.byte_size, 65_552);
    }

    #[test]
    fn empty_case_set_and_oversized_payload_fail_closed() {
        assert_eq!(
            evaluate_conventional_sum_layout(&[], &[]),
            Err(AggregateLayoutError::EmptyCaseSet)
        );
        assert_eq!(
            evaluate_conventional_sum_layout(
                &[],
                &[vec![
                    ValueShape::integer(u32::MAX, 1),
                    ValueShape::integer(1, 1)
                ]],
            ),
            Err(AggregateLayoutError::SizeOverflow)
        );
    }
}
