/*
We verify symbolic and conventional layout materialization, nested record and
sum interior path resolution, bit-fragment writer validation, and replay
fingerprint stability.
*/

use layout_plans::test_support::{
    data, deeply_nested_layout, entry, layout_field_entry, layout_report,
    numbered_layout_field_entry, post_handoff_context, post_handoff_context_with, record_layout,
    recursive_record_array_report, repeated_layout, split_layout, stored_integer_layout,
    sum_layout, symbolic_path, writer_field_offsets,
};

mod conventional_materializations {
    use super::{
        entry, layout_field_entry, layout_report, numbered_layout_field_entry,
        post_handoff_context, split_layout,
    };
    use crate::storage_materialization::{
        decode_scalar_layout, derive_symbolic_materialization, materialize_aggregate_layout_into,
        materialize_scalar_layout_into,
    };
    use layout_plans::{
        AggregateFieldSchema, AggregateFieldValue, ByteOrder, ConventionalSumCaseLayoutReport,
        ConventionalSumLayoutReport, ConventionalSumPayloadFieldLayoutReport, EntryStubId,
        IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
        MaterializationAction, RelocationTarget, ScalarFieldSchema, ScalarFieldValue,
        SymbolicFieldValue, conventional_sum_layout_reports_match_for_replay,
        layout_plan_reports_match_for_replay,
        normalized_conventional_sum_layout_report_fingerprint,
        normalized_layout_plan_report_fingerprint,
    };

    macro_rules! bits {
        ($container:literal => $width:literal => $destination:literal => $source:literal => $span:literal) => {
            LayoutPlacementReport::Bits {
                container: $container,
                container_width: $width,
                destination_lsb: $destination,
                source_lsb: $source,
                width: $span,
            }
        };
    }

    fn ready_sum_report(payload_identity: Option<u64>) -> ConventionalSumLayoutReport {
        ConventionalSumLayoutReport {
            schema_report_fingerprint: 7,
            tag_offset: 0,
            tag_size: 4,
            tag_align: 4,
            common_fields: Vec::new(),
            cases: vec![ConventionalSumCaseLayoutReport {
                case: "Ready".into(),
                member_identity: Some(41),
                ordinal: 0,
                payload_fields: vec![ConventionalSumPayloadFieldLayoutReport {
                    field: "value".into(),
                    member_identity: payload_identity,
                    offset: 4,
                    size: 4,
                    align: 4,
                }],
            }],
            size: 8,
            align: 4,
        }
    }

    fn assert_atomic_aggregate_reject<const N: usize>(
        layout: &LayoutPlanReport,
        fields: &[AggregateFieldSchema],
        values: &[AggregateFieldValue],
        fill: u8,
        expected: &str,
    ) {
        let mut buf = [fill; N];
        let error = materialize_aggregate_layout_into(
            layout,
            fields,
            values,
            ByteOrder::LittleEndian,
            &mut buf,
        )
        .expect_err(expected);
        assert!(error.0.contains(expected), "{error}");
        assert_eq!(buf, [fill; N]);
    }

    #[test]
    fn normalized_layout_report_fingerprint_is_order_independent_and_geometry_bound() {
        let forward = split_layout();
        let mut reversed = forward.clone();
        reversed.entries.reverse();
        assert_eq!(
            normalized_layout_plan_report_fingerprint(&forward),
            normalized_layout_plan_report_fingerprint(&reversed)
        );

        let mut shifted = forward.clone();
        let LayoutPlacementReport::Bits { container, .. } = &mut shifted.entries[0].placement
        else {
            unreachable!("split layout uses bit fragments")
        };
        *container = 4;
        assert_ne!(
            normalized_layout_plan_report_fingerprint(&forward),
            normalized_layout_plan_report_fingerprint(&shifted)
        );
    }

    #[test]
    fn conventional_sum_report_fingerprint_binds_ordinals_geometry_and_unnumbered_names() {
        let baseline = ready_sum_report(None);
        let report_fingerprint = normalized_conventional_sum_layout_report_fingerprint(&baseline);
        assert_ne!(report_fingerprint, 0);

        let mut renamed_numbered_case = baseline.clone();
        renamed_numbered_case.cases[0].case = "Available".into();
        assert_eq!(
            report_fingerprint,
            normalized_conventional_sum_layout_report_fingerprint(&renamed_numbered_case)
        );

        for mutate in [
            (|r: &mut ConventionalSumLayoutReport| r.cases[0].ordinal = 1)
                as fn(&mut ConventionalSumLayoutReport),
            |r| r.cases[0].payload_fields[0].field = "payload".into(),
            |r| r.cases[0].payload_fields[0].offset = 8,
        ] {
            let mut variant = baseline.clone();
            mutate(&mut variant);
            assert_ne!(
                report_fingerprint,
                normalized_conventional_sum_layout_report_fingerprint(&variant)
            );
        }
    }

    #[test]
    fn conventional_sum_replay_uses_numbered_identity_and_exact_geometry() {
        let original = ready_sum_report(Some(52));
        let mut renamed = original.clone();
        renamed.cases[0].case = "Available".into();
        renamed.cases[0].payload_fields[0].field = "payload".into();
        assert!(conventional_sum_layout_reports_match_for_replay(
            &original, &renamed
        ));

        for mutate in [
            (|r: &mut ConventionalSumLayoutReport| r.cases[0].payload_fields[0].offset = 8)
                as fn(&mut ConventionalSumLayoutReport),
            |r| r.cases[0].ordinal = 1,
        ] {
            let mut drifted = renamed.clone();
            mutate(&mut drifted);
            assert!(!conventional_sum_layout_reports_match_for_replay(
                &original, &drifted
            ));
        }

        let mut unnumbered = original.clone();
        unnumbered.cases[0].member_identity = None;
        let mut renamed_unnumbered = unnumbered.clone();
        renamed_unnumbered.cases[0].case = "Available".into();
        assert!(!conventional_sum_layout_reports_match_for_replay(
            &unnumbered,
            &renamed_unnumbered
        ));

        let mut aliased = original;
        aliased.cases.push(aliased.cases[0].clone());
        assert!(!conventional_sum_layout_reports_match_for_replay(
            &aliased, &aliased
        ));
    }

    #[test]
    fn stable_member_identity_makes_source_rename_presentation_only() {
        let mut original = split_layout();
        original.schema_report_fingerprint = 0x44;
        for entry in &mut original.entries {
            entry.member_identity = Some(7);
        }
        let mut renamed = original.clone();
        for entry in &mut renamed.entries {
            entry.field = "renamed_address".into();
        }
        assert_eq!(
            normalized_layout_plan_report_fingerprint(&original),
            normalized_layout_plan_report_fingerprint(&renamed)
        );
        assert!(layout_plan_reports_match_for_replay(&original, &renamed));

        let mut reordered = renamed.clone();
        reordered.entries.reverse();
        assert!(layout_plan_reports_match_for_replay(&original, &reordered));

        let mut shifted = renamed.clone();
        let LayoutPlacementReport::Bits { container, .. } = &mut shifted.entries[0].placement
        else {
            unreachable!("split layout uses bit fragments")
        };
        *container = 4;
        assert!(!layout_plan_reports_match_for_replay(&original, &shifted));

        let mut changed_projection = renamed.clone();
        changed_projection.offsets = Some(vec![0]);
        assert!(!layout_plan_reports_match_for_replay(
            &original,
            &changed_projection
        ));

        let mut aliased = original.clone();
        aliased.entries[0].field = "forged_alias".into();
        assert!(!layout_plan_reports_match_for_replay(&aliased, &aliased));

        let mut changed_schema = renamed;
        changed_schema.schema_report_fingerprint = 0x45;
        assert_ne!(
            normalized_layout_plan_report_fingerprint(&original),
            normalized_layout_plan_report_fingerprint(&changed_schema)
        );
        assert!(!layout_plan_reports_match_for_replay(
            &original,
            &changed_schema
        ));

        let positional = split_layout();
        let mut renamed_positional = positional.clone();
        for entry in &mut renamed_positional.entries {
            entry.field = "renamed_address".into();
        }
        assert!(!layout_plan_reports_match_for_replay(
            &positional,
            &renamed_positional
        ));
    }

    #[test]
    fn materializers_reject_layout_identity_aliases_before_observable_work() {
        let mut aliased = split_layout();
        for entry in &mut aliased.entries {
            entry.field = "legacy_address".into();
            entry.member_identity = Some(7);
        }
        aliased.entries[1].field = "forged_alias".into();

        let scalar = ScalarFieldValue::new_numbered("address", 7, 64, 0).expect("numbered scalar");
        let mut scalar_bytes = [0xa5; 16];
        let error = materialize_scalar_layout_into(
            &aliased,
            &[scalar],
            ByteOrder::LittleEndian,
            &mut scalar_bytes,
        )
        .expect_err("scalar materialization must reject one identity under two names");
        assert!(error.0.contains("identity names both"), "{}", error.0);
        assert_eq!(scalar_bytes, [0xa5; 16]);

        let error = decode_scalar_layout(
            &aliased,
            &[ScalarFieldSchema::new_numbered("address", 7, 64).expect("numbered scalar schema")],
            ByteOrder::LittleEndian,
            &[0; 16],
        )
        .expect_err("scalar decode must reject one identity under two names");
        assert!(error.0.contains("identity names both"), "{}", error.0);

        let mut resolutions = 0;
        let symbolic =
            SymbolicFieldValue::new_numbered("address", 7, 64, entry()).expect("numbered target");
        let error =
            derive_symbolic_materialization(&aliased, &[symbolic], post_handoff_context(), |_| {
                resolutions += 1;
                None
            })
            .expect_err("symbolic derivation must reject one identity under two names");
        assert!(error.0.contains("identity names both"), "{}", error.0);
        assert_eq!(resolutions, 0);

        let aggregate_layout = layout_report(
            vec![
                numbered_layout_field_entry(
                    "legacy_payload",
                    9,
                    LayoutPlacementReport::At { offset: 0 },
                ),
                numbered_layout_field_entry(
                    "forged_payload_alias",
                    9,
                    LayoutPlacementReport::At { offset: 4 },
                ),
            ],
            Some(vec![0, 4]),
            Some(8),
            4,
        );
        let schema = [
            AggregateFieldSchema::new_repeated_numbered("payload", 9, 4, 4, 2)
                .expect("numbered repeated aggregate"),
        ];
        let value = [AggregateFieldValue::new("payload", vec![0; 8]).expect("aggregate value")];
        assert_atomic_aggregate_reject::<8>(
            &aggregate_layout,
            &schema,
            &value,
            0xa5,
            "identity names both",
        );
    }

    #[test]
    fn normalized_layout_identity_distinguishes_dynamic_from_full_width_size() {
        let dynamic = LayoutPlanReport {
            schema_report_fingerprint: 1,
            entries: Vec::new(),
            offsets: Some(Vec::new()),
            size: None,
            align: 1,
        };
        let fixed = LayoutPlanReport {
            size: Some(u64::MAX),
            ..dynamic.clone()
        };
        assert_ne!(
            normalized_layout_plan_report_fingerprint(&dynamic),
            normalized_layout_plan_report_fingerprint(&fixed)
        );
    }

    #[test]
    fn owned_aggregate_materializer_places_complete_values_atomically() {
        let layout = layout_report(
            vec![
                layout_field_entry("header", LayoutPlacementReport::At { offset: 4 }),
                layout_field_entry("payload", LayoutPlacementReport::At { offset: 12 }),
            ],
            Some(vec![4, 12]),
            Some(20),
            4,
        );
        let fields = [
            AggregateFieldSchema::new("header", 4).expect("header schema"),
            AggregateFieldSchema::new("payload", 6).expect("payload schema"),
        ];
        let values = [
            AggregateFieldValue::new("header", [1, 2, 3, 4]).expect("header value"),
            AggregateFieldValue::new("payload", [5, 6, 7, 8, 9, 10]).expect("payload value"),
        ];
        let mut bytes = [0xa5; 20];
        materialize_aggregate_layout_into(
            &layout,
            &fields,
            &values,
            ByteOrder::LittleEndian,
            &mut bytes,
        )
        .expect("complete aggregates should materialize through whole At extents");
        assert_eq!(&bytes[4..8], &[1, 2, 3, 4]);
        assert_eq!(&bytes[12..18], &[5, 6, 7, 8, 9, 10]);
        assert!(
            bytes[..4]
                .iter()
                .chain(&bytes[8..12])
                .chain(&bytes[18..])
                .all(|byte| *byte == 0),
            "padding and reserved bytes should be deterministically zeroed"
        );

        let mut short = values.clone();
        short[1] = AggregateFieldValue::new("payload", [5, 6, 7]).expect("short payload");
        assert_atomic_aggregate_reject::<20>(
            &layout,
            &fields,
            &short,
            0xa5,
            "compiler-derived extent is 6",
        );

        let mut fragmented = layout;
        fragmented.entries[0].placement = bits!(4 => 32 => 0 => 0 => 32);
        let mut fragmented_bytes = [0xa5; 20];
        materialize_aggregate_layout_into(
            &fragmented,
            &fields,
            &values,
            ByteOrder::LittleEndian,
            &mut fragmented_bytes,
        )
        .expect("a carrier-sized field may carry a scalar fragment placement");
        assert_eq!(&fragmented_bytes[4..8], &[1, 2, 3, 4]);

        fragmented.entries[0].placement = bits!(12 => 32 => 0 => 0 => 32);
        assert_atomic_aggregate_reject::<20>(
            &fragmented,
            &fields,
            &values,
            0xa5,
            "fragment destination overlaps a whole-extent placement",
        );
    }

    #[test]
    fn numbered_aggregate_materialization_rejoins_renamed_fields_by_identity() {
        let layout = layout_report(
            vec![numbered_layout_field_entry(
                "legacy_payload",
                7,
                LayoutPlacementReport::At { offset: 4 },
            )],
            Some(vec![4]),
            Some(12),
            4,
        );
        let schema =
            [AggregateFieldSchema::new_numbered("payload", 7, 4).expect("numbered schema")];
        let values = [AggregateFieldValue::new("payload", [1, 2, 3, 4]).expect("complete payload")];
        let mut bytes = [0xa5; 12];
        materialize_aggregate_layout_into(
            &layout,
            &schema,
            &values,
            ByteOrder::LittleEndian,
            &mut bytes,
        )
        .expect("stable identity should rejoin a renamed aggregate field");
        assert_eq!(bytes, [0, 0, 0, 0, 1, 2, 3, 4, 0, 0, 0, 0]);

        let mut drifted = layout;
        drifted.entries[0].member_identity = Some(8);
        assert_atomic_aggregate_reject::<12>(
            &drifted,
            &schema,
            &values,
            0x5a,
            "same stable identity",
        );

        let repeated_layout = LayoutPlanReport {
            schema_report_fingerprint: 2,
            entries: [0, 8]
                .into_iter()
                .map(|offset| {
                    numbered_layout_field_entry(
                        "legacy_items",
                        9,
                        LayoutPlacementReport::At { offset },
                    )
                })
                .collect(),
            offsets: None,
            size: Some(16),
            align: 4,
        };
        let repeated_schema = [
            AggregateFieldSchema::new_repeated_numbered("items", 9, 4, 4, 2)
                .expect("repeated schema"),
        ];
        let repeated_values =
            [AggregateFieldValue::new("items", [1, 2, 3, 4, 5, 6, 7, 8])
                .expect("repeated payload")];
        let mut repeated_bytes = [0xa5; 16];
        materialize_aggregate_layout_into(
            &repeated_layout,
            &repeated_schema,
            &repeated_values,
            ByteOrder::LittleEndian,
            &mut repeated_bytes,
        )
        .expect("stable identity should also rejoin renamed fixed-array tiling");
        assert_eq!(
            repeated_bytes,
            [1, 2, 3, 4, 0, 0, 0, 0, 5, 6, 7, 8, 0, 0, 0, 0]
        );
    }

    #[test]
    fn repeated_aggregate_materializer_rejects_invalid_geometry_atomically() {
        let schema =
            [AggregateFieldSchema::new_repeated("items", 4, 4, 3).expect("repeated schema")];
        let values =
            [AggregateFieldValue::new("items", (0_u8..12).collect::<Vec<_>>()).expect("items")];
        fn layout(offsets: &[u64]) -> LayoutPlanReport {
            LayoutPlanReport {
                schema_report_fingerprint: 1,
                entries: offsets
                    .iter()
                    .map(|&offset| {
                        layout_field_entry("items", LayoutPlacementReport::At { offset })
                    })
                    .collect(),
                offsets: None,
                size: Some(32),
                align: 4,
            }
        }

        for (offsets, expected) in [
            (&[0, 8][..], "2 element placements, expected 3"),
            (
                &[0, 8, 20][..],
                "do not have one nonoverlapping constant stride",
            ),
            (
                &[0, 2, 4][..],
                "do not have one nonoverlapping constant stride",
            ),
            (&[0, 6, 12][..], "violates its compiler-derived alignment 4"),
        ] {
            assert_atomic_aggregate_reject::<32>(
                &layout(offsets),
                &schema,
                &values,
                0xa5,
                expected,
            );
        }
    }

    #[test]
    fn aggregate_materialization_applies_scalar_placements_beside_whole_extents() {
        let layout = layout_report(
            vec![
                layout_field_entry(
                    "id",
                    LayoutPlacementReport::IntegerAt {
                        offset: 0,
                        stored_width: 16,
                        interpretation: IntegerInterpretation::Unsigned,
                    },
                ),
                layout_field_entry("flags", bits!(2 => 16 => 0 => 0 => 10)),
                layout_field_entry("pair", LayoutPlacementReport::At { offset: 4 }),
                layout_field_entry("items", LayoutPlacementReport::At { offset: 8 }),
                layout_field_entry("items", LayoutPlacementReport::At { offset: 10 }),
            ],
            None,
            Some(12),
            4,
        );
        let fields = [
            AggregateFieldSchema::new("id", 4).expect("id carrier"),
            AggregateFieldSchema::new("flags", 2).expect("flags carrier"),
            AggregateFieldSchema::new("pair", 2).expect("pair extent"),
            AggregateFieldSchema::new_repeated("items", 2, 2, 2).expect("items shape"),
        ];
        for (order, id_bytes, flag_bytes, expected) in [
            (
                ByteOrder::LittleEndian,
                [0x34, 0x12, 0, 0],
                [0xa5, 0x02],
                [0x34, 0x12, 0xa5, 0x02, 9, 8, 0, 0, 0x11, 0x22, 0x33, 0x44],
            ),
            (
                ByteOrder::BigEndian,
                [0, 0, 0x12, 0x34],
                [0x02, 0xa5],
                [0x12, 0x34, 0x02, 0xa5, 9, 8, 0, 0, 0x11, 0x22, 0x33, 0x44],
            ),
        ] {
            let values = [
                AggregateFieldValue::new("id", id_bytes).expect("id"),
                AggregateFieldValue::new("flags", flag_bytes).expect("flags"),
                AggregateFieldValue::new("pair", [9, 8]).expect("pair"),
                AggregateFieldValue::new("items", [0x11, 0x22, 0x33, 0x44]).expect("items"),
            ];
            let mut bytes = [0x5a; 12];
            materialize_aggregate_layout_into(&layout, &fields, &values, order, &mut bytes)
                .expect("scalar placements should compose with whole extents");
            assert_eq!(bytes, expected);
        }
    }

    #[test]
    fn aggregate_materialization_shares_fragment_bytes_but_rejects_bit_drift() {
        let shared = layout_report(
            vec![
                layout_field_entry("low", bits!(4 => 8 => 0 => 0 => 4)),
                layout_field_entry("high", bits!(4 => 8 => 4 => 0 => 4)),
            ],
            None,
            Some(8),
            1,
        );
        let fields = [
            AggregateFieldSchema::new("low", 1).expect("low carrier"),
            AggregateFieldSchema::new("high", 1).expect("high carrier"),
        ];
        let values = [
            AggregateFieldValue::new("low", [0x0b]).expect("low"),
            AggregateFieldValue::new("high", [0x0d]).expect("high"),
        ];
        let mut bytes = [0x5a; 8];
        materialize_aggregate_layout_into(
            &shared,
            &fields,
            &values,
            ByteOrder::LittleEndian,
            &mut bytes,
        )
        .expect("disjoint destination bits may share one container byte");
        assert_eq!(bytes[4], 0xdb);
        assert!(bytes.iter().enumerate().all(|(i, b)| i == 4 || *b == 0));

        for (placement, expected) in [
            (
                bits!(4 => 8 => 2 => 0 => 4),
                "fragment destination overlaps an earlier fragment",
            ),
            (
                bits!(7 => 16 => 8 => 0 => 4),
                "writes byte 8, past the 8-byte layout",
            ),
        ] {
            let mut bad = shared.clone();
            bad.entries[1].placement = placement;
            assert_atomic_aggregate_reject::<8>(&bad, &fields, &values, 0x5a, expected);
        }
    }

    #[test]
    fn aggregate_fragment_overlap_uses_physical_bytes_in_both_byte_orders() {
        let fields = [
            AggregateFieldSchema::new("whole", 1).expect("whole byte"),
            AggregateFieldSchema::new("fragment", 1).expect("fragment byte"),
        ];
        let values = [
            AggregateFieldValue::new("whole", [0x17]).expect("whole value"),
            AggregateFieldValue::new("fragment", [0x29]).expect("fragment value"),
        ];
        for (byte_order, fragment_byte) in [(ByteOrder::LittleEndian, 0), (ByteOrder::BigEndian, 1)]
        {
            for whole_byte in [fragment_byte, 1 - fragment_byte] {
                let layout = layout_report(
                    vec![
                        layout_field_entry(
                            "whole",
                            LayoutPlacementReport::At { offset: whole_byte },
                        ),
                        layout_field_entry("fragment", bits!(0 => 16 => 0 => 0 => 8)),
                    ],
                    None,
                    Some(2),
                    1,
                );
                let mut destination = [0xa5; 3];
                let result = materialize_aggregate_layout_into(
                    &layout,
                    &fields,
                    &values,
                    byte_order,
                    &mut destination,
                );
                if whole_byte == fragment_byte {
                    assert!(
                        result
                            .expect_err("a fragment must not overwrite another complete field")
                            .0
                            .contains("fragment destination overlaps a whole-extent placement")
                    );
                    assert_eq!(destination, [0xa5; 3], "rejection must not change any byte");
                } else {
                    result.expect("disjoint physical bytes may share a transfer container");
                    assert_eq!(destination[whole_byte as usize], 0x17);
                    assert_eq!(destination[fragment_byte as usize], 0x29);
                    assert_eq!(
                        destination[2], 0xa5,
                        "the unused allocation tail is unchanged"
                    );
                }
            }
        }
    }

    #[test]
    fn aggregate_fragment_overlap_compares_different_container_widths() {
        let fields = [
            AggregateFieldSchema::new("narrow", 1).expect("narrow carrier"),
            AggregateFieldSchema::new("wide", 1).expect("wide carrier"),
        ];
        let values = [
            AggregateFieldValue::new("narrow", [0x17]).expect("narrow value"),
            AggregateFieldValue::new("wide", [0x29]).expect("wide value"),
        ];
        for (byte_order, wide_byte) in [(ByteOrder::LittleEndian, 0), (ByteOrder::BigEndian, 1)] {
            for narrow_byte in [wide_byte, 1 - wide_byte] {
                let layout = layout_report(
                    vec![
                        layout_field_entry(
                            "narrow",
                            LayoutPlacementReport::Bits {
                                container: narrow_byte,
                                container_width: 8,
                                destination_lsb: 0,
                                source_lsb: 0,
                                width: 8,
                            },
                        ),
                        layout_field_entry("wide", bits!(0 => 16 => 0 => 0 => 8)),
                    ],
                    None,
                    Some(2),
                    1,
                );
                let mut destination = [0xa5; 2];
                let result = materialize_aggregate_layout_into(
                    &layout,
                    &fields,
                    &values,
                    byte_order,
                    &mut destination,
                );
                if narrow_byte == wide_byte {
                    assert!(
                        result
                            .expect_err(
                                "different containers cannot overwrite the same physical bit"
                            )
                            .0
                            .contains("fragment destination overlaps an earlier fragment")
                    );
                    assert_eq!(destination, [0xa5; 2]);
                } else {
                    result.expect("disjoint fragment bytes may have overlapping container ranges");
                    assert_eq!(destination[narrow_byte as usize], 0x17);
                    assert_eq!(destination[wide_byte as usize], 0x29);
                }
            }
        }
    }

    #[test]
    fn aggregate_materialization_rejects_scalar_placement_misuse_atomically() {
        let fields = [
            AggregateFieldSchema::new("id", 4).expect("id carrier"),
            AggregateFieldSchema::new_repeated("items", 2, 2, 2).expect("items shape"),
            AggregateFieldSchema::new("wide", 16).expect("wide extent"),
            AggregateFieldSchema::new("zz", 4).expect("zz carrier"),
        ];
        let values = [
            AggregateFieldValue::new("id", [0x34, 0x12, 0, 0]).expect("id"),
            AggregateFieldValue::new("items", [1, 2, 3, 4]).expect("items"),
            AggregateFieldValue::new("wide", [7; 16]).expect("wide"),
            AggregateFieldValue::new("zz", [9, 9, 9, 9]).expect("zz"),
        ];
        let mut overflow_values = values.clone();
        overflow_values[0] = AggregateFieldValue::new("id", [0, 0, 1, 0]).expect("id");
        let base = |entries: Vec<LayoutFieldEntryReport>| LayoutPlanReport {
            schema_report_fingerprint: 1,
            entries,
            offsets: None,
            size: Some(32),
            align: 4,
        };
        let at = |name, offset| layout_field_entry(name, LayoutPlacementReport::At { offset });
        let int_u16 = |name, offset| {
            layout_field_entry(
                name,
                LayoutPlacementReport::IntegerAt {
                    offset,
                    stored_width: 16,
                    interpretation: IntegerInterpretation::Unsigned,
                },
            )
        };

        let cases = [
            (
                vec![
                    at("id", 0),
                    layout_field_entry("id", bits!(4 => 8 => 0 => 0 => 8)),
                    at("items", 8),
                    at("items", 10),
                    at("wide", 16),
                    at("zz", 24),
                ],
                &values,
                "mixes `At` with scalar placements",
            ),
            (
                vec![
                    at("id", 0),
                    layout_field_entry("items", bits!(4 => 8 => 0 => 0 => 8)),
                    at("wide", 16),
                    at("zz", 24),
                ],
                &values,
                "carries scalar placements but is an outer fixed array",
            ),
            (
                vec![
                    at("id", 0),
                    at("items", 8),
                    at("items", 10),
                    layout_field_entry("wide", bits!(16 => 64 => 0 => 0 => 64)),
                    at("zz", 24),
                ],
                &values,
                "past the 64-bit scalar placement carrier",
            ),
            (
                vec![
                    at("id", 0),
                    at("items", 8),
                    at("items", 10),
                    at("wide", 16),
                    int_u16("zz", 8),
                ],
                &values,
                "stored-integer placement overlaps an earlier placement",
            ),
            (
                vec![
                    int_u16("id", 0),
                    at("items", 8),
                    at("items", 10),
                    at("wide", 16),
                    at("zz", 24),
                ],
                &overflow_values,
                "does not fit its 16-bit unsigned storage",
            ),
        ];
        for (entries, vals, expected) in cases {
            assert_atomic_aggregate_reject::<32>(&base(entries), &fields, vals, 0xa5, expected);
        }
    }

    #[test]
    fn numbered_scalar_materialization_and_decode_rejoin_renamed_fields_by_identity() {
        let layout = layout_report(
            vec![
                numbered_layout_field_entry("legacy_counter", 7, bits!(0 => 8 => 0 => 0 => 8)),
                numbered_layout_field_entry("legacy_counter", 7, bits!(1 => 8 => 0 => 8 => 8)),
                numbered_layout_field_entry(
                    "legacy_status",
                    9,
                    LayoutPlacementReport::At { offset: 2 },
                ),
            ],
            None,
            Some(4),
            2,
        );
        let values = [
            ScalarFieldValue::new_numbered("counter", 7, 16, 0x1234).expect("numbered counter"),
            ScalarFieldValue::new_numbered("status", 9, 16, 0xabcd).expect("numbered status"),
        ];
        let mut bytes = [0xa5; 4];
        materialize_scalar_layout_into(&layout, &values, ByteOrder::LittleEndian, &mut bytes)
            .expect("stable identities should rejoin renamed scalar fields and fragments");
        assert_eq!(bytes, [0x34, 0x12, 0xcd, 0xab]);

        let decoded = decode_scalar_layout(
            &layout,
            &[
                ScalarFieldSchema::new_numbered("counter", 7, 16).expect("numbered counter schema"),
                ScalarFieldSchema::new_numbered("status", 9, 16).expect("numbered status schema"),
            ],
            ByteOrder::LittleEndian,
            &bytes,
        )
        .expect("stable identities should decode through current schema spellings");
        let decoded = decoded
            .iter()
            .map(|value| (value.field.as_str(), (value.member_identity, value.value)))
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(decoded["counter"], (Some(7), 0x1234));
        assert_eq!(decoded["status"], (Some(9), 0xabcd));

        let mut drifted = layout.clone();
        drifted.entries[1].member_identity = Some(8);
        let mut unchanged = [0x5a; 4];
        let error = materialize_scalar_layout_into(
            &drifted,
            &values,
            ByteOrder::LittleEndian,
            &mut unchanged,
        )
        .expect_err("fragment identity drift must reject before destination mutation");
        assert!(error.0.contains("same stable identity"));
        assert_eq!(unchanged, [0x5a; 4]);

        let error = decode_scalar_layout(
            &drifted,
            &[ScalarFieldSchema::new_numbered("counter", 7, 16).expect("numbered counter schema")],
            ByteOrder::LittleEndian,
            &bytes,
        )
        .expect_err("decode identity drift must reject before exposing partial values");
        assert!(error.0.contains("same stable identity"));

        let duplicate_identity = ScalarFieldValue::new_numbered("alias", 7, 16, 0)
            .expect("second spelling with the same identity");
        let error = materialize_scalar_layout_into(
            &layout,
            &[values[0].clone(), duplicate_identity],
            ByteOrder::LittleEndian,
            &mut unchanged,
        )
        .expect_err("one stable identity cannot name two supplied scalar values");
        assert!(error.0.contains("repeats stable member identity #7"));

        let error = decode_scalar_layout(
            &layout,
            &[
                ScalarFieldSchema::new_numbered("counter", 7, 16).expect("numbered counter schema"),
                ScalarFieldSchema::new_numbered("alias", 7, 16).expect("duplicate numbered schema"),
            ],
            ByteOrder::LittleEndian,
            &bytes,
        )
        .expect_err("one stable identity cannot name two scalar decode schemas");
        assert!(error.0.contains("repeats stable member identity #7"));
    }

    #[test]
    fn ordinary_scalar_materializer_round_trips_stored_integers() {
        let layout = layout_report(
            vec![
                layout_field_entry(
                    "signed",
                    LayoutPlacementReport::IntegerAt {
                        offset: 0,
                        stored_width: 8,
                        interpretation: IntegerInterpretation::Signed,
                    },
                ),
                layout_field_entry(
                    "unsigned",
                    LayoutPlacementReport::IntegerAt {
                        offset: 1,
                        stored_width: 16,
                        interpretation: IntegerInterpretation::Unsigned,
                    },
                ),
            ],
            None,
            Some(3),
            1,
        );
        let values = [
            ScalarFieldValue::new("signed", 64, (-9_i64) as u64).expect("signed"),
            ScalarFieldValue::new("unsigned", 64, 0x1234).expect("unsigned"),
        ];

        let mut little_endian = [0xa5_u8; 3];
        materialize_scalar_layout_into(
            &layout,
            &values,
            ByteOrder::LittleEndian,
            &mut little_endian,
        )
        .expect("proved-fit stored integers should materialize");
        assert_eq!(little_endian, [0xf7, 0x34, 0x12]);

        let decoded = decode_scalar_layout(
            &layout,
            &[
                ScalarFieldSchema::new("signed", 64).expect("signed schema"),
                ScalarFieldSchema::new("unsigned", 64).expect("unsigned schema"),
            ],
            ByteOrder::LittleEndian,
            &little_endian,
        )
        .expect("stored integers should decode into their semantic carriers");
        let decoded = decoded
            .iter()
            .map(|field| (field.field.as_str(), field.value))
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(decoded["signed"], (-9_i64) as u64);
        assert_eq!(decoded["unsigned"], 0x1234);

        let mut big_endian = [0xa5_u8; 3];
        materialize_scalar_layout_into(&layout, &values, ByteOrder::BigEndian, &mut big_endian)
            .expect("stored integers should honor the selected byte order");
        assert_eq!(big_endian, [0xf7, 0x12, 0x34]);
    }

    #[test]
    fn ordinary_scalar_stored_integer_write_is_fit_checked_and_atomic() {
        let layout = layout_report(
            vec![layout_field_entry(
                "value",
                LayoutPlacementReport::IntegerAt {
                    offset: 0,
                    stored_width: 8,
                    interpretation: IntegerInterpretation::Signed,
                },
            )],
            None,
            Some(1),
            1,
        );
        let mut bytes = [0xa5_u8];
        for (width, val, expected) in [(64, 128, "does not fit"), (4, 7, "narrower")] {
            let error = materialize_scalar_layout_into(
                &layout,
                &[ScalarFieldValue::new("value", width, val).expect("scalar value")],
                ByteOrder::LittleEndian,
                &mut bytes,
            )
            .expect_err(expected);
            assert!(error.0.contains(expected), "{}", error.0);
            assert_eq!(bytes, [0xa5]);
        }
    }

    #[test]
    fn scalar_materialization_is_complete_and_atomic() {
        let layout = layout_report(
            vec![
                layout_field_entry("low", bits!(0 => 8 => 0 => 0 => 4)),
                layout_field_entry("high", bits!(0 => 8 => 4 => 0 => 4)),
            ],
            None,
            Some(1),
            1,
        );
        let mut bytes = [0xa5_u8];
        let low = ScalarFieldValue::new("low", 4, 3).expect("low");
        for (vals, expected) in [
            (vec![low.clone()], "`high`"),
            (vec![low.clone(), low], "more than once"),
        ] {
            let error =
                materialize_scalar_layout_into(&layout, &vals, ByteOrder::LittleEndian, &mut bytes)
                    .expect_err(expected);
            assert!(error.0.contains(expected));
            assert_eq!(bytes, [0xa5]);
        }

        let error = decode_scalar_layout(
            &layout,
            &[ScalarFieldSchema::new("low", 4).expect("low")],
            ByteOrder::LittleEndian,
            &bytes,
        )
        .expect_err("an imported scan also requires the complete schema");
        assert!(error.0.contains("`high`"));
    }

    #[test]
    fn symbolic_write_geometry_rejects_before_any_resolver_invocation() {
        let layout = layout_report(
            vec![
                layout_field_entry("first", LayoutPlacementReport::At { offset: 0 }),
                layout_field_entry("second", LayoutPlacementReport::At { offset: 16 }),
            ],
            Some(vec![0, 16]),
            Some(16),
            8,
        );
        let symbolic_fields = [
            SymbolicFieldValue::new("first", 64, entry()).expect("first symbolic field"),
            SymbolicFieldValue::new("second", 64, entry()).expect("second symbolic field"),
        ];
        let mut resolutions = 0;
        let error = derive_symbolic_materialization(
            &layout,
            &symbolic_fields,
            post_handoff_context(),
            |_| {
                resolutions += 1;
                Some(0)
            },
        )
        .expect_err("out-of-range writer geometry must reject during static preflight");
        assert!(error.0.contains("writes outside"), "{}", error.0);
        assert_eq!(
            resolutions, 0,
            "no provider/compiler resolver runs before all static writer geometry validates"
        );
    }

    #[test]
    fn symbolic_value_constraints_reject_before_unrelated_target_resolution() {
        let first = entry();
        let unrelated = RelocationTarget::Entry(
            EntryStubId::from_normalized_identity(0x66bb).expect("unrelated entry identity"),
        );
        let layout = layout_report(
            vec![
                layout_field_entry("first", LayoutPlacementReport::At { offset: 0 }),
                layout_field_entry("unrelated", LayoutPlacementReport::At { offset: 8 }),
                layout_field_entry(
                    "narrow",
                    LayoutPlacementReport::IntegerAt {
                        offset: 16,
                        stored_width: 8,
                        interpretation: IntegerInterpretation::Unsigned,
                    },
                ),
            ],
            None,
            Some(24),
            8,
        );
        let symbolic_fields = [
            SymbolicFieldValue::new("first", 64, first).expect("first symbolic field"),
            SymbolicFieldValue::new("unrelated", 64, unrelated).expect("unrelated symbolic field"),
            SymbolicFieldValue::new("narrow", 64, first).expect("narrow symbolic field"),
        ];
        let mut resolutions = Vec::new();
        let error = derive_symbolic_materialization(
            &layout,
            &symbolic_fields,
            post_handoff_context(),
            |target| {
                resolutions.push(target);
                Some(if target == first { 0x100 } else { 0 })
            },
        )
        .expect_err("one target must satisfy all retained writes before derivation continues");
        assert!(error.0.contains("does not fit"), "{}", error.0);
        assert_eq!(resolutions, vec![first]);

        resolutions.clear();
        let plan = derive_symbolic_materialization(
            &layout,
            &symbolic_fields,
            post_handoff_context(),
            |target| {
                resolutions.push(target);
                Some(if target == first { 0x7f } else { 0 })
            },
        )
        .expect("one fitting value serves every retained write for its exact target");
        assert_eq!(resolutions, vec![first, unrelated]);
        assert_eq!(
            plan.actions
                .iter()
                .map(|action| match action {
                    MaterializationAction::ResolvedWrite { source_value, .. } => *source_value,
                    action => panic!("resolved derivation produced {action:?}"),
                })
                .collect::<Vec<_>>(),
            vec![0x7f, 0, 0x7f]
        );
    }
}
mod symbolic_materializations {
    use super::{
        assert_reject_inner, data, deeply_nested_layout, entry, layout_field_entry, nested_layout,
        numbered_layout_field_entry, post_handoff_context, post_handoff_context_with,
        record_interior, record_layout, repeated_layout, split_layout, sym_path,
    };
    use crate::storage_materialization::derive_symbolic_materialization;
    use layout_plans::{
        ByteOrder, ConsumptionInstant, IntegerInterpretation, LayoutPlacementReport,
        LayoutPlanReport, MaterializationAction, MaterializationContext, PlacementAddressRange,
        PlacementConstraints, PlacementPhase, SymbolicFieldInnerLayout, SymbolicFieldPathSegment,
        SymbolicFieldValue,
    };

    #[test]
    fn placement_range_must_fit_the_materialization() {
        let constraints = PlacementConstraints::new(
            Some(PlacementAddressRange::new(0x1000, 0x1008).expect("eight-byte range")),
            1,
            PlacementPhase::PostHandoff,
            None,
            None,
        )
        .expect("placement constraints");
        let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
        let error = derive_symbolic_materialization(
            &split_layout(),
            &[symbolic],
            post_handoff_context_with(constraints),
            |_| None,
        )
        .expect_err("sixteen bytes cannot fit an eight-byte range");
        assert!(error.0.contains("cannot fit"));
    }

    #[test]
    fn symbolic_index_materialization_uses_loader_native_relocation() {
        let symbolic = SymbolicFieldValue::new_indexed_numbered("handlers", 9, 1, 64, entry())
            .expect("indexed field");
        let plan = derive_symbolic_materialization(
            &repeated_layout(),
            &[symbolic],
            MaterializationContext {
                consumption: ConsumptionInstant::BeforeOmegaEntry,
                byte_order: ByteOrder::LittleEndian,
                native_pointer_relocation_bits: Some(64),
                placement: PlacementConstraints::unconstrained(PlacementPhase::Load),
            },
            |_| None,
        )
        .expect("an indexed whole pointer uses the loader-native relocation");

        assert!(matches!(
            plan.actions.as_slice(),
            [MaterializationAction::NativePointerRelocation {
                field,
                destination_byte_offset: 8,
                width_bits: 64,
                ..
            }] if field == "handlers"
        ));
    }

    #[test]
    fn symbolic_index_materialization_rejects_before_target_resolution() {
        let out_of_range = SymbolicFieldValue::new_indexed_numbered("handlers", 9, 4, 64, entry())
            .expect("indexed field");
        let mut resolutions = 0;
        let error = derive_symbolic_materialization(
            &repeated_layout(),
            &[out_of_range],
            post_handoff_context(),
            |_| {
                resolutions += 1;
                None
            },
        )
        .expect_err("an element index beyond the placements must reject");
        assert!(
            error
                .0
                .contains("index 4 is outside its 4 element placements"),
            "{}",
            error.0
        );
        assert_eq!(
            resolutions, 0,
            "the exact index bound rejects before any target resolution"
        );

        let fragmented =
            SymbolicFieldValue::new_indexed("address", 0, 64, entry()).expect("indexed field");
        let error = derive_symbolic_materialization(
            &split_layout(),
            &[fragmented],
            post_handoff_context(),
            |_| None,
        )
        .expect_err("a fragmented field cannot be addressed by element index");
        assert!(
            error.0.contains("fragmented or stored-integer placement"),
            "{}",
            error.0
        );
    }

    #[test]
    fn symbolic_inner_materialization_requires_a_bound_traversed_carrier() {
        let (layout, carrier) = nested_layout();
        let inner_path = sym_path("slot", &["entry"], entry());

        let error = derive_symbolic_materialization(
            &layout,
            std::slice::from_ref(&inner_path),
            post_handoff_context(),
            |_| None,
        )
        .expect_err("an inner hop cannot resolve without its interior carrier");
        assert!(
            error
                .0
                .contains("`slot.entry` has no supplied inner layout"),
            "{}",
            error.0
        );

        let flat = SymbolicFieldValue::new("header", 64, entry()).expect("flat field");
        let orphan = SymbolicFieldInnerLayout::new("missing", record_interior(&carrier));
        let unsized_carrier = SymbolicFieldInnerLayout::new(
            "slot",
            LayoutPlanReport {
                size: None,
                ..record_interior(&carrier)
            },
        );
        for (carriers, fields, expected) in [
            (
                vec![carrier.clone()],
                vec![flat],
                "no symbolic field path traverses the supplied inner layout for `slot`",
            ),
            (
                vec![orphan],
                vec![inner_path.clone()],
                "inner layout for `missing` binds to a field the validated layout plan does not contain",
            ),
            (
                vec![carrier.clone(), carrier],
                vec![inner_path.clone()],
                "inner layout for `slot` is supplied more than once",
            ),
            (
                vec![unsized_carrier],
                vec![inner_path],
                "inner layout for `slot` requires a fixed-size interior layout plan",
            ),
        ] {
            assert_reject_inner(&layout, &carriers, &fields, expected);
        }
    }

    #[test]
    fn symbolic_inner_materialization_rejects_ambiguous_or_missing_members() {
        let (layout, carrier) = nested_layout();
        let repeated_outer = LayoutPlanReport {
            schema_report_fingerprint: 1,
            entries: (0..4)
                .map(|i| {
                    numbered_layout_field_entry(
                        "handlers",
                        9,
                        LayoutPlacementReport::At { offset: i * 16 },
                    )
                })
                .collect(),
            offsets: None,
            size: Some(64),
            align: 8,
        };
        let repeated_carrier =
            SymbolicFieldInnerLayout::new_numbered("handlers", 9, record_interior(&carrier));
        let ambiguous = SymbolicFieldValue::new_numbered("handlers", 9, 64, entry())
            .expect("repeated outer record")
            .with_inner_segment(SymbolicFieldPathSegment::new("flags"));
        assert_reject_inner(
            &repeated_outer,
            &[repeated_carrier],
            &[ambiguous],
            "requires the outer field `handlers` to resolve to exactly one element placement, found 4",
        );

        let integer_outer = LayoutPlanReport {
            schema_report_fingerprint: 1,
            entries: vec![layout_field_entry(
                "address",
                LayoutPlacementReport::IntegerAt {
                    offset: 0,
                    stored_width: 32,
                    interpretation: IntegerInterpretation::Unsigned,
                },
            )],
            offsets: None,
            size: Some(4),
            align: 4,
        };
        let bits_outer = LayoutPlanReport {
            entries: vec![layout_field_entry(
                "address",
                LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 32,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 32,
                },
            )],
            ..integer_outer.clone()
        };
        let addr_carrier = SymbolicFieldInnerLayout::new("address", record_interior(&carrier));
        let through_addr = sym_path("address", &["entry"], entry());
        for bad_outer in [&integer_outer, &bits_outer] {
            assert_reject_inner(
                bad_outer,
                std::slice::from_ref(&addr_carrier),
                std::slice::from_ref(&through_addr),
                "inner layout for `address` requires a whole `At` placement",
            );
        }

        assert_reject_inner(
            &layout,
            std::slice::from_ref(&carrier),
            &[sym_path("slot", &["missing"], entry())],
            "`slot.missing` has no entry in the inner layout plan",
        );
        assert_reject_inner(
            &layout,
            &[carrier],
            &[
                sym_path("slot", &["entry"], entry()),
                sym_path("slot", &["entry"], data()),
            ],
            "`slot.entry` is supplied more than once",
        );
    }

    #[test]
    fn symbolic_inner_materialization_rejects_deeper_path_faults() {
        let (layout, carrier) = deeply_nested_layout();
        let deep_path = |member: &str| sym_path("slot", &["sub", member], entry());
        let shallow = SymbolicFieldInnerLayout::new("slot", record_interior(&carrier));
        let shallow_path = sym_path("slot", &["pad"], entry());
        let stray =
            SymbolicFieldInnerLayout::new("slot", record_interior(&carrier)).with_inner_layout(
                SymbolicFieldInnerLayout::new("missing", record_layout(4, &[], 8)),
            );

        for (carriers, fields, expected) in [
            (
                vec![shallow],
                vec![deep_path("entry")],
                "`slot.sub.entry` has no supplied inner layout for `slot.sub`",
            ),
            (
                vec![carrier.clone()],
                vec![shallow_path.clone()],
                "no symbolic field path traverses the supplied inner layout for `slot.sub`",
            ),
            (
                vec![stray],
                vec![shallow_path],
                "inner layout for `slot.missing` binds to a field the enclosing interior layout plan does not contain",
            ),
            (
                vec![carrier],
                vec![
                    deep_path("entry"),
                    sym_path("slot", &["sub", "entry"], data()),
                ],
                "`slot.sub.entry` is supplied more than once",
            ),
        ] {
            assert_reject_inner(&layout, &carriers, &fields, expected);
        }
    }

    #[test]
    fn symbolic_inner_materialization_bounds_each_record_boundary() {
        let (layout, carrier) = deeply_nested_layout();
        let deep_path = |member: &str| sym_path("slot", &["sub", member], entry());

        let repeated_middle = SymbolicFieldInnerLayout::new(
            "slot",
            LayoutPlanReport {
                schema_report_fingerprint: 2,
                entries: (0..2)
                    .map(|i| {
                        layout_field_entry("subs", LayoutPlacementReport::At { offset: i * 24 })
                    })
                    .collect(),
                offsets: None,
                size: Some(48),
                align: 8,
            },
        )
        .with_inner_layout(SymbolicFieldInnerLayout::new(
            "subs",
            record_layout(3, &[("entry", 0)], 24),
        ));
        let outer_56 = LayoutPlanReport {
            size: Some(56),
            ..layout.clone()
        };
        assert_reject_inner(
            &outer_56,
            &[repeated_middle],
            &[sym_path("slot", &["subs", "entry"], entry())],
            "requires the enclosing field `slot.subs` to resolve to exactly one element placement, found 2",
        );

        let integer_middle = SymbolicFieldInnerLayout::new(
            "slot",
            LayoutPlanReport {
                schema_report_fingerprint: 2,
                entries: vec![layout_field_entry(
                    "sub",
                    LayoutPlacementReport::IntegerAt {
                        offset: 8,
                        stored_width: 64,
                        interpretation: IntegerInterpretation::Unsigned,
                    },
                )],
                offsets: None,
                size: Some(32),
                align: 8,
            },
        )
        .with_inner_layout(SymbolicFieldInnerLayout::new(
            "sub",
            record_interior(&carrier.inner_layouts()[0]),
        ));
        let overflowing_interior =
            SymbolicFieldInnerLayout::new("slot", record_interior(&carrier)).with_inner_layout(
                SymbolicFieldInnerLayout::new("sub", record_layout(3, &[("entry", 0)], 32)),
            );
        let mut deep_escape = record_interior(&carrier.inner_layouts()[0]);
        deep_escape.entries.push(layout_field_entry(
            "escape",
            LayoutPlacementReport::At { offset: 20 },
        ));
        let escaping = SymbolicFieldInnerLayout::new("slot", record_interior(&carrier))
            .with_inner_layout(SymbolicFieldInnerLayout::new("sub", deep_escape));

        for (bad_carrier, leaf, expected) in [
            (
                integer_middle,
                "entry",
                "inner layout for `slot.sub` requires a whole `At` placement",
            ),
            (
                overflowing_interior,
                "entry",
                "interior layout for `slot.sub` exceeds the enclosing 32-byte record extent",
            ),
            (
                escaping,
                "escape",
                "writes outside the 24-byte materialization",
            ),
        ] {
            assert_reject_inner(&layout, &[bad_carrier], &[deep_path(leaf)], expected);
        }
    }
}

mod symbolic_sum_materializations {
    use super::{
        assert_reject_inner, data, deeply_nested_layout, entry, layout_field_entry, nested_layout,
        numbered_layout_field_entry, post_handoff_context, record_interior, record_layout,
        recursive_record_array_report, recursive_sum_report, sum_array_layout, sum_field_layout,
        sum_layout, sym_path, symbolic_path, writer_field_offsets,
    };
    use crate::storage_materialization::{
        derive_symbolic_materialization, derive_symbolic_materialization_with_inner_layouts,
    };
    use layout_plans::{
        CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT, ConventionalRecordSumChildHop,
        ConventionalRecordSumChildInterior, ConventionalRecordSumChildLayoutReport,
        ConventionalRecursiveRecordSumPathsLayoutReport, ConventionalSumLayoutReport,
        LayoutPlacementReport, LayoutPlanReport, MaterializationAction, SymbolicFieldInnerLayout,
        SymbolicFieldPathSegment, SymbolicFieldValue,
    };

    #[test]
    fn empty_interiors_preserve_geometry_without_requiring_a_write_path() {
        let (mut layout, _) = sum_array_layout();
        layout.size = Some(8);
        let carrier = SymbolicFieldInnerLayout::new_sum_array("sums", sum_layout(), 0, 24);
        let value = SymbolicFieldValue::new("header", 64, data()).unwrap();
        let derive = |layout: &LayoutPlanReport, carriers: &[SymbolicFieldInnerLayout]| {
            derive_symbolic_materialization_with_inner_layouts(
                layout,
                carriers,
                std::slice::from_ref(&value),
                post_handoff_context(),
                |_| None,
            )
        };
        derive(&layout, std::slice::from_ref(&carrier)).unwrap();

        for mutation in 0..4 {
            let mut malformed = layout.clone();
            match mutation {
                0 => malformed.entries[1].placement = LayoutPlacementReport::At { offset: 9 },
                1 => malformed.entries.push(malformed.entries[1].clone()),
                2 => {
                    malformed.entries[1].placement = LayoutPlacementReport::IntegerAt {
                        offset: 8,
                        stored_width: 8,
                        interpretation: layout_plans::IntegerInterpretation::Unsigned,
                    }
                }
                3 => malformed.entries[1].member_identity = Some(2),
                _ => unreachable!(),
            }
            assert!(
                derive(&malformed, std::slice::from_ref(&carrier)).is_err(),
                "mutation {mutation}"
            );
        }
        assert!(derive(&layout, &[carrier.clone(), carrier]).is_err());
        assert!(
            derive(
                &layout,
                &[SymbolicFieldInnerLayout::new_sum_array(
                    "sums",
                    sum_layout(),
                    0,
                    16
                )]
            )
            .is_err()
        );
        let mut escaping_sum = sum_layout();
        escaping_sum.cases[1].payload_fields[0].offset = 24;
        assert!(
            derive(
                &layout,
                &[SymbolicFieldInnerLayout::new_sum_array(
                    "sums",
                    escaping_sum,
                    0,
                    24
                )]
            )
            .is_err()
        );

        let interior = record_layout(2, &[("choice", 0)], 24);
        let record_carrier = |interior| {
            SymbolicFieldInnerLayout::new_record_array("sums", interior, 0, 24)
                .with_inner_layout(SymbolicFieldInnerLayout::new_sum("choice", sum_layout()))
        };
        derive(&layout, &[record_carrier(interior.clone())]).unwrap();
        let mut escaping_interior = interior;
        escaping_interior.entries[0].placement = LayoutPlacementReport::At { offset: 8 };
        assert!(
            derive(&layout, &[record_carrier(escaping_interior)]).is_err(),
            "a malformed descendant cannot hide behind an empty ancestor"
        );
    }

    #[test]
    fn symbolic_inner_materialization_bounds_record_path_depth() {
        let (layout, carrier) = deeply_nested_layout();
        let make_chain = |count: usize| {
            let mut chain = SymbolicFieldPathSegment::new("leaf");
            for _ in 1..count {
                chain = SymbolicFieldPathSegment::new("sub").with_inner_segment(chain);
            }
            SymbolicFieldValue::new("slot", 64, entry())
                .expect("outer record field")
                .with_inner_segment(chain)
        };
        assert_reject_inner(
            &layout,
            std::slice::from_ref(&carrier),
            &[make_chain(CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT - 1)],
            "has no entry in the inner layout plan",
        );
        assert_reject_inner(
            &layout,
            &[carrier],
            &[make_chain(CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT)],
            "exceeds the compiler's 64-segment record path bound",
        );

        let self_similar =
            || SymbolicFieldInnerLayout::new("slot", record_layout(2, &[("slot", 0)], 8));
        let mut tree = self_similar();
        for _ in 1..CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT + 1 {
            tree = self_similar().with_inner_layout(tree);
        }
        assert_reject_inner(
            &layout,
            &[tree],
            &[sym_path("slot", &["slot"], entry())],
            "nests beyond the compiler's 64-segment record path bound",
        );
    }

    #[test]
    fn symbolic_sum_materialization_joins_case_and_payload_by_identity() {
        let numbered_layout = LayoutPlanReport {
            schema_report_fingerprint: 1,
            entries: vec![
                numbered_layout_field_entry("header", 4, LayoutPlacementReport::At { offset: 0 }),
                numbered_layout_field_entry("choice", 5, LayoutPlacementReport::At { offset: 8 }),
            ],
            offsets: Some(vec![0, 8]),
            size: Some(32),
            align: 8,
        };
        let mut numbered_sum = sum_layout();
        numbered_sum.cases[1].member_identity = Some(11);
        numbered_sum.cases[1].payload_fields[0].member_identity = Some(21);
        let carrier = SymbolicFieldInnerLayout::new_sum_numbered("choice", 5, numbered_sum);
        let symbolic = SymbolicFieldValue::new_numbered("choice", 5, 64, entry())
            .expect("numbered sum field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_numbered("RenamedCase", 11).with_inner_segment(
                    SymbolicFieldPathSegment::new_numbered("RenamedPayload", 21),
                ),
            );
        let plan = derive_symbolic_materialization_with_inner_layouts(
            &numbered_layout,
            &[carrier],
            &[symbolic],
            post_handoff_context(),
            |_| None,
        )
        .expect("numbered case and payload hops join on stable identities");

        let MaterializationAction::RuntimeWriter(write) = &plan.actions[0] else {
            panic!("an unresolved symbolic derives a runtime writer");
        };
        assert_eq!(write.field, "choice.RenamedCase.RenamedPayload");
        assert_eq!(write.container_byte_offset, 16);
    }

    #[test]
    fn symbolic_sum_materialization_rejects_malformed_sum_paths() {
        let (layout, carrier) = sum_field_layout();
        let indexed_case = SymbolicFieldValue::new("choice", 64, entry())
            .expect("direct sum field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_indexed("Run", 0)
                    .with_inner_segment(SymbolicFieldPathSegment::new("callback")),
            );
        let indexed_payload = SymbolicFieldValue::new("choice", 64, entry())
            .expect("direct sum field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new("Run")
                    .with_inner_segment(SymbolicFieldPathSegment::new_indexed("callback", 0)),
            );

        for (field, expected) in [
            (
                sym_path("choice", &["Run"], entry()),
                "requires a payload field below case `choice.Run`",
            ),
            (
                sym_path("choice", &["Bogus", "callback"], entry()),
                "spells no case or common field `Bogus` of the inner sum layout for `choice`",
            ),
            (
                sym_path("choice", &["Run", "flags"], entry()),
                "spells no payload field `flags` of case `choice.Run`",
            ),
            (
                sym_path("choice", &["Run", "callback", "deeper"], entry()),
                "continues below sum payload `choice.Run.callback`; a case payload is the leaf of a sum path",
            ),
            (
                indexed_case,
                "case `choice.Run` cannot carry an element index",
            ),
            (
                indexed_payload,
                "payload `choice.Run.callback` cannot carry an element index; a sum payload retains one extent per field",
            ),
            (
                sym_path("choice", &["Small", "flags"], entry()),
                "width 64 exceeds the 8-bit sum member `choice.Small.flags`",
            ),
        ] {
            assert_reject_inner(&layout, std::slice::from_ref(&carrier), &[field], expected);
        }
    }

    #[test]
    fn symbolic_sum_materialization_rejects_malformed_sum_carriers() {
        let (layout, carrier) = sum_field_layout();
        let path = sym_path("choice", &["Run", "callback"], entry());
        let error = derive_symbolic_materialization(
            &layout,
            std::slice::from_ref(&path),
            post_handoff_context(),
            |_| None,
        )
        .expect_err("a sum hop cannot resolve without its interior carrier");
        assert!(
            error
                .0
                .contains("`choice.Run.callback` has no supplied inner layout for `choice`"),
            "{}",
            error.0
        );

        let flat = SymbolicFieldValue::new("header", 64, entry()).expect("scalar field");
        let nested_under_sum =
            SymbolicFieldInnerLayout::new_sum("choice", sum_layout()).with_inner_layout(
                SymbolicFieldInnerLayout::new("callback", record_interior(&nested_layout().1)),
            );
        let mut escaping_layout = sum_layout();
        escaping_layout.cases[1].payload_fields[1].offset = 20;
        let escaping = SymbolicFieldInnerLayout::new_sum("choice", escaping_layout);
        let oversized = SymbolicFieldInnerLayout::new_sum(
            "choice",
            ConventionalSumLayoutReport {
                size: 40,
                ..sum_layout()
            },
        );
        for (carriers, fields, expected) in [
            (
                vec![carrier],
                vec![flat],
                "no symbolic field path traverses the supplied inner layout for `choice`",
            ),
            (
                vec![nested_under_sum],
                vec![path.clone()],
                "inner layout for `choice` binds a sum interior; its case payload fields carry no nested record carriers",
            ),
            (
                vec![escaping],
                vec![path.clone()],
                "inner layout for `choice` places payload field `clock` of case `Run` outside the sum's 24-byte extent",
            ),
            (
                vec![oversized],
                vec![path],
                "interior layout for `choice` exceeds the enclosing 40-byte record extent",
            ),
        ] {
            assert_reject_inner(&layout, &carriers, &fields, expected);
        }

        let (array_layout, _) = sum_array_layout();
        let indexed_path = SymbolicFieldValue::new_indexed("sums", 0, 64, entry())
            .expect("repeated sum field")
            .with_inner_segment(symbolic_path(&["Run", "callback"]));
        for (bad_carrier, expected) in [
            (
                SymbolicFieldInnerLayout::new_sum_array("sums", sum_layout(), 0, 24),
                "element index 0 is outside its 0 element placements",
            ),
            (
                SymbolicFieldInnerLayout::new_sum_array("sums", sum_layout(), 2, 16),
                "inner layout for `sums` strides repeated sum elements by 16 bytes inside their 24-byte extent",
            ),
        ] {
            assert_reject_inner(
                &array_layout,
                &[bad_carrier],
                std::slice::from_ref(&indexed_path),
                expected,
            );
        }
    }

    #[test]
    fn symbolic_sum_array_materialization_rejects_malformed_element_hops() {
        let (layout, carrier) = sum_array_layout();
        assert_reject_inner(
            &layout,
            std::slice::from_ref(&carrier),
            &[sym_path("sums", &["Run", "callback"], entry())],
            "requires an element index into the repeated sum field `sums`",
        );
        let out_of_range = SymbolicFieldValue::new_indexed("sums", 2, 64, entry())
            .expect("repeated sum field")
            .with_inner_segment(symbolic_path(&["Run", "callback"]));
        assert_reject_inner(
            &layout,
            std::slice::from_ref(&carrier),
            &[out_of_range],
            "element index 2 is outside its 2 element placements",
        );

        let sums_at = |offsets: &[u64]| LayoutPlanReport {
            entries: offsets
                .iter()
                .map(|&offset| layout_field_entry("sums", LayoutPlacementReport::At { offset }))
                .collect(),
            ..layout.clone()
        };
        let indexed = SymbolicFieldValue::new_indexed("sums", 1, 64, entry())
            .expect("repeated sum field")
            .with_inner_segment(symbolic_path(&["Run", "callback"]));
        let materialization = derive_symbolic_materialization_with_inner_layouts(
            &sums_at(&[8, 32]),
            std::slice::from_ref(&carrier),
            std::slice::from_ref(&indexed),
            post_handoff_context(),
            |_| None,
        )
        .expect("per-element `At` placements carry the same repeated boundary");
        assert_eq!(
            writer_field_offsets(&materialization),
            [("sums[1].Run.callback", 40)]
        );

        for (bad_layout, expected) in [
            (
                sums_at(&[8, 24, 40]),
                "inner layout for `sums` element placements drift from its count or stride",
            ),
            (
                sums_at(&[8, 30]),
                "inner layout for `sums` element placements drift from its count or stride",
            ),
            (
                LayoutPlanReport {
                    size: Some(40),
                    ..layout
                },
                "interior layout for `sums` exceeds the enclosing 40-byte record extent",
            ),
        ] {
            assert_reject_inner(
                &bad_layout,
                std::slice::from_ref(&carrier),
                std::slice::from_ref(&indexed),
                expected,
            );
        }
    }

    #[test]
    fn symbolic_recursive_sum_materialization_joins_boundaries_by_identity() {
        let mut report = recursive_sum_report();
        for entry in &mut report.outer_layout.entries {
            entry.member_identity = Some(match entry.field.as_str() {
                "header" => 4,
                "middle" => 5,
                _ => unreachable!("outer report fields"),
            });
        }
        let middle = &mut report.children[0];
        middle.member_identity = Some(5);
        let ConventionalRecordSumChildInterior::Record(mid) = &mut middle.interior else {
            panic!("the middle row carries a record interior");
        };
        for entry in &mut mid.outer_layout.entries {
            entry.member_identity = Some(match entry.field.as_str() {
                "inner" => 6,
                "tag" => 7,
                "route" => 10,
                _ => unreachable!("middle report fields"),
            });
        }
        let [inner, route] = mid.children.as_mut_slice() else {
            unreachable!("the middle level spells two children")
        };
        inner.member_identity = Some(6);
        route.member_identity = Some(10);
        let ConventionalRecordSumChildInterior::Sum(route_layout) = &mut route.interior else {
            panic!("the route row carries a sum interior");
        };
        route_layout.cases[1].member_identity = Some(13);
        route_layout.cases[1].payload_fields[1].member_identity = Some(23);
        let ConventionalRecordSumChildInterior::Record(innermost) = &mut inner.interior else {
            panic!("the inner row carries a record interior");
        };
        for entry in &mut innermost.outer_layout.entries {
            entry.member_identity = Some(match entry.field.as_str() {
                "choice" => 8,
                "pad" => 9,
                _ => unreachable!("inner report fields"),
            });
        }
        let choice = &mut innermost.children[0];
        choice.member_identity = Some(8);
        let ConventionalRecordSumChildInterior::Sum(choice_layout) = &mut choice.interior else {
            panic!("the choice row carries a sum interior");
        };
        choice_layout.cases[1].member_identity = Some(11);
        choice_layout.cases[1].payload_fields[0].member_identity = Some(21);

        let carriers = SymbolicFieldInnerLayout::from_recursive_sum_paths(&report)
            .expect("the numbered recursive report folds into carriers");
        assert_eq!(carriers[0].member_identity, Some(5));
        let inner_carriers = carriers[0].inner_layouts();
        assert_eq!(inner_carriers[0].member_identity, Some(6));
        assert_eq!(inner_carriers[1].member_identity, Some(10));
        assert_eq!(
            inner_carriers[0].inner_layouts()[0].member_identity,
            Some(8)
        );

        let numbered_chain = |target, segs: &[(&str, u64)]| {
            let mut iter = segs.iter().rev();
            let &(last_name, last_id) = iter.next().unwrap();
            let tail = iter.fold(
                SymbolicFieldPathSegment::new_numbered(last_name, last_id),
                |acc, &(name, id)| {
                    SymbolicFieldPathSegment::new_numbered(name, id).with_inner_segment(acc)
                },
            );
            SymbolicFieldValue::new_numbered("middle", 5, 64, target)
                .expect("numbered outer record field")
                .with_inner_segment(tail)
        };
        let symbolic = [
            numbered_chain(
                entry(),
                &[
                    ("inner", 6),
                    ("choice", 8),
                    ("RenamedCase", 11),
                    ("RenamedPayload", 21),
                ],
            ),
            numbered_chain(
                data(),
                &[("route", 10), ("RenamedCase", 13), ("RenamedPayload", 23)],
            ),
        ];
        let plan = derive_symbolic_materialization_with_inner_layouts(
            &report.outer_layout,
            &carriers,
            &symbolic,
            post_handoff_context(),
            |_| None,
        )
        .expect("numbered recursive boundaries join on stable identities");
        assert_eq!(
            writer_field_offsets(&plan),
            vec![
                ("middle.inner.choice.RenamedCase.RenamedPayload", 16),
                ("middle.route.RenamedCase.RenamedPayload", 64),
            ]
        );
    }

    #[test]
    fn symbolic_recursive_sum_fold_bounds_report_depth() {
        let leaf = || ConventionalRecursiveRecordSumPathsLayoutReport {
            outer_layout: LayoutPlanReport {
                schema_report_fingerprint: 9,
                entries: Vec::new(),
                offsets: Some(Vec::new()),
                size: Some(0),
                align: 1,
            },
            children: Vec::new(),
        };
        fn wrap(
            inner: ConventionalRecursiveRecordSumPathsLayoutReport,
        ) -> ConventionalRecursiveRecordSumPathsLayoutReport {
            ConventionalRecursiveRecordSumPathsLayoutReport {
                outer_layout: record_layout(9, &[("sub", 0)], 8),
                children: vec![ConventionalRecordSumChildLayoutReport {
                    field: "sub".into(),
                    member_identity: None,
                    hop: ConventionalRecordSumChildHop::Field,
                    interior: ConventionalRecordSumChildInterior::Record(inner),
                }],
            }
        }
        let mut at_bound = leaf();
        for _ in 1..CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT - 1 {
            at_bound = wrap(at_bound);
        }
        SymbolicFieldInnerLayout::from_recursive_sum_paths(&at_bound)
            .expect("a report at the record path bound still folds");

        let mut over_bound = leaf();
        for _ in 0..CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT {
            over_bound = wrap(over_bound);
        }
        let error = SymbolicFieldInnerLayout::from_recursive_sum_paths(&over_bound)
            .expect_err("a report nesting past the carrier bound must reject");
        assert!(
            error
                .0
                .contains("nests beyond the compiler's 64-segment record path bound"),
            "{}",
            error.0
        );
    }

    #[test]
    fn symbolic_recursive_record_array_paths_stay_symbolic_until_assignment() {
        let report = recursive_record_array_report();
        let carriers = SymbolicFieldInnerLayout::from_recursive_sum_paths(&report)
            .expect("the recursive record-array report folds into inner layout carriers");

        let neighbors_choice = |index: Option<u64>| {
            let segment = match index {
                Some(index) => SymbolicFieldPathSegment::new_indexed("neighbors", index),
                None => SymbolicFieldPathSegment::new("neighbors"),
            };
            SymbolicFieldValue::new("middle", 64, data())
                .expect("outer record field")
                .with_inner_segment(
                    segment.with_inner_segment(symbolic_path(&["choice", "Run", "callback"])),
                )
        };
        let cover_rest = || {
            vec![
                sym_path("middle", &["inner", "choice", "Run", "clock"], data()),
                sym_path("middle", &["route", "Run", "callback"], data()),
            ]
        };
        let with_neighbor = |index: Option<u64>| {
            std::iter::once(neighbors_choice(index))
                .chain(cover_rest())
                .collect::<Vec<_>>()
        };

        let mut shrunken = carriers.clone();
        let ConventionalRecordSumChildHop::Index { element_stride, .. } =
            &mut shrunken[0].inner_layouts[2].inner_layout.hop
        else {
            unreachable!()
        };
        *element_stride = 16;

        for (case_carriers, fields, expected) in [
            (
                &carriers,
                with_neighbor(None),
                "requires an element index into the repeated record field `middle.neighbors`",
            ),
            (
                &carriers,
                with_neighbor(Some(2)),
                "element index 2 is outside its 2 element placements",
            ),
            (
                &shrunken,
                with_neighbor(Some(1)),
                "strides repeated record elements by 16 bytes inside their 32-byte extent",
            ),
            (
                &carriers,
                cover_rest(),
                "no symbolic field path traverses the supplied inner layout for `middle.neighbors`",
            ),
        ] {
            assert_reject_inner(&report.outer_layout, case_carriers, &fields, expected);
        }
    }
}

mod writer_fragments {
    use super::{entry, post_handoff_context, split_layout, stored_integer_layout};
    use crate::storage_materialization::derive_symbolic_materialization;
    use layout_plans::test_support::post_handoff_context_with;
    use layout_plans::{
        ArtifactInstallationScopeId, ByteOrder, ConsumptionInstant, MachineRegimeId,
        MaterializationAction, MaterializationContext, PlacementAddressRange, PlacementConstraints,
        PlacementPhase, PlacementSite, SymbolicFieldValue,
    };

    fn load_context() -> MaterializationContext {
        MaterializationContext {
            consumption: ConsumptionInstant::BeforeOmegaEntry,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: PlacementConstraints::unconstrained(PlacementPhase::Load),
        }
    }

    fn post_handoff_split_writer_plan() -> layout_plans::SymbolicMaterializationPlan {
        let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
        derive_symbolic_materialization(
            &split_layout(),
            &[symbolic],
            post_handoff_context(),
            |_| None,
        )
        .expect("writer plan")
    }

    fn constrained_post_handoff_plan() -> (
        layout_plans::SymbolicMaterializationPlan,
        MachineRegimeId,
        ArtifactInstallationScopeId,
    ) {
        let regime = MachineRegimeId::from_normalized_identity(11).expect("machine regime");
        let scope =
            ArtifactInstallationScopeId::from_normalized_identity(12).expect("installation scope");
        let constraints = PlacementConstraints::new(
            Some(PlacementAddressRange::new(0x1000, 0x10_0000).expect("low-memory range")),
            4096,
            PlacementPhase::PostHandoff,
            Some(regime),
            Some(scope),
        )
        .expect("placement constraints");
        let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
        let mut layout = split_layout();
        layout.align = 16;
        let plan = derive_symbolic_materialization(
            &layout,
            &[symbolic],
            post_handoff_context_with(constraints),
            |_| None,
        )
        .expect("constrained post-handoff plan");
        (plan, regime, scope)
    }

    #[test]
    fn fixed_entry_constant_folds_split_little_endian_bytes() {
        let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
        let plan =
            derive_symbolic_materialization(&split_layout(), &[symbolic], load_context(), |_| {
                Some(0x1122_3344_5566_7788)
            })
            .expect("fixed-address fragments constant-fold");
        let mut bytes = [0_u8; 16];
        crate::storage_materialization::materialize_resolved_into(&plan, &mut bytes)
            .expect("all writes resolved");

        assert_eq!(&bytes[0..4], &[0x88, 0x77, 0x66, 0x55]);
        assert_eq!(&bytes[8..12], &[0x44, 0x33, 0x22, 0x11]);
    }

    #[test]
    fn symbolic_stored_integer_fit_is_enforced_before_each_consumption_phase() {
        let layout = stored_integer_layout();
        let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
        let context = load_context();
        let plan = derive_symbolic_materialization(
            &layout,
            std::slice::from_ref(&symbolic),
            context,
            |_| Some(0x1234_5678),
        )
        .expect("a resolved fitting symbolic value supplies a concrete fit proof");
        let mut bytes = [0xa5_u8; 4];

        let mut tampered = plan.clone();
        let MaterializationAction::ResolvedWrite { source_value, .. } = &mut tampered.actions[0]
        else {
            panic!("resolved derivation must retain a resolved write")
        };
        *source_value = 1_u64 << 32;
        let error =
            crate::storage_materialization::materialize_resolved_into(&tampered, &mut bytes)
                .expect_err("resolved execution must replay retained stored-integer fit evidence");
        assert!(error.0.contains("does not fit"), "{}", error.0);
        assert_eq!(bytes, [0xa5; 4]);

        crate::storage_materialization::materialize_resolved_into(&plan, &mut bytes)
            .expect("the resolved value should use the exact stored width");
        assert_eq!(bytes, [0x78, 0x56, 0x34, 0x12]);

        let error = derive_symbolic_materialization(
            &layout,
            std::slice::from_ref(&symbolic),
            context,
            |_| Some(1_u64 << 32),
        )
        .expect_err("an out-of-range symbolic value must reject");
        assert!(error.0.contains("does not fit"), "{}", error.0);

        let post_handoff = derive_symbolic_materialization(
            &layout,
            std::slice::from_ref(&symbolic),
            post_handoff_context(),
            |_| None,
        )
        .expect("a post-handoff resolver can discharge stored-integer fit");
        assert!(matches!(
            post_handoff.actions.as_slice(),
            [MaterializationAction::RuntimeWriter(write)]
                if write.stored_integer_fit.is_some()
        ));

        let error = derive_symbolic_materialization(&layout, &[symbolic], context, |_| None)
            .expect_err("a loader cannot defer stored-integer fit to Omega");
        assert!(error.0.contains("before Omega entry"), "{}", error.0);
    }

    #[test]
    fn unresolved_loader_consumed_fragments_reject() {
        let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
        let error =
            derive_symbolic_materialization(&split_layout(), &[symbolic], load_context(), |_| None)
                .expect_err("a loader cannot apply split pointer relocations");
        assert!(error.0.contains("before Omega entry"));
    }

    #[test]
    fn unresolved_action_cannot_partially_materialize() {
        let plan = post_handoff_split_writer_plan();
        let mut bytes = [0xa5_u8; 16];
        assert!(
            crate::storage_materialization::materialize_resolved_into(&plan, &mut bytes).is_err()
        );
        assert_eq!(bytes, [0xa5; 16]);
    }

    #[test]
    fn placement_constraints_join_layout_alignment_and_validate_all_axes() {
        let (plan, regime, scope) = constrained_post_handoff_plan();
        assert_eq!(plan.placement.alignment(), 4096);
        let valid = PlacementSite {
            base_address: 0x8000,
            phase: PlacementPhase::PostHandoff,
            machine_regime: Some(regime),
            installation_scope: Some(scope),
        };
        plan.placement
            .validate_site(plan.byte_len, valid)
            .expect("all concrete placement facts match");

        for (site, expected) in [
            (
                PlacementSite {
                    phase: PlacementPhase::Load,
                    ..valid
                },
                "phase",
            ),
            (
                PlacementSite {
                    base_address: 0x8001,
                    ..valid
                },
                "aligned",
            ),
            (
                PlacementSite {
                    machine_regime: None,
                    ..valid
                },
                "regime",
            ),
            (
                PlacementSite {
                    installation_scope: None,
                    ..valid
                },
                "scope",
            ),
            (
                PlacementSite {
                    base_address: 0x10_0000,
                    ..valid
                },
                "outside",
            ),
        ] {
            assert!(
                plan.placement
                    .validate_site(plan.byte_len, site)
                    .expect_err(expected)
                    .0
                    .contains(expected)
            );
        }
    }
}

use layout_plans::{
    ConventionalRecordSumChildHop, ConventionalRecordSumChildInterior,
    ConventionalRecordSumChildLayoutReport, ConventionalRecursiveRecordSumPathsLayoutReport,
    LayoutPlanReport, RelocationTarget, SymbolicFieldInnerLayout, SymbolicFieldValue,
};

fn sym_path(root: &str, segments: &[&str], target: RelocationTarget) -> SymbolicFieldValue {
    SymbolicFieldValue::new(root, 64, target)
        .expect("outer record field")
        .with_inner_segment(symbolic_path(segments))
}

fn nested_layout() -> (LayoutPlanReport, SymbolicFieldInnerLayout) {
    (
        record_layout(1, &[("header", 0), ("slot", 8)], 24),
        SymbolicFieldInnerLayout::new("slot", record_layout(2, &[("entry", 0), ("flags", 8)], 16)),
    )
}

fn record_interior(carrier: &SymbolicFieldInnerLayout) -> LayoutPlanReport {
    let layout_plans::SymbolicFieldInterior::Record(layout) = &carrier.inner_layout.interior else {
        panic!("the test carrier binds a record interior");
    };
    layout.clone()
}

fn sum_field_layout() -> (LayoutPlanReport, SymbolicFieldInnerLayout) {
    (
        record_layout(1, &[("header", 0), ("choice", 8), ("tail", 32)], 40),
        SymbolicFieldInnerLayout::new_sum("choice", sum_layout()),
    )
}

fn recursive_sum_report() -> ConventionalRecursiveRecordSumPathsLayoutReport {
    let field_child = |field: &str, interior| ConventionalRecordSumChildLayoutReport {
        field: field.into(),
        member_identity: None,
        hop: ConventionalRecordSumChildHop::Field,
        interior,
    };
    let innermost = ConventionalRecursiveRecordSumPathsLayoutReport {
        outer_layout: record_layout(3, &[("choice", 0), ("pad", 24)], 32),
        children: vec![field_child(
            "choice",
            ConventionalRecordSumChildInterior::Sum(sum_layout()),
        )],
    };
    let mid = ConventionalRecursiveRecordSumPathsLayoutReport {
        outer_layout: record_layout(2, &[("inner", 0), ("tag", 32), ("route", 40)], 64),
        children: vec![
            field_child(
                "inner",
                ConventionalRecordSumChildInterior::Record(innermost),
            ),
            field_child(
                "route",
                ConventionalRecordSumChildInterior::Sum(sum_layout()),
            ),
        ],
    };
    ConventionalRecursiveRecordSumPathsLayoutReport {
        outer_layout: record_layout(1, &[("header", 0), ("middle", 8)], 72),
        children: vec![field_child(
            "middle",
            ConventionalRecordSumChildInterior::Record(mid),
        )],
    }
}

fn sum_array_layout() -> (LayoutPlanReport, SymbolicFieldInnerLayout) {
    (
        record_layout(1, &[("header", 0), ("sums", 8)], 56),
        SymbolicFieldInnerLayout::new_sum_array("sums", sum_layout(), 2, 24),
    )
}

pub(super) fn assert_reject_inner(
    layout: &LayoutPlanReport,
    inner_layouts: &[SymbolicFieldInnerLayout],
    symbolic_fields: &[SymbolicFieldValue],
    expected: &str,
) {
    let error = crate::storage_materialization::derive_symbolic_materialization_with_inner_layouts(
        layout,
        inner_layouts,
        symbolic_fields,
        post_handoff_context(),
        |_| None,
    )
    .expect_err(expected)
    .0;
    assert!(error.contains(expected), "{error}");
}
