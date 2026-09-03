use crate::ValueShape;

/// One field placed in declaration order within an aggregate region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackedFieldLayout {
    pub shape: ValueShape,
    pub byte_offset: u16,
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
    pub tag_byte_offset: u16,
    pub tag_shape: ValueShape,
    pub common_fields: Vec<PackedFieldLayout>,
    pub payload_byte_offset: u16,
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
    if case_field_shapes.is_empty() {
        return Err(AggregateLayoutError::EmptyCaseSet);
    }

    let tag_shape = ValueShape::integer(4, 4);
    let (common_fields, common_end, common_alignment) =
        pack_fields_at(common_field_shapes, u32::from(tag_shape.byte_size))?;
    let common_end = common_end.max(u32::from(tag_shape.byte_size));

    let payload_alignment = case_field_shapes
        .iter()
        .flatten()
        .try_fold(1_u16, |alignment, shape| {
            valid_alignment(shape.alignment).map(|_| alignment.max(shape.alignment))
        })?;
    let payload_byte_offset = align_up(common_end, u32::from(payload_alignment))?;

    let mut end = common_end;
    let cases = case_field_shapes
        .iter()
        .map(|fields| {
            let (fields, case_end, _) = pack_fields_at(fields, payload_byte_offset)?;
            end = end.max(case_end);
            Ok(ConventionalSumCaseLayout { fields })
        })
        .collect::<Result<Vec<_>, AggregateLayoutError>>()?;

    let alignment = tag_shape
        .alignment
        .max(common_alignment)
        .max(payload_alignment);
    let byte_size = align_up(end, u32::from(alignment))?;

    Ok(ConventionalSumLayout {
        shape: ValueShape::integer(narrow(byte_size)?, alignment),
        tag_byte_offset: 0,
        tag_shape,
        common_fields,
        payload_byte_offset: narrow(payload_byte_offset)?,
        cases,
    })
}

fn pack_fields_at(
    shapes: &[ValueShape],
    base_offset: u32,
) -> Result<(Vec<PackedFieldLayout>, u32, u16), AggregateLayoutError> {
    let mut offset = base_offset;
    let mut alignment = 1_u16;
    let fields = shapes
        .iter()
        .map(|shape| {
            valid_alignment(shape.alignment)?;
            offset = align_up(offset, u32::from(shape.alignment))?;
            let byte_offset = narrow(offset)?;
            offset = offset
                .checked_add(u32::from(shape.byte_size))
                .ok_or(AggregateLayoutError::SizeOverflow)?;
            alignment = alignment.max(shape.alignment);
            Ok(PackedFieldLayout {
                shape: *shape,
                byte_offset,
            })
        })
        .collect::<Result<Vec<_>, AggregateLayoutError>>()?;
    Ok((fields, align_up(offset, u32::from(alignment))?, alignment))
}

fn valid_alignment(alignment: u16) -> Result<(), AggregateLayoutError> {
    (alignment != 0)
        .then_some(())
        .ok_or(AggregateLayoutError::ZeroAlignment)
}

fn align_up(value: u32, alignment: u32) -> Result<u32, AggregateLayoutError> {
    if alignment == 0 {
        return Err(AggregateLayoutError::ZeroAlignment);
    }
    let remainder = value % alignment;
    if remainder == 0 {
        Ok(value)
    } else {
        value
            .checked_add(alignment - remainder)
            .ok_or(AggregateLayoutError::SizeOverflow)
    }
}

fn narrow(value: u32) -> Result<u16, AggregateLayoutError> {
    u16::try_from(value).map_err(|_| AggregateLayoutError::SizeOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn empty_case_set_and_oversized_payload_fail_closed() {
        assert_eq!(
            evaluate_conventional_sum_layout(&[], &[]),
            Err(AggregateLayoutError::EmptyCaseSet)
        );
        assert_eq!(
            evaluate_conventional_sum_layout(
                &[],
                &[vec![
                    ValueShape::integer(u16::MAX, 1),
                    ValueShape::integer(1, 1)
                ]],
            ),
            Err(AggregateLayoutError::SizeOverflow)
        );
    }
}
