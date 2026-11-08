/*
We implement byte-level materialization and decoding for scalar and aggregate
layout plans, combining field value schemas (`field_values`), member-identity
indexing (`field_identities`), bit-fragment integer validation
(`stored_integer_writes`), and scalar/aggregate codec routines (`field_codec`).
*/

pub(crate) mod field_codec {
    use super::field_identities::materialization_field_key;
    use super::field_values::{AggregateFieldSchema, AggregateFieldShape, ScalarFieldSchema};
    use super::{MaterializationDiagnostic, decode_scalar_layout, fixed_layout_byte_len};
    use crate::{ByteOrder, LayoutPlacementReport, LayoutPlanReport};

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

    pub fn decode_aggregate_field_into(
        layout: &LayoutPlanReport,
        field: &AggregateFieldSchema,
        byte_order: ByteOrder,
        source: &[u8],
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        prepare_aggregate_field(layout, field, byte_order, source)?.copy_into(destination)
    }

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
                        let Some((receiving, tail)) = remaining.split_at_mut_checked(part.len())
                        else {
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
                        AggregateFieldValue::from_storage(&value.field, value.bytes.as_slice())
                            .unwrap()
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
                            "old_name",
                            91,
                            LayoutPlacementReport::At { offset },
                        )
                    })
                    .collect(),
            };
            let schema =
                AggregateFieldSchema::new_repeated_numbered("new_name", 91, 2, 2, 3).unwrap();
            let original = AggregateFieldValue::new("new_name", vec![1, 2, 3, 4, 5, 6]).unwrap();
            let mut image = vec![0; 10];
            materialize_aggregate_layout_into(
                &layout,
                std::slice::from_ref(&schema),
                std::slice::from_ref(&original),
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
            let wrong =
                AggregateFieldSchema::new_repeated_numbered("old_name", 92, 2, 2, 3).unwrap();
            assert!(
                decode_aggregate_field(&layout, &wrong, ByteOrder::LittleEndian, &image).is_err()
            );
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
            assert!(
                decode_aggregate_field(&layout, &schema, ByteOrder::LittleEndian, &[7]).is_err()
            );
            layout.entries[0].placement = LayoutPlacementReport::At { offset: 1 };
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
            let error = decode_aggregate_field(&layout, &absent, ByteOrder::LittleEndian, &[7])
                .unwrap_err();
            assert!(error.0.contains("field is absent"), "{error:?}");
            let present = AggregateFieldSchema::new("value", u64::MAX).unwrap();
            let error = decode_aggregate_field(&layout, &present, ByteOrder::LittleEndian, &[])
                .unwrap_err();
            assert!(error.0.contains("source is shorter"), "{error:?}");
            layout.size = None;
            let error = decode_aggregate_field(&layout, &present, ByteOrder::LittleEndian, &[7])
                .unwrap_err();
            assert!(
                error.0.contains("requires a fixed-size layout plan"),
                "{error:?}"
            );
        }
    }
}
pub(crate) mod field_identities {
    use crate::layout_reports::{
        ConventionalSumLayoutReport, LayoutFieldEntryReport, LayoutPlanReport,
    };
    use crate::materialization::MaterializationDiagnostic;
    use crate::symbolic_values::SymbolicFieldValue;

    use std::collections::{BTreeMap, BTreeSet};

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) enum MaterializationFieldKey {
        Numbered(u64),
        Positional(String),
    }

    pub(crate) fn materialization_field_key(
        field: &str,
        member_identity: Option<u64>,
    ) -> MaterializationFieldKey {
        match member_identity {
            Some(identity) => MaterializationFieldKey::Numbered(identity),
            None => MaterializationFieldKey::Positional(field.to_owned()),
        }
    }

    pub(crate) fn planned_entries_by_key(
        layout: &LayoutPlanReport,
    ) -> BTreeMap<MaterializationFieldKey, Vec<&LayoutFieldEntryReport>> {
        let mut planned: BTreeMap<MaterializationFieldKey, Vec<&LayoutFieldEntryReport>> =
            BTreeMap::new();
        for entry in &layout.entries {
            planned
                .entry(materialization_field_key(
                    &entry.field,
                    entry.member_identity,
                ))
                .or_default()
                .push(entry);
        }
        planned
    }

    struct SuppliedField<'fields, Field> {
        value: &'fields Field,
        name: &'fields str,
        member_identity: Option<u64>,
    }

    pub(crate) struct SuppliedFields<'fields, Field> {
        by_key: BTreeMap<MaterializationFieldKey, SuppliedField<'fields, Field>>,
        by_name: BTreeMap<&'fields str, &'fields Field>,
    }

    impl<'fields, Field> SuppliedFields<'fields, Field> {
        pub(crate) fn value(&self, key: &MaterializationFieldKey) -> Option<&'fields Field> {
            self.by_key.get(key).map(|supplied| supplied.value)
        }

        pub(crate) fn named(&self, name: &str) -> Option<&'fields Field> {
            self.by_name.get(name).copied()
        }

        pub(crate) fn fields_by_key(
            &self,
        ) -> impl Iterator<Item = (&MaterializationFieldKey, &'fields Field)> {
            self.by_key
                .iter()
                .map(|(key, supplied)| (key, supplied.value))
        }

        pub(crate) fn names(&self) -> impl Iterator<Item = &'fields str> {
            self.by_name.keys().copied()
        }
    }

    pub(crate) fn index_supplied_fields<'fields, Field>(
        fields: &'fields [Field],
        kind: &str,
        duplicate_verb: &str,
        supplied: impl Fn(&'fields Field) -> (&'fields str, Option<u64>),
    ) -> Result<SuppliedFields<'fields, Field>, MaterializationDiagnostic> {
        let mut indexed = SuppliedFields {
            by_key: BTreeMap::new(),
            by_name: BTreeMap::new(),
        };
        for field in fields {
            let (name, member_identity) = supplied(field);
            if indexed.by_name.insert(name, field).is_some() {
                return Err(MaterializationDiagnostic(format!(
                    "{kind} field `{name}` is {duplicate_verb} more than once"
                )));
            }
            let key = materialization_field_key(name, member_identity);
            let indexed_field = SuppliedField {
                value: field,
                name,
                member_identity,
            };
            if indexed.by_key.insert(key, indexed_field).is_some() {
                return Err(MaterializationDiagnostic(format!(
                    "{kind} field `{name}` repeats stable member identity #{}",
                    member_identity
                        .expect("only numbered fields can collide after name validation")
                )));
            }
        }
        Ok(indexed)
    }

    pub(crate) fn validate_supplied_field_coverage<Field>(
        layout: &LayoutPlanReport,
        supplied: &SuppliedFields<'_, Field>,
        missing_noun: &str,
        extra_noun: &str,
    ) -> Result<(), MaterializationDiagnostic> {
        if let Some(entry) = layout.entries.iter().find(|entry| {
            !supplied.by_key.contains_key(&materialization_field_key(
                &entry.field,
                entry.member_identity,
            ))
        }) {
            let suffix = stable_identity_suffix(entry.member_identity);
            return Err(MaterializationDiagnostic(format!(
                "layout field `{}` has no {missing_noun}{suffix}",
                entry.field
            )));
        }
        let planned = layout
            .entries
            .iter()
            .map(|entry| materialization_field_key(&entry.field, entry.member_identity))
            .collect::<BTreeSet<_>>();
        if let Some(field) = supplied
            .by_key
            .iter()
            .find_map(|(key, field)| (!planned.contains(key)).then_some(field))
        {
            let suffix = stable_identity_suffix(field.member_identity);
            return Err(MaterializationDiagnostic(format!(
                "{extra_noun} `{}` has no entry in the validated layout plan{suffix}",
                field.name
            )));
        }
        Ok(())
    }

    pub(crate) fn validate_materialization_field_identities(
        layout: &LayoutPlanReport,
    ) -> Result<(), MaterializationDiagnostic> {
        let mut identity_names = std::collections::BTreeMap::new();
        let mut name_identities = std::collections::BTreeMap::new();
        for entry in &layout.entries {
            let key = materialization_field_key(&entry.field, entry.member_identity);
            if let Some(prior_name) = identity_names.insert(key.clone(), entry.field.as_str())
                && prior_name != entry.field
            {
                return Err(MaterializationDiagnostic(format!(
                    "layout field identity names both `{prior_name}` and `{}`",
                    entry.field
                )));
            }
            if let Some(prior_identity) = name_identities.insert(entry.field.as_str(), key.clone())
                && prior_identity != key
            {
                return Err(MaterializationDiagnostic(format!(
                    "layout field `{}` fragments do not retain the same stable identity",
                    entry.field
                )));
            }
        }
        Ok(())
    }

    pub(crate) fn validate_conventional_sum_materialization_identities(
        layout: &ConventionalSumLayoutReport,
    ) -> Result<(), MaterializationDiagnostic> {
        let mut identity_names = std::collections::BTreeMap::new();
        let mut name_identities = std::collections::BTreeMap::new();
        for case in &layout.cases {
            for (field, member_identity) in [(case.case.as_str(), case.member_identity)]
                .into_iter()
                .chain(
                    case.payload_fields
                        .iter()
                        .map(|payload| (payload.field.as_str(), payload.member_identity)),
                )
            {
                let key = materialization_field_key(field, member_identity);
                if let Some(prior_name) = identity_names.insert(key.clone(), field)
                    && prior_name != field
                {
                    return Err(MaterializationDiagnostic(format!(
                        "sum interior identity names both `{prior_name}` and `{field}`"
                    )));
                }
                if let Some(prior_identity) = name_identities.insert(field, key.clone())
                    && prior_identity != key
                {
                    return Err(MaterializationDiagnostic(format!(
                        "sum interior field `{field}` does not retain the same stable identity"
                    )));
                }
            }
        }
        Ok(())
    }

    pub(crate) const fn stable_identity_suffix(member_identity: Option<u64>) -> &'static str {
        if member_identity.is_some() {
            " with the same stable identity"
        } else {
            ""
        }
    }

    pub(crate) fn symbolic_index_display(element_index: Option<u64>) -> String {
        match element_index {
            Some(index) => format!("[{index}]"),
            None => String::new(),
        }
    }

    pub(crate) fn symbolic_path_display(symbolic: &SymbolicFieldValue) -> String {
        let mut display = format!(
            "{}{}",
            symbolic.field,
            symbolic_index_display(symbolic.element_index)
        );
        let mut segment = symbolic.inner.as_ref();
        while let Some(inner) = segment {
            display.push('.');
            display.push_str(&inner.field);
            display.push_str(&symbolic_index_display(inner.element_index));
            segment = inner.inner();
        }
        display
    }

    type SymbolicPathHop<'a> = (&'a str, Option<u64>, Option<u64>);

    pub(crate) fn symbolic_path_hops(symbolic: &SymbolicFieldValue) -> Vec<SymbolicPathHop<'_>> {
        let mut hops = vec![(
            symbolic.field.as_str(),
            symbolic.member_identity,
            symbolic.element_index,
        )];
        let mut segment = symbolic.inner.as_ref();
        while let Some(inner) = segment {
            hops.push((
                inner.field.as_str(),
                inner.member_identity,
                inner.element_index,
            ));
            segment = inner.inner();
        }
        hops
    }
}
pub(crate) mod field_values {
    use crate::materialization::MaterializationDiagnostic;
    use crate::materialization::stored_integer_writes::low_mask;

    pub(crate) fn validate_authored_width(
        kind: &str,
        width_bits: u16,
    ) -> Result<(), MaterializationDiagnostic> {
        if width_bits == 0 || width_bits > 64 {
            return Err(MaterializationDiagnostic(format!(
                "{kind} field width {width_bits} is outside 1..=64 bits"
            )));
        }
        Ok(())
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ScalarFieldValue {
        pub field: String,
        pub(crate) member_identity: Option<u64>,
        pub width_bits: u16,
        pub value: u64,
    }

    impl ScalarFieldValue {
        pub fn new(
            field: impl Into<String>,
            width_bits: u16,
            value: u64,
        ) -> Result<Self, MaterializationDiagnostic> {
            validate_authored_width("scalar", width_bits)?;
            if width_bits < 64 && value > low_mask(width_bits) {
                return Err(MaterializationDiagnostic(format!(
                    "scalar value {value:#x} does not fit its {width_bits}-bit field"
                )));
            }
            Ok(Self {
                field: field.into(),
                member_identity: None,
                width_bits,
                value,
            })
        }

        numbered_constructor!(new_numbered(field, width_bits: u16, value: u64) -> new);
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AggregateFieldValue<Bytes = Vec<u8>> {
        pub field: String,
        pub bytes: Bytes,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AggregateFieldSchema {
        pub field: String,
        pub(crate) member_identity: Option<u64>,
        pub byte_size: u64,
        pub(crate) shape: AggregateFieldShape,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum AggregateFieldShape {
        Whole,
        Repeated {
            element_byte_size: u64,
            element_align: u64,
            element_count: u64,
        },
    }

    impl AggregateFieldSchema {
        pub fn new(
            field: impl Into<String>,
            byte_size: u64,
        ) -> Result<Self, MaterializationDiagnostic> {
            if byte_size == 0 {
                return Err(MaterializationDiagnostic(
                    "aggregate field schema requires a nonzero physical extent".into(),
                ));
            }
            Ok(Self {
                field: field.into(),
                member_identity: None,
                byte_size,
                shape: AggregateFieldShape::Whole,
            })
        }

        numbered_constructor!(new_numbered(field, byte_size: u64) -> new);

        pub fn new_repeated(
            field: impl Into<String>,
            element_byte_size: u64,
            element_align: u64,
            element_count: u64,
        ) -> Result<Self, MaterializationDiagnostic> {
            if element_byte_size == 0 || element_count == 0 {
                return Err(MaterializationDiagnostic(
                    "repeated aggregate schema requires nonzero element extent and count".into(),
                ));
            }
            if element_align == 0 || !element_align.is_power_of_two() {
                return Err(MaterializationDiagnostic(format!(
                    "repeated aggregate element alignment {element_align} is not a positive power of two"
                )));
            }
            let byte_size = element_byte_size
                .checked_mul(element_count)
                .ok_or_else(|| {
                    MaterializationDiagnostic(
                        "repeated aggregate compiler-derived physical extent overflows".into(),
                    )
                })?;
            Ok(Self {
                field: field.into(),
                member_identity: None,
                byte_size,
                shape: AggregateFieldShape::Repeated {
                    element_byte_size,
                    element_align,
                    element_count,
                },
            })
        }

        numbered_constructor!(
            new_repeated_numbered(
                field,
                element_byte_size: u64,
                element_align: u64,
                element_count: u64,
            ) -> new_repeated
        );
    }

    impl AggregateFieldValue {
        pub fn new(
            field: impl Into<String>,
            bytes: impl Into<Vec<u8>>,
        ) -> Result<Self, MaterializationDiagnostic> {
            Self::from_storage(field, bytes.into())
        }
    }

    impl<Bytes: AsRef<[u8]>> AggregateFieldValue<Bytes> {
        pub fn from_storage(
            field: impl Into<String>,
            bytes: Bytes,
        ) -> Result<Self, MaterializationDiagnostic> {
            if bytes.as_ref().is_empty() {
                return Err(MaterializationDiagnostic(
                    "aggregate field materialization requires a nonempty complete value".into(),
                ));
            }
            Ok(Self {
                field: field.into(),
                bytes,
            })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ScalarFieldSchema {
        pub field: String,
        pub(crate) member_identity: Option<u64>,
        pub width_bits: u16,
    }

    impl ScalarFieldSchema {
        pub fn new(
            field: impl Into<String>,
            width_bits: u16,
        ) -> Result<Self, MaterializationDiagnostic> {
            validate_authored_width("scalar", width_bits)?;
            Ok(Self {
                field: field.into(),
                member_identity: None,
                width_bits,
            })
        }

        numbered_constructor!(new_numbered(field, width_bits: u16) -> new);
    }
}
pub(crate) mod stored_integer_writes {
    use crate::layout_reports::{
        IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport,
    };
    use crate::materialization::MaterializationDiagnostic;
    use crate::materialization::field_values::ScalarFieldValue;
    use crate::placement::{ByteOrder, MaterializationWrite, StoredIntegerFit};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ScalarFragment {
        pub(crate) container_byte_offset: u64,
        pub(crate) container_width_bits: u16,
        pub(crate) destination_lsb: u16,
        pub(crate) source_lsb: u16,
        pub(crate) width: u16,
    }

    pub(crate) fn scalar_fragment(
        entry: &LayoutFieldEntryReport,
        source_width_bits: u16,
    ) -> Result<ScalarFragment, MaterializationDiagnostic> {
        let (container, container_width, destination_lsb, source_lsb, width) =
            materializer_scalar_window(entry.placement, source_width_bits, "scalar", &entry.field)?;
        Ok(ScalarFragment {
            container_byte_offset: container,
            container_width_bits: container_width,
            destination_lsb,
            source_lsb,
            width,
        })
    }

    pub(crate) fn materializer_scalar_window(
        placement: LayoutPlacementReport,
        access_width_bits: u16,
        field_kind: &str,
        field_display: &str,
    ) -> Result<(u64, u16, u16, u16, u16), MaterializationDiagnostic> {
        let (container, container_width, destination_lsb, source_lsb, width) = match placement {
            LayoutPlacementReport::At { offset } => (
                offset,
                u64::from(access_width_bits),
                0,
                0,
                u64::from(access_width_bits),
            ),
            LayoutPlacementReport::IntegerAt {
                offset,
                stored_width,
                ..
            } => (offset, stored_width, 0, 0, stored_width),
            LayoutPlacementReport::Bits {
                container,
                container_width,
                destination_lsb,
                source_lsb,
                width,
            } => (
                container,
                container_width,
                destination_lsb,
                source_lsb,
                width,
            ),
        };
        if container_width == 0 || container_width > 64 || container_width % 8 != 0 || width == 0 {
            return Err(MaterializationDiagnostic(format!(
                "{field_kind} field `{field_display}` uses a materializer-incompatible placement"
            )));
        }
        let source_end = source_lsb.checked_add(width).ok_or_else(|| {
            MaterializationDiagnostic(format!("{field_kind} source bit range overflows"))
        })?;
        if source_end > u64::from(access_width_bits) {
            return Err(MaterializationDiagnostic(format!(
                "{field_kind} field `{field_display}` placement reads through bit {source_end}, past its {access_width_bits}-bit source"
            )));
        }
        let destination_end = destination_lsb.checked_add(width).ok_or_else(|| {
            MaterializationDiagnostic(format!("{field_kind} destination bit range overflows"))
        })?;
        if destination_end > container_width {
            return Err(MaterializationDiagnostic(format!(
                "{field_kind} field `{field_display}` placement writes through bit {destination_end}, past its {container_width}-bit container"
            )));
        }
        Ok((
            container,
            u16::try_from(container_width).expect("validated materializer container width"),
            u16::try_from(destination_lsb).expect("validated destination bit index"),
            u16::try_from(source_lsb).expect("validated source bit index"),
            u16::try_from(width).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "{field_kind} field `{field_display}` fragment width {width} is too large"
                ))
            })?,
        ))
    }

    pub(crate) fn validate_stored_integer_value(
        entry: &LayoutFieldEntryReport,
        value: &ScalarFieldValue,
    ) -> Result<(), MaterializationDiagnostic> {
        validate_stored_integer_bits(entry, value.width_bits, value.value, "scalar")
    }

    pub(crate) const fn byte_sized_width(width_bits: u16) -> bool {
        width_bits != 0 && width_bits <= 64 && width_bits.is_multiple_of(8)
    }

    pub(crate) fn stored_value_fits(
        interpretation: IntegerInterpretation,
        stored_width_bits: u16,
        source_width_bits: u16,
        source_value: u64,
    ) -> bool {
        let (minimum, maximum) = interpretation.stored_bounds(u64::from(stored_width_bits));
        let semantic = match interpretation {
            IntegerInterpretation::Signed => signed_bits(source_value, source_width_bits),
            IntegerInterpretation::Unsigned => i128::from(source_value),
        };
        semantic >= minimum && semantic <= maximum
    }

    fn validate_stored_integer_bits(
        entry: &LayoutFieldEntryReport,
        source_width_bits: u16,
        source_value: u64,
        value_kind: &str,
    ) -> Result<(), MaterializationDiagnostic> {
        let LayoutPlacementReport::IntegerAt {
            stored_width,
            interpretation,
            ..
        } = entry.placement
        else {
            return Ok(());
        };
        let stored_width = u16::try_from(stored_width).map_err(|_| {
            MaterializationDiagnostic(format!(
                "{value_kind} field `{}` has an invalid stored-integer width",
                entry.field,
            ))
        })?;
        if !byte_sized_width(stored_width) {
            return Err(MaterializationDiagnostic(format!(
                "{value_kind} field `{}` has an invalid {stored_width}-bit stored-integer width",
                entry.field
            )));
        }
        if source_width_bits < stored_width {
            return Err(MaterializationDiagnostic(format!(
                "{value_kind} field `{}` has a {source_width_bits}-bit value narrower than its {stored_width}-bit storage",
                entry.field,
            )));
        }

        if !stored_value_fits(
            interpretation,
            stored_width,
            source_width_bits,
            source_value,
        ) {
            return Err(MaterializationDiagnostic(format!(
                "{value_kind} field `{}` value {source_value:#x} does not fit its {stored_width}-bit {} storage",
                entry.field,
                match interpretation {
                    IntegerInterpretation::Signed => "signed",
                    IntegerInterpretation::Unsigned => "unsigned",
                }
            )));
        }
        Ok(())
    }

    pub fn validate_write_source_value(
        write: &MaterializationWrite,
        source_value: u64,
        value_kind: &str,
    ) -> Result<(), MaterializationDiagnostic> {
        let Some(fit) = write.stored_integer_fit else {
            return Ok(());
        };
        validate_stored_integer_fit(&write.field, fit, source_value, value_kind)
    }

    pub fn validate_stored_integer_fit(
        field: &str,
        fit: StoredIntegerFit,
        source_value: u64,
        value_kind: &str,
    ) -> Result<(), MaterializationDiagnostic> {
        validate_stored_integer_fit_shape(field, fit, value_kind)?;
        let StoredIntegerFit {
            source_width_bits,
            stored_width_bits,
            interpretation,
        } = fit;

        if !stored_value_fits(
            interpretation,
            stored_width_bits,
            source_width_bits,
            source_value,
        ) {
            return Err(MaterializationDiagnostic(format!(
                "{value_kind} field `{field}` value {source_value:#x} does not fit its {stored_width_bits}-bit {} storage",
                match interpretation {
                    IntegerInterpretation::Signed => "signed",
                    IntegerInterpretation::Unsigned => "unsigned",
                }
            )));
        }
        Ok(())
    }

    pub fn validate_stored_integer_fit_shape(
        field: &str,
        fit: StoredIntegerFit,
        value_kind: &str,
    ) -> Result<(), MaterializationDiagnostic> {
        let StoredIntegerFit {
            source_width_bits,
            stored_width_bits,
            interpretation: _,
        } = fit;
        if source_width_bits == 0 || source_width_bits > 64 {
            return Err(MaterializationDiagnostic(format!(
                "{value_kind} field `{field}` has an invalid {source_width_bits}-bit source width"
            )));
        }
        if !byte_sized_width(stored_width_bits) || source_width_bits < stored_width_bits {
            return Err(MaterializationDiagnostic(format!(
                "{value_kind} field `{field}` has an invalid {stored_width_bits}-bit stored-integer fit constraint"
            )));
        }
        Ok(())
    }

    fn signed_bits(value: u64, width: u16) -> i128 {
        let value = value & low_mask(width);
        if value & (1_u64 << (width - 1)) == 0 {
            i128::from(value)
        } else {
            i128::from(value) - (1_i128 << width)
        }
    }

    pub fn apply_write(
        bytes: &mut [u8],
        byte_order: ByteOrder,
        write: &MaterializationWrite,
        source_value: u64,
    ) -> Result<(), MaterializationDiagnostic> {
        apply_fragment(
            bytes,
            byte_order,
            &write.field,
            write.container_byte_offset,
            write.container_width_bits,
            write.destination_lsb,
            write.source_lsb,
            write.width,
            source_value,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply_fragment(
        bytes: &mut [u8],
        byte_order: ByteOrder,
        field: &str,
        container_byte_offset: u64,
        container_width_bits: u16,
        destination_lsb: u16,
        source_lsb: u16,
        width: u16,
        source_value: u64,
    ) -> Result<(), MaterializationDiagnostic> {
        let container_bytes = usize::from(container_width_bits / 8);
        let start = usize::try_from(container_byte_offset).map_err(|_| {
            MaterializationDiagnostic("container offset cannot be represented on this host".into())
        })?;
        let end = start
            .checked_add(container_bytes)
            .ok_or_else(|| MaterializationDiagnostic("container byte range overflows".into()))?;
        let materialization_len = bytes.len();
        let container_slice = bytes.get_mut(start..end).ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "symbolic field `{}` writes outside the {}-byte materialization",
                field, materialization_len
            ))
        })?;
        let mut container_value = read_container(container_slice, byte_order);
        let fragment_mask = low_mask(width);
        let fragment = (source_value >> source_lsb) & fragment_mask;
        let destination_mask = fragment_mask << destination_lsb;
        container_value = (container_value & !destination_mask)
            | ((fragment << destination_lsb) & destination_mask);
        write_container(container_slice, byte_order, container_value);
        Ok(())
    }

    pub fn validate_write(
        materialization_len: usize,
        write: &MaterializationWrite,
    ) -> Result<(), MaterializationDiagnostic> {
        validate_fragment(
            materialization_len,
            &write.field,
            write.container_byte_offset,
            write.container_width_bits,
            write.destination_lsb,
            write.source_lsb,
            write.width,
        )?;
        if let Some(fit) = write.stored_integer_fit
            && (fit.stored_width_bits != write.container_width_bits
                || fit.stored_width_bits != write.width
                || write.destination_lsb != 0
                || write.source_lsb != 0)
        {
            return Err(MaterializationDiagnostic(format!(
                "symbolic field `{}` has stored-integer fit evidence inconsistent with its write geometry",
                write.field
            )));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_fragment(
        materialization_len: usize,
        field: &str,
        container_byte_offset: u64,
        container_width_bits: u16,
        destination_lsb: u16,
        source_lsb: u16,
        width: u16,
    ) -> Result<(), MaterializationDiagnostic> {
        if container_width_bits == 0
            || container_width_bits > 64
            || !container_width_bits.is_multiple_of(8)
        {
            return Err(MaterializationDiagnostic(format!(
                "symbolic field `{}` has invalid {}-bit container",
                field, container_width_bits
            )));
        }
        if width == 0
            || width > 64
            || source_lsb.checked_add(width).is_none_or(|end| end > 64)
            || destination_lsb
                .checked_add(width)
                .is_none_or(|end| end > container_width_bits)
        {
            return Err(MaterializationDiagnostic(format!(
                "symbolic field `{}` has an invalid source or destination bit range",
                field
            )));
        }
        let start = usize::try_from(container_byte_offset).map_err(|_| {
            MaterializationDiagnostic("container offset cannot be represented on this host".into())
        })?;
        let end = start
            .checked_add(usize::from(container_width_bits / 8))
            .ok_or_else(|| MaterializationDiagnostic("container byte range overflows".into()))?;
        if end > materialization_len {
            return Err(MaterializationDiagnostic(format!(
                "symbolic field `{}` writes outside the {}-byte materialization",
                field, materialization_len
            )));
        }
        Ok(())
    }

    pub(crate) fn read_container(bytes: &[u8], byte_order: ByteOrder) -> u64 {
        bytes
            .iter()
            .enumerate()
            .fold(0_u64, |value, (index, byte)| {
                let shift = match byte_order {
                    ByteOrder::LittleEndian => index * 8,
                    ByteOrder::BigEndian => (bytes.len() - 1 - index) * 8,
                };
                value | (u64::from(*byte) << shift)
            })
    }

    fn write_container(bytes: &mut [u8], byte_order: ByteOrder, value: u64) {
        let byte_len = bytes.len();
        for (index, byte) in bytes.iter_mut().enumerate() {
            let shift = match byte_order {
                ByteOrder::LittleEndian => index * 8,
                ByteOrder::BigEndian => (byte_len - 1 - index) * 8,
            };
            *byte = ((value >> shift) & 0xff) as u8;
        }
    }

    pub(crate) const fn low_mask(width: u16) -> u64 {
        if width == 64 {
            u64::MAX
        } else {
            (1_u64 << width) - 1
        }
    }
}

use crate::layout_reports::{IntegerInterpretation, LayoutPlacementReport, LayoutPlanReport};
use crate::materialization::field_identities::{
    index_supplied_fields, materialization_field_key, planned_entries_by_key,
    validate_materialization_field_identities, validate_supplied_field_coverage,
};
use crate::materialization::field_values::{
    AggregateFieldSchema, AggregateFieldShape, AggregateFieldValue, ScalarFieldSchema,
    ScalarFieldValue,
};
use crate::materialization::stored_integer_writes::{
    apply_write, byte_sized_width, low_mask, read_container, scalar_fragment, validate_fragment,
    validate_write, validate_write_source_value,
};
use crate::placement::{ByteOrder, MaterializationAction, PlacementConstraints};
use crate::symbolic_materialization::apply_scalar_entry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicMaterializationPlan {
    pub byte_len: usize,
    pub byte_order: ByteOrder,
    pub placement: PlacementConstraints,
    pub actions: Vec<MaterializationAction>,
}

impl SymbolicMaterializationPlan {
    pub fn materialize_resolved_into(
        &self,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        if destination.len() < self.byte_len {
            return Err(MaterializationDiagnostic(format!(
                "materialization needs {} bytes, destination has {}",
                self.byte_len,
                destination.len()
            )));
        }
        if let Some(action) = self
            .actions
            .iter()
            .find(|action| !matches!(action, MaterializationAction::ResolvedWrite { .. }))
        {
            return Err(MaterializationDiagnostic(format!(
                "materialization still contains an unresolved action: {action:?}"
            )));
        }

        for action in &self.actions {
            let MaterializationAction::ResolvedWrite {
                write,
                source_value,
            } = action
            else {
                unreachable!("unresolved actions were rejected above")
            };
            validate_write(self.byte_len, write)?;
            validate_write_source_value(write, *source_value, "resolved symbolic")?;
        }

        let mut staged = destination[..self.byte_len].to_vec();
        for action in &self.actions {
            let MaterializationAction::ResolvedWrite {
                write,
                source_value,
            } = action
            else {
                unreachable!("unresolved actions were rejected above")
            };
            apply_write(&mut staged, self.byte_order, write, *source_value)?;
        }
        destination[..self.byte_len].copy_from_slice(&staged);
        Ok(())
    }
}

pub(crate) fn fixed_layout_byte_len(
    layout: &LayoutPlanReport,
    materialization: &str,
) -> Result<usize, MaterializationDiagnostic> {
    layout
        .size
        .ok_or_else(|| {
            MaterializationDiagnostic(format!(
                "{materialization} requires a fixed-size layout plan"
            ))
        })
        .and_then(|size| {
            usize::try_from(size).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "fixed layout size {size} cannot be represented on this compiler host"
                ))
            })
        })
}

pub fn materialize_scalar_layout_into(
    layout: &LayoutPlanReport,
    values: &[ScalarFieldValue],
    byte_order: ByteOrder,
    destination: &mut [u8],
) -> Result<(), MaterializationDiagnostic> {
    let byte_len = fixed_layout_byte_len(layout, "scalar materialization")?;
    if destination.len() < byte_len {
        return Err(MaterializationDiagnostic(format!(
            "scalar materialization needs {byte_len} bytes, destination has {}",
            destination.len()
        )));
    }
    validate_materialization_field_identities(layout)?;
    let supplied =
        index_supplied_fields(values, "scalar", "supplied", |value: &ScalarFieldValue| {
            (value.field.as_str(), value.member_identity)
        })?;
    validate_supplied_field_coverage(
        layout,
        &supplied,
        "supplied scalar value",
        "supplied scalar field",
    )?;

    let mut staged = vec![0_u8; byte_len];
    for entry in &layout.entries {
        let key = materialization_field_key(&entry.field, entry.member_identity);
        let value = supplied
            .value(&key)
            .expect("complete field set validated above");
        apply_scalar_entry(&mut staged, byte_order, entry, value)?;
    }
    destination[..byte_len].copy_from_slice(&staged);
    Ok(())
}

pub fn materialize_aggregate_layout_into(
    layout: &LayoutPlanReport,
    fields: &[AggregateFieldSchema],
    values: &[AggregateFieldValue],
    byte_order: ByteOrder,
    destination: &mut [u8],
) -> Result<(), MaterializationDiagnostic> {
    let byte_len = fixed_layout_byte_len(layout, "aggregate materialization")?;
    if destination.len() < byte_len {
        return Err(MaterializationDiagnostic(format!(
            "aggregate materialization needs {byte_len} bytes, destination has {}",
            destination.len()
        )));
    }
    let mut staged = vec![0_u8; byte_len];
    stage_aggregate_layout(layout, fields, values, byte_order, &mut staged)?;
    destination[..byte_len].copy_from_slice(&staged);
    Ok(())
}

pub fn stage_aggregate_layout<Bytes: AsRef<[u8]>>(
    layout: &LayoutPlanReport,
    fields: &[AggregateFieldSchema],
    values: &[AggregateFieldValue<Bytes>],
    byte_order: ByteOrder,
    destination: &mut [u8],
) -> Result<(), MaterializationDiagnostic> {
    let byte_len = fixed_layout_byte_len(layout, "aggregate materialization")?;
    if destination.len() < byte_len {
        return Err(MaterializationDiagnostic(format!(
            "aggregate materialization needs {byte_len} bytes, destination has {}",
            destination.len()
        )));
    }
    validate_materialization_field_identities(layout)?;
    let schemas = index_supplied_fields(
        fields,
        "aggregate",
        "declared",
        |field: &AggregateFieldSchema| (field.field.as_str(), field.member_identity),
    )?;
    let supplied = index_supplied_fields(
        values,
        "aggregate",
        "supplied",
        |value: &AggregateFieldValue<Bytes>| (value.field.as_str(), None),
    )?;
    validate_supplied_field_coverage(
        layout,
        &schemas,
        "aggregate schema extent",
        "aggregate schema field",
    )?;
    if let Some(field) = fields
        .iter()
        .find(|field| supplied.named(field.field.as_str()).is_none())
    {
        return Err(MaterializationDiagnostic(format!(
            "layout field `{}` has no supplied aggregate value",
            field.field
        )));
    }
    if let Some(field) = supplied
        .names()
        .find(|field| schemas.named(field).is_none())
    {
        return Err(MaterializationDiagnostic(format!(
            "supplied aggregate field `{field}` has no compiler-derived aggregate schema"
        )));
    }
    let mut planned = planned_entries_by_key(layout);

    let staged = &mut destination[..byte_len];
    staged.fill(0);
    let mut occupied = vec![false; byte_len];
    let mut deferred_fragments = Vec::new();
    for (field_key, schema) in schemas.fields_by_key() {
        let field_name = schema.field.as_str();
        let entries = planned
            .get_mut(field_key)
            .expect("complete aggregate plan set validated above");
        let has_scalar_placement = entries.iter().any(|entry| {
            matches!(
                entry.placement,
                LayoutPlacementReport::IntegerAt { .. } | LayoutPlacementReport::Bits { .. }
            )
        });
        if has_scalar_placement
            && entries
                .iter()
                .any(|entry| matches!(entry.placement, LayoutPlacementReport::At { .. }))
        {
            return Err(MaterializationDiagnostic(format!(
                "aggregate field `{field_name}` mixes `At` with scalar placements"
            )));
        }
        if has_scalar_placement {
            if !matches!(schema.shape, AggregateFieldShape::Whole) {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` carries scalar placements but is an outer fixed array"
                )));
            }
            if schema.byte_size > 8 {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` extent is {} bytes, past the 64-bit scalar placement carrier",
                    schema.byte_size
                )));
            }
            let value = supplied
                .named(field_name)
                .expect("complete aggregate field set validated above");
            let expected_size = usize::try_from(schema.byte_size).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "aggregate field `{}` extent cannot be represented on this compiler host",
                    field_name
                ))
            })?;
            if value.bytes.as_ref().len() != expected_size {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{}` supplies {} bytes, but its compiler-derived extent is {expected_size}",
                    field_name,
                    value.bytes.as_ref().len()
                )));
            }
            let scalar = ScalarFieldValue {
                field: value.field.clone(),
                member_identity: schema.member_identity,
                width_bits: u16::try_from(expected_size * 8)
                    .expect("the eight-byte carrier bound keeps the extent inside u16"),
                value: read_container(value.bytes.as_ref(), byte_order),
            };
            for entry in entries.iter() {
                match entry.placement {
                    LayoutPlacementReport::IntegerAt { .. } => {
                        let fragment = scalar_fragment(entry, scalar.width_bits)?;
                        let start =
                            usize::try_from(fragment.container_byte_offset).map_err(|_| {
                                MaterializationDiagnostic(format!(
                                    "aggregate field `{field_name}` stored-integer offset cannot be represented on this compiler host"
                                ))
                            })?;
                        let end = start
                            .checked_add(usize::from(fragment.container_width_bits / 8))
                            .ok_or_else(|| {
                                MaterializationDiagnostic(format!(
                                    "aggregate field `{field_name}` stored-integer range overflows"
                                ))
                            })?;
                        if end > byte_len {
                            return Err(MaterializationDiagnostic(format!(
                                "aggregate field `{field_name}` writes through byte {end}, past the {byte_len}-byte layout"
                            )));
                        }
                        if occupied[start..end].iter().any(|claimed| *claimed) {
                            return Err(MaterializationDiagnostic(format!(
                                "aggregate field `{field_name}` stored-integer placement overlaps an earlier placement"
                            )));
                        }
                        apply_scalar_entry(staged, byte_order, entry, &scalar)?;
                        occupied[start..end].fill(true);
                    }
                    LayoutPlacementReport::Bits { .. } => {
                        deferred_fragments.push((*entry, scalar.clone()));
                    }
                    LayoutPlacementReport::At { .. } => {
                        unreachable!("mixed `At` and scalar placements rejected above")
                    }
                }
            }
            continue;
        }
        let value = supplied
            .named(field_name)
            .expect("complete aggregate field set validated above");
        let expected_size = usize::try_from(schema.byte_size).map_err(|_| {
            MaterializationDiagnostic(format!(
                "aggregate field `{}` extent cannot be represented on this compiler host",
                field_name
            ))
        })?;
        if value.bytes.as_ref().len() != expected_size {
            return Err(MaterializationDiagnostic(format!(
                "aggregate field `{}` supplies {} bytes, but its compiler-derived extent is {expected_size}",
                field_name,
                value.bytes.as_ref().len()
            )));
        }

        entries.sort_unstable_by_key(|entry| match entry.placement {
            LayoutPlacementReport::At { offset } => offset,
            _ => unreachable!("non-At aggregate entries rejected above"),
        });
        let (source_chunk_size, required_align) = if entries.len() == 1 {
            (expected_size, None)
        } else {
            let AggregateFieldShape::Repeated {
                element_byte_size,
                element_align,
                element_count,
            } = schema.shape
            else {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` has more than one `At` placement but is not an outer fixed array"
                )));
            };
            let actual_count = u64::try_from(entries.len()).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` placement count cannot be represented as u64"
                ))
            })?;
            if actual_count != element_count {
                return Err(MaterializationDiagnostic(format!(
                    "repeated aggregate field `{field_name}` has {actual_count} element placements, expected {element_count}"
                )));
            }
            let element_byte_size = usize::try_from(element_byte_size).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` element extent cannot be represented on this compiler host"
                ))
            })?;
            let offsets = entries
                .iter()
                .map(|entry| match entry.placement {
                    LayoutPlacementReport::At { offset } => offset,
                    _ => unreachable!("non-At aggregate entries rejected above"),
                })
                .collect::<Vec<_>>();
            let stride = offsets[1].checked_sub(offsets[0]).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "repeated aggregate field `{field_name}` element offsets are not ordered"
                ))
            })?;
            if stride < element_byte_size as u64
                || offsets.windows(2).any(|pair| pair[1] - pair[0] != stride)
            {
                return Err(MaterializationDiagnostic(format!(
                    "repeated aggregate field `{field_name}` element placements do not have one nonoverlapping constant stride"
                )));
            }
            (element_byte_size, Some(element_align))
        };

        let chunks = value.bytes.as_ref().chunks_exact(source_chunk_size);
        if !chunks.remainder().is_empty() || chunks.len() != entries.len() {
            return Err(MaterializationDiagnostic(format!(
                "aggregate field `{field_name}` bytes do not tile its compiler-derived source elements exactly"
            )));
        }
        for (entry, source) in entries.iter().zip(chunks) {
            let LayoutPlacementReport::At { offset } = entry.placement else {
                unreachable!("non-At aggregate entries rejected above")
            };
            if required_align.is_some_and(|align| !offset.is_multiple_of(align)) {
                return Err(MaterializationDiagnostic(format!(
                    "repeated aggregate field `{field_name}` element offset {offset} violates its compiler-derived alignment {}",
                    required_align.expect("checked as present")
                )));
            }
            let start = usize::try_from(offset).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` offset cannot be represented on this compiler host"
                ))
            })?;
            let end = start.checked_add(source.len()).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` destination range overflows"
                ))
            })?;
            if end > byte_len {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` writes through byte {end}, past the {byte_len}-byte layout"
                )));
            }
            if occupied[start..end].iter().any(|occupied| *occupied) {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` overlaps an earlier aggregate placement"
                )));
            }
            staged[start..end].copy_from_slice(source);
            occupied[start..end].fill(true);
        }
    }
    let mut fragment_destinations = std::collections::BTreeMap::<usize, u8>::new();
    for (entry, scalar) in deferred_fragments {
        let fragment = scalar_fragment(entry, scalar.width_bits)?;
        for bit in fragment.destination_lsb..fragment.destination_lsb + fragment.width {
            let relative_byte = match byte_order {
                ByteOrder::LittleEndian => bit / 8,
                ByteOrder::BigEndian => fragment.container_width_bits / 8 - 1 - bit / 8,
            };
            let physical_byte = fragment
                .container_byte_offset
                .checked_add(u64::from(relative_byte))
                .ok_or_else(|| {
                    MaterializationDiagnostic(format!(
                        "aggregate field `{}` fragment destination range overflows",
                        entry.field
                    ))
                })?;
            let byte = usize::try_from(physical_byte)
                .map_err(|_| {
                    MaterializationDiagnostic(format!(
                        "aggregate field `{}` fragment destination cannot be represented on this compiler host",
                        entry.field
                    ))
                })?;
            let Some(whole_byte_claimed) = occupied.get(byte) else {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{}` fragment writes byte {byte}, past the {byte_len}-byte layout",
                    entry.field
                )));
            };
            if *whole_byte_claimed {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{}` fragment destination overlaps a whole-extent placement",
                    entry.field
                )));
            }
            let mask = 1_u8 << (bit % 8);
            let applied = fragment_destinations.entry(byte).or_default();
            if *applied & mask != 0 {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{}` fragment destination overlaps an earlier fragment",
                    entry.field
                )));
            }
            *applied |= mask;
        }
        apply_scalar_entry(staged, byte_order, entry, &scalar)?;
    }
    Ok(())
}

pub fn decode_scalar_layout(
    layout: &LayoutPlanReport,
    fields: &[ScalarFieldSchema],
    byte_order: ByteOrder,
    source: &[u8],
) -> Result<Vec<ScalarFieldValue>, MaterializationDiagnostic> {
    let byte_len = layout
        .size
        .ok_or_else(|| {
            MaterializationDiagnostic("scalar decoding requires a fixed-size layout plan".into())
        })
        .and_then(|size| {
            usize::try_from(size).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "fixed layout size {size} cannot be represented on this compiler host"
                ))
            })
        })?;
    if source.len() < byte_len {
        return Err(MaterializationDiagnostic(format!(
            "scalar decoding needs {byte_len} bytes, source has {}",
            source.len()
        )));
    }
    validate_materialization_field_identities(layout)?;
    let supplied =
        index_supplied_fields(fields, "scalar", "declared", |field: &ScalarFieldSchema| {
            (field.field.as_str(), field.member_identity)
        })?;
    validate_supplied_field_coverage(
        layout,
        &supplied,
        "scalar decode schema",
        "scalar decode field",
    )?;
    let mut decoded = supplied
        .fields_by_key()
        .map(|(key, field)| (key.clone(), (field, 0_u64, 0_u64)))
        .collect::<std::collections::BTreeMap<_, _>>();

    for entry in &layout.entries {
        let key = materialization_field_key(&entry.field, entry.member_identity);
        let (field, value, covered) = decoded
            .get_mut(&key)
            .expect("complete field set validated above");
        let width_bits = field.width_bits;
        let fragment = scalar_fragment(entry, width_bits)?;
        validate_fragment(
            byte_len,
            &entry.field,
            fragment.container_byte_offset,
            fragment.container_width_bits,
            fragment.destination_lsb,
            fragment.source_lsb,
            fragment.width,
        )?;
        if let LayoutPlacementReport::IntegerAt {
            stored_width,
            interpretation,
            ..
        } = entry.placement
        {
            let stored_width = u16::try_from(stored_width).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "scalar field `{}` has an invalid stored-integer width",
                    entry.field
                ))
            })?;
            if !byte_sized_width(stored_width) || width_bits < stored_width {
                return Err(MaterializationDiagnostic(format!(
                    "scalar field `{}` cannot decode {stored_width}-bit stored-integer storage into its {}-bit semantic carrier",
                    entry.field, width_bits
                )));
            }
            if *covered != 0 {
                return Err(MaterializationDiagnostic(format!(
                    "scalar field `{}` has more than one stored-integer decode entry",
                    entry.field
                )));
            }
            let container_bytes = usize::from(fragment.container_width_bits / 8);
            let start = usize::try_from(fragment.container_byte_offset).map_err(|_| {
                MaterializationDiagnostic(
                    "stored-integer offset cannot be represented on this host".into(),
                )
            })?;
            let end = start.checked_add(container_bytes).ok_or_else(|| {
                MaterializationDiagnostic("stored-integer byte range overflows".into())
            })?;
            let stored = read_container(&source[start..end], byte_order) & low_mask(stored_width);
            *value = match interpretation {
                IntegerInterpretation::Unsigned => stored,
                IntegerInterpretation::Signed if stored & (1_u64 << (stored_width - 1)) != 0 => {
                    stored | (low_mask(width_bits) & !low_mask(stored_width))
                }
                IntegerInterpretation::Signed => stored,
            };
            *covered = low_mask(width_bits);
            continue;
        }
        let source_mask = low_mask(fragment.width) << fragment.source_lsb;
        if *covered & source_mask != 0 {
            return Err(MaterializationDiagnostic(format!(
                "scalar field `{}` decode fragments overlap in the logical source",
                entry.field
            )));
        }
        let container_bytes = usize::from(fragment.container_width_bits / 8);
        let start = usize::try_from(fragment.container_byte_offset).map_err(|_| {
            MaterializationDiagnostic("container offset cannot be represented on this host".into())
        })?;
        let end = start
            .checked_add(container_bytes)
            .ok_or_else(|| MaterializationDiagnostic("container byte range overflows".into()))?;
        let container = read_container(&source[start..end], byte_order);
        let value_fragment = (container >> fragment.destination_lsb) & low_mask(fragment.width);
        *value |= value_fragment << fragment.source_lsb;
        *covered |= source_mask;
    }

    decoded
        .into_iter()
        .map(|(_, (field, value, covered))| {
            let width_bits = field.width_bits;
            if covered != low_mask(width_bits) {
                return Err(MaterializationDiagnostic(format!(
                    "scalar field `{}` decode fragments do not tile its complete {width_bits}-bit source",
                    field.field
                )));
            }
            Ok(ScalarFieldValue {
                field: field.field.clone(),
                member_identity: field.member_identity,
                width_bits,
                value,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializationDiagnostic(pub String);

impl std::fmt::Display for MaterializationDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for MaterializationDiagnostic {}
