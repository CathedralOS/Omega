//! Read or replace one field through retained placement geometry.
//!
//! This is the inverse data operation of aggregate materialization, not a
//! layout validator or a content-admission route. Whole aggregates copy their
//! exact extent; repeated outer arrays gather ordered element placements;
//! scalar fragments and narrowed integers use the existing scalar decoder.
//! Stable member identity selects numbered fields even when names change.
//! No result of this reader establishes ownership, qualifications or backing.

use super::field_identities::materialization_field_key;
use super::field_values::{AggregateFieldSchema, AggregateFieldShape, ScalarFieldSchema};
use super::{MaterializationDiagnostic, decode_scalar_layout, fixed_layout_byte_len};
use crate::{ByteOrder, LayoutPlacementReport, LayoutPlanReport};

/// Replace one scalar's laid bits, preserving every other field and padding.
/// This pure codec grants no access. The caller supplies admitted bytes and
/// commits only its authorized effect footprint. Failure leaves them unchanged.
pub fn update_scalar_field(
    layout: &LayoutPlanReport,
    value: &super::field_values::ScalarFieldValue,
    byte_order: ByteOrder,
    destination: &mut [u8],
) -> Result<(), MaterializationDiagnostic> {
    let extent = fixed_layout_byte_len(layout, "scalar field replacement")?;
    let source = destination.get(..extent).ok_or_else(|| {
        MaterializationDiagnostic("scalar field replacement exceeds supplied storage".into())
    })?;
    let mut staged = source.to_vec();
    stage_scalar_field(layout, value, byte_order, &mut staged)?;
    destination[..extent].copy_from_slice(&staged);
    Ok(())
}

/// Replace one scalar inside caller-owned private scratch without allocating
/// a second byte image. Other fields, padding and the allocation tail remain
/// unchanged on success. On refusal scratch contents are unspecified: callers
/// must not publish them. Use `update_scalar_field` for failure-atomic mutation.
pub fn stage_scalar_field(
    layout: &LayoutPlanReport,
    value: &super::field_values::ScalarFieldValue,
    byte_order: ByteOrder,
    destination: &mut [u8],
) -> Result<(), MaterializationDiagnostic> {
    let extent = fixed_layout_byte_len(layout, "scalar field replacement")?;
    let staged = destination.get_mut(..extent).ok_or_else(|| {
        MaterializationDiagnostic("scalar field replacement exceeds supplied storage".into())
    })?;
    let key = materialization_field_key(&value.field, value.member_identity);
    let mut entries = layout
        .entries
        .iter()
        .filter(|entry| materialization_field_key(&entry.field, entry.member_identity) == key)
        .peekable();
    if entries.peek().is_none() {
        return Err(MaterializationDiagnostic(
            "scalar field is absent from its retained plan".into(),
        ));
    }
    for entry in entries {
        crate::symbolic_materialization::apply_scalar_entry(staged, byte_order, entry, value)?;
    }
    Ok(())
}

/// Decode a compiler-derived field extent from its validated containing plan.
/// The source may include a larger allocation tail; only the fixed plan extent
/// is visible. Other fields are not decoded or revalidated by this operation.
pub fn decode_aggregate_field(
    layout: &LayoutPlanReport,
    field: &AggregateFieldSchema,
    byte_order: ByteOrder,
    source: &[u8],
) -> Result<Vec<u8>, MaterializationDiagnostic> {
    let prepared = prepare_aggregate_field(layout, field, byte_order, source)?;
    let width = prepared.width();
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(width)
        .map_err(|_| MaterializationDiagnostic("field exceeds available storage".into()))?;
    decoded.resize(width, 0);
    prepared.copy_into(&mut decoded)?;
    Ok(decoded)
}

/// Decode into a caller-owned field payload of exactly the declared extent.
/// No byte payload is allocated inside this operation. Destination contents
/// are unspecified on refusal; the caller keeps them private until success.
pub fn decode_aggregate_field_into(
    layout: &LayoutPlanReport,
    field: &AggregateFieldSchema,
    byte_order: ByteOrder,
    source: &[u8],
    destination: &mut [u8],
) -> Result<(), MaterializationDiagnostic> {
    prepare_aggregate_field(layout, field, byte_order, source)?.copy_into(destination)
}

/// Geometry and source ranges are established before either API allocates or
/// writes a payload. The prepared form owns only one inline scalar or borrowed
/// source slices; it never hides another heap byte image.
enum DecodedAggregateField<'source> {
    Scalar {
        value: u64,
        byte_order: ByteOrder,
        width: usize,
    },
    Chunks {
        width: usize,
        parts: Vec<&'source [u8]>,
    },
}

impl DecodedAggregateField<'_> {
    fn width(&self) -> usize {
        match self {
            Self::Scalar { width, .. } | Self::Chunks { width, .. } => *width,
        }
    }

    fn copy_into(self, destination: &mut [u8]) -> Result<(), MaterializationDiagnostic> {
        if destination.len() != self.width() {
            return Err(MaterializationDiagnostic(
                "aggregate field destination differs from the complete field extent".into(),
            ));
        }
        match self {
            Self::Scalar {
                value,
                byte_order,
                width,
            } => match byte_order {
                ByteOrder::LittleEndian => {
                    destination.copy_from_slice(&value.to_le_bytes()[..width])
                }
                ByteOrder::BigEndian => {
                    destination.copy_from_slice(&value.to_be_bytes()[8 - width..])
                }
            },
            Self::Chunks { parts, .. } => {
                let mut remaining = destination;
                for part in parts {
                    let Some((receiving, tail)) = remaining.split_at_mut_checked(part.len()) else {
                        return Err(MaterializationDiagnostic(
                            "aggregate field source ranges exceed its complete extent".into(),
                        ));
                    };
                    receiving.copy_from_slice(part);
                    remaining = tail;
                }
            }
        }
        Ok(())
    }
}

fn prepare_aggregate_field<'source>(
    layout: &LayoutPlanReport,
    field: &AggregateFieldSchema,
    byte_order: ByteOrder,
    source: &'source [u8],
) -> Result<DecodedAggregateField<'source>, MaterializationDiagnostic> {
    let failure = |reason: &str| {
        MaterializationDiagnostic(format!(
            "aggregate field `{}` cannot decode: {reason}",
            field.field,
        ))
    };
    let extent = fixed_layout_byte_len(layout, "aggregate field decoding")?;
    let source = source
        .get(..extent)
        .ok_or_else(|| failure("source is shorter than the layout"))?;
    let key = materialization_field_key(&field.field, field.member_identity);
    let entries = layout
        .entries
        .iter()
        .filter(|entry| materialization_field_key(&entry.field, entry.member_identity) == key)
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return Err(failure("field is absent from the retained plan"));
    }
    let width = usize::try_from(field.byte_size)
        .map_err(|_| failure("field extent exceeds host coordinates"))?;
    if entries
        .iter()
        .any(|entry| !matches!(entry.placement, LayoutPlacementReport::At { .. }))
    {
        if !matches!(field.shape, AggregateFieldShape::Whole) || width == 0 || width > 8 {
            return Err(failure(
                "scalar placements require one field of at most eight bytes",
            ));
        }
        if entries
            .iter()
            .any(|entry| matches!(entry.placement, LayoutPlacementReport::At { .. }))
        {
            return Err(failure("whole-field and scalar placements cannot mix"));
        }
        let integer = entries
            .iter()
            .any(|entry| matches!(entry.placement, LayoutPlacementReport::IntegerAt { .. }));
        // A validated fragmented representation may use fewer bits than its
        // semantic carrier (bool and nonnegative range-qualified integers).
        // Gather exactly that complete representation and zero-fill the high
        // carrier bits. Signed-capable fragments retain the carrier width;
        // IntegerAt instead extends according to its authored interpretation.
        let representation_bits = if integer {
            field
                .byte_size
                .checked_mul(8)
                .ok_or_else(|| failure("carrier width overflows"))?
        } else {
            entries.iter().try_fold(0_u64, |maximum, entry| {
                let LayoutPlacementReport::Bits {
                    source_lsb, width, ..
                } = entry.placement
                else {
                    return Err(failure("unsupported scalar placement"));
                };
                source_lsb
                    .checked_add(width)
                    .map(|end| maximum.max(end))
                    .ok_or_else(|| failure("fragment source width overflows"))
            })?
        };
        if representation_bits > field.byte_size * 8 {
            return Err(failure("fragment exceeds its semantic carrier"));
        }
        let representation_bits = u16::try_from(representation_bits)
            .map_err(|_| failure("scalar width exceeds the decoder carrier"))?;
        let schema = match field.member_identity {
            Some(identity) => {
                ScalarFieldSchema::new_numbered(&field.field, identity, representation_bits)
            }
            None => ScalarFieldSchema::new(&field.field, representation_bits),
        }?;
        let selected = LayoutPlanReport {
            entries: entries.into_iter().cloned().collect(),
            offsets: None,
            ..layout.clone()
        };
        let decoded = decode_scalar_layout(&selected, &[schema], byte_order, source)?;
        let [value] = decoded.as_slice() else {
            return Err(failure("scalar decoder lost its field"));
        };
        return Ok(DecodedAggregateField::Scalar {
            value: value.value,
            byte_order,
            width,
        });
    }
    let mut offsets = entries
        .iter()
        .map(|entry| match entry.placement {
            LayoutPlacementReport::At { offset } => Ok(offset),
            _ => Err(failure("non-byte placement reached aggregate gathering")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    offsets.sort_unstable();
    let chunk_size = if offsets.len() == 1 {
        width
    } else {
        let AggregateFieldShape::Repeated {
            element_byte_size,
            element_align,
            element_count,
        } = field.shape
        else {
            return Err(failure("multiple placements require an outer fixed array"));
        };
        if u64::try_from(offsets.len()).ok() != Some(element_count) {
            return Err(failure(
                "element placement count differs from the array length",
            ));
        }
        let stride = offsets[1]
            .checked_sub(offsets[0])
            .ok_or_else(|| failure("unordered element offsets"))?;
        if stride < element_byte_size
            || offsets.windows(2).any(|pair| pair[1] - pair[0] != stride)
            || element_align == 0
            || offsets
                .iter()
                .any(|offset| !offset.is_multiple_of(element_align))
        {
            return Err(failure(
                "element placements need an aligned nonoverlapping constant stride",
            ));
        }
        usize::try_from(element_byte_size)
            .map_err(|_| failure("element extent exceeds host coordinates"))?
    };
    if chunk_size.checked_mul(offsets.len()) != Some(width) {
        return Err(failure("placements do not cover the complete field"));
    }
    let mut parts = Vec::with_capacity(offsets.len());
    for offset in offsets {
        let start =
            usize::try_from(offset).map_err(|_| failure("offset exceeds host coordinates"))?;
        let end = start
            .checked_add(chunk_size)
            .ok_or_else(|| failure("field byte range overflows"))?;
        let bytes = source
            .get(start..end)
            .ok_or_else(|| failure("field exceeds the retained layout extent"))?;
        parts.push(bytes);
    }
    Ok(DecodedAggregateField::Chunks { width, parts })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{layout_field_entry, numbered_layout_field_entry};
    use crate::{
        AggregateFieldValue, IntegerInterpretation, materialize_aggregate_layout_into,
        stage_aggregate_layout,
    };

    #[test]
    fn scalar_replacement_preserves_neighbours_padding_and_failed_writes() {
        let layout = LayoutPlanReport {
            schema_report_fingerprint: 0,
            offsets: None,
            size: Some(8),
            align: 1,
            entries: vec![
                layout_field_entry(
                    "small",
                    LayoutPlacementReport::Bits {
                        container: 0,
                        container_width: 16,
                        destination_lsb: 3,
                        source_lsb: 0,
                        width: 3,
                    },
                ),
                layout_field_entry(
                    "signed",
                    LayoutPlacementReport::IntegerAt {
                        offset: 4,
                        stored_width: 8,
                        interpretation: IntegerInterpretation::Signed,
                    },
                ),
            ],
        };
        for order in [ByteOrder::LittleEndian, ByteOrder::BigEndian] {
            let mut image = [0xa5; 12];
            let value = crate::ScalarFieldValue::new("small", 8, 2).unwrap();
            let mut scratch = image;
            stage_scalar_field(&layout, &value, order, &mut scratch).unwrap();
            update_scalar_field(&layout, &value, order, &mut image).unwrap();
            assert_eq!(scratch, image);
            let changed = if order == ByteOrder::LittleEndian {
                0
            } else {
                1
            };
            let mut expected = [0xa5; 12];
            expected[changed] = (0xa5 & !0x38) | (2 << 3);
            assert_eq!(image, expected);
            assert_eq!(
                decode_aggregate_field(
                    &layout,
                    &AggregateFieldSchema::new("small", 1).unwrap(),
                    order,
                    &image
                )
                .unwrap(),
                [2]
            );
            let signed = crate::ScalarFieldValue::new("signed", 64, (-2_i64) as u64).unwrap();
            stage_scalar_field(&layout, &signed, order, &mut scratch).unwrap();
            update_scalar_field(&layout, &signed, order, &mut image).unwrap();
            assert_eq!(scratch, image);
            expected[4] = 0xfe;
            assert_eq!(image, expected);
            let too_large = crate::ScalarFieldValue::new("signed", 64, 128).unwrap();
            assert!(update_scalar_field(&layout, &too_large, order, &mut image).is_err());
            assert_eq!(image, expected);
            let missing = crate::ScalarFieldValue::new("absent", 8, 0).unwrap();
            assert!(update_scalar_field(&layout, &missing, order, &mut image).is_err());
            assert_eq!(image, expected);

            // A later fragment can refuse after earlier bits were staged. The
            // atomic wrapper must not publish that partial update or touch tail.
            let mut invalid = layout.clone();
            invalid.entries.push(layout_field_entry(
                "small",
                LayoutPlacementReport::At { offset: 8 },
            ));
            let mut atomic = [0xa5; 12];
            assert!(update_scalar_field(&invalid, &value, order, &mut atomic).is_err());
            assert_eq!(atomic, [0xa5; 12]);
            let mut private = atomic;
            assert!(stage_scalar_field(&invalid, &value, order, &mut private).is_err());
            assert_ne!(private[..8], atomic[..8]);
            assert_eq!(private[8..], atomic[8..]);
        }
    }

    #[test]
    fn decode_aggregate_field_inverts_narrow_fragments_integers_and_whole_bytes() {
        let layout = LayoutPlanReport {
            schema_report_fingerprint: 0,
            offsets: None,
            size: Some(12),
            align: 1,
            entries: vec![
                layout_field_entry(
                    "enabled",
                    LayoutPlacementReport::Bits {
                        container: 0,
                        container_width: 8,
                        destination_lsb: 0,
                        source_lsb: 0,
                        width: 1,
                    },
                ),
                layout_field_entry(
                    "small",
                    LayoutPlacementReport::Bits {
                        container: 0,
                        container_width: 8,
                        destination_lsb: 1,
                        source_lsb: 0,
                        width: 3,
                    },
                ),
                layout_field_entry(
                    "signed",
                    LayoutPlacementReport::IntegerAt {
                        offset: 1,
                        stored_width: 8,
                        interpretation: IntegerInterpretation::Signed,
                    },
                ),
                layout_field_entry("bytes", LayoutPlacementReport::At { offset: 4 }),
            ],
        };
        let schemas = [
            AggregateFieldSchema::new("enabled", 1).unwrap(),
            AggregateFieldSchema::new("small", 4).unwrap(),
            AggregateFieldSchema::new("signed", 8).unwrap(),
            AggregateFieldSchema::new("bytes", 8).unwrap(),
        ];
        for order in [ByteOrder::LittleEndian, ByteOrder::BigEndian] {
            let (small, signed) = match order {
                ByteOrder::LittleEndian => (5_u32.to_le_bytes(), (-2_i64).to_le_bytes()),
                ByteOrder::BigEndian => (5_u32.to_be_bytes(), (-2_i64).to_be_bytes()),
            };
            let values = [
                AggregateFieldValue::new("enabled", vec![1]).unwrap(),
                AggregateFieldValue::new("small", small.to_vec()).unwrap(),
                AggregateFieldValue::new("signed", signed.to_vec()).unwrap(),
                AggregateFieldValue::new("bytes", vec![1, 9, 2, 8, 3, 7, 4, 6]).unwrap(),
            ];
            let mut image = vec![0xa5; 16];
            materialize_aggregate_layout_into(&layout, &schemas, &values, order, &mut image)
                .unwrap();
            let borrowed = values
                .iter()
                .map(|value| {
                    AggregateFieldValue::from_storage(&value.field, value.bytes.as_slice()).unwrap()
                })
                .collect::<Vec<_>>();
            let mut scratch = [0xa5; 16];
            stage_aggregate_layout(&layout, &schemas, &borrowed, order, &mut scratch).unwrap();
            assert_eq!(scratch.as_slice(), image);
            for (schema, original) in schemas.iter().zip(&values) {
                assert_eq!(
                    decode_aggregate_field(&layout, schema, order, &image).unwrap(),
                    original.bytes
                );
                let mut receiving = vec![0xa5; schema.byte_size as usize];
                decode_aggregate_field_into(&layout, schema, order, &image, &mut receiving)
                    .unwrap();
                assert_eq!(receiving, original.bytes);
                receiving.push(0);
                assert!(
                    decode_aggregate_field_into(&layout, schema, order, &image, &mut receiving)
                        .is_err()
                );
            }
            assert_eq!(&image[12..], &[0xa5; 4]);
        }
    }

    #[test]
    fn decode_aggregate_field_gathers_numbered_array_elements_by_retained_offsets() {
        let layout = LayoutPlanReport {
            schema_report_fingerprint: 0,
            offsets: None,
            size: Some(10),
            align: 2,
            entries: [8, 0, 4]
                .into_iter()
                .map(|offset| {
                    numbered_layout_field_entry(
                        "old_name".into(),
                        91,
                        LayoutPlacementReport::At { offset },
                    )
                })
                .collect(),
        };
        let schema = AggregateFieldSchema::new_repeated_numbered("new_name", 91, 2, 2, 3).unwrap();
        let original = AggregateFieldValue::new("new_name", vec![1, 2, 3, 4, 5, 6]).unwrap();
        let mut image = vec![0; 10];
        materialize_aggregate_layout_into(
            &layout,
            &[schema.clone()],
            &[original.clone()],
            ByteOrder::LittleEndian,
            &mut image,
        )
        .unwrap();
        assert_eq!(image, [1, 2, 0, 0, 3, 4, 0, 0, 5, 6]);
        let mut receiving = [0; 6];
        decode_aggregate_field_into(
            &layout,
            &schema,
            ByteOrder::LittleEndian,
            &image,
            &mut receiving,
        )
        .unwrap();
        assert_eq!(receiving.as_slice(), original.bytes);
        assert_eq!(
            decode_aggregate_field(&layout, &schema, ByteOrder::LittleEndian, &image).unwrap(),
            original.bytes
        );
        let wrong = AggregateFieldSchema::new_repeated_numbered("old_name", 92, 2, 2, 3).unwrap();
        assert!(decode_aggregate_field(&layout, &wrong, ByteOrder::LittleEndian, &image).is_err());
    }

    #[test]
    fn decode_aggregate_field_refuses_fragment_gaps_and_layout_tail_reads() {
        let mut layout = LayoutPlanReport {
            schema_report_fingerprint: 0,
            offsets: None,
            size: Some(1),
            align: 1,
            entries: vec![layout_field_entry(
                "value",
                LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 8,
                    destination_lsb: 0,
                    source_lsb: 1,
                    width: 3,
                },
            )],
        };
        let schema = AggregateFieldSchema::new("value", 1).unwrap();
        assert!(decode_aggregate_field(&layout, &schema, ByteOrder::LittleEndian, &[7]).is_err());
        layout.entries[0].placement = LayoutPlacementReport::At { offset: 1 };
        // The supplied allocation has a second byte, but this plan does not.
        assert!(
            decode_aggregate_field(&layout, &schema, ByteOrder::LittleEndian, &[7, 8]).is_err()
        );
    }

    #[test]
    fn decode_rejects_invalid_geometry_before_allocating_declared_payload() {
        let mut layout = LayoutPlanReport {
            schema_report_fingerprint: 0,
            offsets: None,
            size: Some(1),
            align: 1,
            entries: vec![layout_field_entry(
                "value",
                LayoutPlacementReport::At { offset: 0 },
            )],
        };
        let absent = AggregateFieldSchema::new("absent", u64::MAX).unwrap();
        let error =
            decode_aggregate_field(&layout, &absent, ByteOrder::LittleEndian, &[7]).unwrap_err();
        assert!(error.0.contains("field is absent"), "{error:?}");
        let present = AggregateFieldSchema::new("value", u64::MAX).unwrap();
        let error =
            decode_aggregate_field(&layout, &present, ByteOrder::LittleEndian, &[]).unwrap_err();
        assert!(error.0.contains("source is shorter"), "{error:?}");
        layout.size = None;
        let error =
            decode_aggregate_field(&layout, &present, ByteOrder::LittleEndian, &[7]).unwrap_err();
        assert!(
            error.0.contains("requires a fixed-size layout plan"),
            "{error:?}"
        );
    }
}
