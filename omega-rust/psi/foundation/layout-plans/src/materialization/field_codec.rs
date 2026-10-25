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
    let mut staged = source.to_vec();
    for entry in entries {
        crate::symbolic_materialization::apply_scalar_entry(&mut staged, byte_order, entry, value)?;
    }
    destination[..extent].copy_from_slice(&staged);
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
    let width = usize::try_from(field.byte_size)
        .map_err(|_| MaterializationDiagnostic("field extent exceeds host coordinates".into()))?;
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(width)
        .map_err(|_| MaterializationDiagnostic("field exceeds available storage".into()))?;
    decoded.resize(width, 0);
    decode_aggregate_field_into(layout, field, byte_order, source, &mut decoded)?;
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
    if destination.len() != width {
        return Err(failure(
            "destination differs from the complete field extent",
        ));
    }
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
        match byte_order {
            ByteOrder::LittleEndian => {
                destination.copy_from_slice(&value.value.to_le_bytes()[..width])
            }
            ByteOrder::BigEndian => {
                destination.copy_from_slice(&value.value.to_be_bytes()[8 - width..])
            }
        }
        return Ok(());
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
    for (ordinal, offset) in offsets.into_iter().enumerate() {
        let start =
            usize::try_from(offset).map_err(|_| failure("offset exceeds host coordinates"))?;
        let end = start
            .checked_add(chunk_size)
            .ok_or_else(|| failure("field byte range overflows"))?;
        let bytes = source
            .get(start..end)
            .ok_or_else(|| failure("field exceeds the retained layout extent"))?;
        let receiving_start = ordinal
            .checked_mul(chunk_size)
            .ok_or_else(|| failure("receiving range overflows"))?;
        let receiving_end = receiving_start
            .checked_add(chunk_size)
            .ok_or_else(|| failure("receiving range overflows"))?;
        destination
            .get_mut(receiving_start..receiving_end)
            .ok_or_else(|| failure("receiving range exceeds complete field"))?
            .copy_from_slice(bytes);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AggregateFieldValue, IntegerInterpretation, LayoutFieldEntryReport,
        materialize_aggregate_layout_into, stage_aggregate_layout,
    };

    fn entry(field: &str, placement: LayoutPlacementReport) -> LayoutFieldEntryReport {
        LayoutFieldEntryReport {
            field: field.into(),
            member_identity: None,
            placement,
        }
    }

    #[test]
    fn scalar_replacement_preserves_neighbours_padding_and_failed_writes() {
        let layout = LayoutPlanReport {
            schema_report_fingerprint: 0,
            offsets: None,
            size: Some(8),
            align: 1,
            entries: vec![
                entry(
                    "small",
                    LayoutPlacementReport::Bits {
                        container: 0,
                        container_width: 16,
                        destination_lsb: 3,
                        source_lsb: 0,
                        width: 3,
                    },
                ),
                entry(
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
            update_scalar_field(&layout, &value, order, &mut image).unwrap();
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
            update_scalar_field(&layout, &signed, order, &mut image).unwrap();
            expected[4] = 0xfe;
            assert_eq!(image, expected);
            let too_large = crate::ScalarFieldValue::new("signed", 64, 128).unwrap();
            assert!(update_scalar_field(&layout, &too_large, order, &mut image).is_err());
            assert_eq!(image, expected);
            let missing = crate::ScalarFieldValue::new("absent", 8, 0).unwrap();
            assert!(update_scalar_field(&layout, &missing, order, &mut image).is_err());
            assert_eq!(image, expected);
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
                entry(
                    "enabled",
                    LayoutPlacementReport::Bits {
                        container: 0,
                        container_width: 8,
                        destination_lsb: 0,
                        source_lsb: 0,
                        width: 1,
                    },
                ),
                entry(
                    "small",
                    LayoutPlacementReport::Bits {
                        container: 0,
                        container_width: 8,
                        destination_lsb: 1,
                        source_lsb: 0,
                        width: 3,
                    },
                ),
                entry(
                    "signed",
                    LayoutPlacementReport::IntegerAt {
                        offset: 1,
                        stored_width: 8,
                        interpretation: IntegerInterpretation::Signed,
                    },
                ),
                entry("bytes", LayoutPlacementReport::At { offset: 4 }),
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
                .map(|offset| LayoutFieldEntryReport {
                    field: "old_name".into(),
                    member_identity: Some(91),
                    placement: LayoutPlacementReport::At { offset },
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
            entries: vec![entry(
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
}
