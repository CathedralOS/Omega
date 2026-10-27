use super::{
    entry, layout_field_entry, layout_report, numbered_layout_field_entry, post_handoff_context,
    split_layout,
};
use crate::{
    AggregateFieldSchema, AggregateFieldValue, ByteOrder, ConventionalSumCaseLayoutReport,
    ConventionalSumLayoutReport, ConventionalSumPayloadFieldLayoutReport, EntryStubId,
    IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
    MaterializationAction, RelocationTarget, ScalarFieldSchema, ScalarFieldValue,
    SymbolicFieldValue, conventional_sum_layout_reports_match_for_replay, decode_scalar_layout,
    derive_symbolic_materialization, layout_plan_reports_match_for_replay,
    materialize_aggregate_layout_into, materialize_scalar_layout_into,
    normalized_conventional_sum_layout_report_fingerprint,
    normalized_layout_plan_report_fingerprint,
};

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
    let LayoutPlacementReport::Bits { container, .. } = &mut shifted.entries[0].placement else {
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
    let baseline = ConventionalSumLayoutReport {
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
                member_identity: None,
                offset: 4,
                size: 4,
                align: 4,
            }],
        }],
        size: 8,
        align: 4,
    };
    let report_fingerprint = normalized_conventional_sum_layout_report_fingerprint(&baseline);
    assert_ne!(report_fingerprint, 0);

    let mut renamed_numbered_case = baseline.clone();
    renamed_numbered_case.cases[0].case = "Available".into();
    assert_eq!(
        report_fingerprint,
        normalized_conventional_sum_layout_report_fingerprint(&renamed_numbered_case)
    );

    let mut reordered = baseline.clone();
    reordered.cases[0].ordinal = 1;
    assert_ne!(
        report_fingerprint,
        normalized_conventional_sum_layout_report_fingerprint(&reordered)
    );

    let mut renamed_unnumbered_field = baseline.clone();
    renamed_unnumbered_field.cases[0].payload_fields[0].field = "payload".into();
    assert_ne!(
        report_fingerprint,
        normalized_conventional_sum_layout_report_fingerprint(&renamed_unnumbered_field)
    );

    let mut shifted = baseline;
    shifted.cases[0].payload_fields[0].offset = 8;
    assert_ne!(
        report_fingerprint,
        normalized_conventional_sum_layout_report_fingerprint(&shifted)
    );
}

#[test]
fn conventional_sum_replay_uses_numbered_identity_and_exact_geometry() {
    let original = ConventionalSumLayoutReport {
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
                member_identity: Some(52),
                offset: 4,
                size: 4,
                align: 4,
            }],
        }],
        size: 8,
        align: 4,
    };
    let mut renamed = original.clone();
    renamed.cases[0].case = "Available".into();
    renamed.cases[0].payload_fields[0].field = "payload".into();
    assert!(conventional_sum_layout_reports_match_for_replay(
        &original, &renamed
    ));

    let mut shifted = renamed.clone();
    shifted.cases[0].payload_fields[0].offset = 8;
    assert!(!conventional_sum_layout_reports_match_for_replay(
        &original, &shifted
    ));

    let mut changed_ordinal = renamed.clone();
    changed_ordinal.cases[0].ordinal = 1;
    assert!(!conventional_sum_layout_reports_match_for_replay(
        &original,
        &changed_ordinal
    ));

    let mut unnumbered = original.clone();
    unnumbered.cases[0].member_identity = None;
    let mut renamed_unnumbered = unnumbered.clone();
    renamed_unnumbered.cases[0].case = "Available".into();
    assert!(!conventional_sum_layout_reports_match_for_replay(
        &unnumbered,
        &renamed_unnumbered
    ));

    let mut aliased = original.clone();
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
    let LayoutPlacementReport::Bits { container, .. } = &mut shifted.entries[0].placement else {
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
                "legacy_payload".into(),
                9,
                LayoutPlacementReport::At { offset: 0 },
            ),
            numbered_layout_field_entry(
                "forged_payload_alias".into(),
                9,
                LayoutPlacementReport::At { offset: 4 },
            ),
        ],
        Some(vec![0, 4]),
        Some(8),
        4,
    );
    let schema = AggregateFieldSchema::new_repeated_numbered("payload", 9, 4, 4, 2)
        .expect("numbered repeated aggregate");
    let value = AggregateFieldValue::new("payload", vec![0; 8]).expect("aggregate value");
    let mut aggregate_bytes = [0xa5; 8];
    let error = materialize_aggregate_layout_into(
        &aggregate_layout,
        &[schema],
        &[value],
        ByteOrder::LittleEndian,
        &mut aggregate_bytes,
    )
    .expect_err("aggregate materialization must reject one identity under two names");
    assert!(error.0.contains("identity names both"), "{}", error.0);
    assert_eq!(aggregate_bytes, [0xa5; 8]);
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
            layout_field_entry("header".into(), LayoutPlacementReport::At { offset: 4 }),
            layout_field_entry("payload".into(), LayoutPlacementReport::At { offset: 12 }),
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
    let mut unchanged = [0xa5; 20];
    let error = materialize_aggregate_layout_into(
        &layout,
        &fields,
        &short,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("caller bytes cannot claim a complete aggregate extent");
    assert!(error.0.contains("compiler-derived extent is 6"));
    assert_eq!(unchanged, [0xa5; 20]);

    let mut fragmented = layout.clone();
    fragmented.entries[0].placement = LayoutPlacementReport::Bits {
        container: 4,
        container_width: 32,
        destination_lsb: 0,
        source_lsb: 0,
        width: 32,
    };
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

    fragmented.entries[0].placement = LayoutPlacementReport::Bits {
        container: 12,
        container_width: 32,
        destination_lsb: 0,
        source_lsb: 0,
        width: 32,
    };
    let error = materialize_aggregate_layout_into(
        &fragmented,
        &fields,
        &values,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("a fragment destination cannot overlap a whole-extent placement");
    assert!(
        error
            .0
            .contains("fragment destination overlaps a whole-extent placement"),
        "{error}"
    );
    assert_eq!(unchanged, [0xa5; 20]);
}

#[test]
fn numbered_aggregate_materialization_rejoins_renamed_fields_by_identity() {
    let layout = layout_report(
        vec![numbered_layout_field_entry(
            "legacy_payload".into(),
            7,
            LayoutPlacementReport::At { offset: 4 },
        )],
        Some(vec![4]),
        Some(12),
        4,
    );
    let schema = [AggregateFieldSchema::new_numbered("payload", 7, 4)
        .expect("compiler-derived numbered schema")];
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
    let mut unchanged = [0x5a; 12];
    let error = materialize_aggregate_layout_into(
        &drifted,
        &schema,
        &values,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("stable member identity drift must reject before mutation");
    assert!(error.0.contains("same stable identity"));
    assert_eq!(unchanged, [0x5a; 12]);

    let repeated_layout = LayoutPlanReport {
        schema_report_fingerprint: 2,
        entries: [0, 8]
            .into_iter()
            .map(|offset| {
                numbered_layout_field_entry(
                    "legacy_items".into(),
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
            .expect("compiler-derived numbered repeated schema"),
    ];
    let repeated_values = [AggregateFieldValue::new("items", [1, 2, 3, 4, 5, 6, 7, 8])
        .expect("complete repeated payload")];
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
    let schema = [AggregateFieldSchema::new_repeated("items", 4, 4, 3)
        .expect("compiler-derived repeated schema")];
    let values = [
        AggregateFieldValue::new("items", (0_u8..12).collect::<Vec<_>>())
            .expect("complete repeated aggregate"),
    ];
    let layout = |offsets: &[u64]| LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: offsets
            .iter()
            .map(|offset| {
                layout_field_entry(
                    "items".into(),
                    LayoutPlacementReport::At { offset: *offset },
                )
            })
            .collect(),
        offsets: None,
        size: Some(32),
        align: 4,
    };

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
        let mut unchanged = [0xa5; 32];
        let error = materialize_aggregate_layout_into(
            &layout(offsets),
            &schema,
            &values,
            ByteOrder::LittleEndian,
            &mut unchanged,
        )
        .expect_err("invalid repeated aggregate geometry must reject");
        assert!(
            error.0.contains(expected),
            "unexpected diagnostic: {error:?}"
        );
        assert_eq!(unchanged, [0xa5; 32]);
    }
}

#[test]
fn aggregate_materialization_applies_scalar_placements_beside_whole_extents() {
    let layout = layout_report(
        vec![
            layout_field_entry(
                "id".into(),
                LayoutPlacementReport::IntegerAt {
                    offset: 0,
                    stored_width: 16,
                    interpretation: IntegerInterpretation::Unsigned,
                },
            ),
            layout_field_entry(
                "flags".into(),
                LayoutPlacementReport::Bits {
                    container: 2,
                    container_width: 16,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 10,
                },
            ),
            layout_field_entry("pair".into(), LayoutPlacementReport::At { offset: 4 }),
            layout_field_entry("items".into(), LayoutPlacementReport::At { offset: 8 }),
            layout_field_entry("items".into(), LayoutPlacementReport::At { offset: 10 }),
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
    let little_values = [
        AggregateFieldValue::new("id", [0x34, 0x12, 0, 0]).expect("id"),
        AggregateFieldValue::new("flags", [0xa5, 0x02]).expect("flags"),
        AggregateFieldValue::new("pair", [9, 8]).expect("pair"),
        AggregateFieldValue::new("items", [0x11, 0x22, 0x33, 0x44]).expect("items"),
    ];
    let mut bytes = [0x5a; 12];
    materialize_aggregate_layout_into(
        &layout,
        &fields,
        &little_values,
        ByteOrder::LittleEndian,
        &mut bytes,
    )
    .expect("scalar placements should compose with whole extents");
    assert_eq!(
        bytes,
        [0x34, 0x12, 0xa5, 0x02, 9, 8, 0, 0, 0x11, 0x22, 0x33, 0x44]
    );

    let big_values = [
        AggregateFieldValue::new("id", [0, 0, 0x12, 0x34]).expect("id"),
        AggregateFieldValue::new("flags", [0x02, 0xa5]).expect("flags"),
        AggregateFieldValue::new("pair", [9, 8]).expect("pair"),
        AggregateFieldValue::new("items", [0x11, 0x22, 0x33, 0x44]).expect("items"),
    ];
    let mut big = [0x5a; 12];
    materialize_aggregate_layout_into(
        &layout,
        &fields,
        &big_values,
        ByteOrder::BigEndian,
        &mut big,
    )
    .expect("the same plan should materialize big-endian");
    assert_eq!(
        big,
        [0x12, 0x34, 0x02, 0xa5, 9, 8, 0, 0, 0x11, 0x22, 0x33, 0x44]
    );
}

#[test]
fn aggregate_materialization_shares_fragment_bytes_but_rejects_bit_drift() {
    // Two fields may tile disjoint destination bits of one container byte.
    let shared = layout_report(
        vec![
            layout_field_entry(
                "low".into(),
                LayoutPlacementReport::Bits {
                    container: 4,
                    container_width: 8,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 4,
                },
            ),
            layout_field_entry(
                "high".into(),
                LayoutPlacementReport::Bits {
                    container: 4,
                    container_width: 8,
                    destination_lsb: 4,
                    source_lsb: 0,
                    width: 4,
                },
            ),
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
    assert!(
        bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || *byte == 0)
    );

    let mut overlapping = shared.clone();
    overlapping.entries[1].placement = LayoutPlacementReport::Bits {
        container: 4,
        container_width: 8,
        destination_lsb: 2,
        source_lsb: 0,
        width: 4,
    };
    let mut unchanged = [0x5a; 8];
    let error = materialize_aggregate_layout_into(
        &overlapping,
        &fields,
        &values,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("overlapping fragment destinations must reject");
    assert!(
        error
            .0
            .contains("fragment destination overlaps an earlier fragment"),
        "{error}"
    );
    assert_eq!(unchanged, [0x5a; 8]);

    let mut out_of_layout = shared.clone();
    out_of_layout.entries[1].placement = LayoutPlacementReport::Bits {
        container: 7,
        container_width: 16,
        destination_lsb: 8,
        source_lsb: 0,
        width: 4,
    };
    let error = materialize_aggregate_layout_into(
        &out_of_layout,
        &fields,
        &values,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("a fragment destination past the layout must reject");
    assert!(
        error.0.contains("writes byte 8, past the 8-byte layout"),
        "{error}"
    );
    assert_eq!(unchanged, [0x5a; 8]);
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
    for (byte_order, fragment_byte) in [(ByteOrder::LittleEndian, 0), (ByteOrder::BigEndian, 1)] {
        for whole_byte in [fragment_byte, 1 - fragment_byte] {
            let layout = layout_report(
                vec![
                    layout_field_entry("whole", LayoutPlacementReport::At { offset: whole_byte }),
                    layout_field_entry(
                        "fragment",
                        LayoutPlacementReport::Bits {
                            container: 0,
                            container_width: 16,
                            destination_lsb: 0,
                            source_lsb: 0,
                            width: 8,
                        },
                    ),
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
                    layout_field_entry(
                        "wide",
                        LayoutPlacementReport::Bits {
                            container: 0,
                            container_width: 16,
                            destination_lsb: 0,
                            source_lsb: 0,
                            width: 8,
                        },
                    ),
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
                        .expect_err("different containers cannot overwrite the same physical bit")
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
    let base = |entries: Vec<LayoutFieldEntryReport>| LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries,
        offsets: None,
        size: Some(32),
        align: 4,
    };

    let mixed = base(vec![
        layout_field_entry("id", LayoutPlacementReport::At { offset: 0 }),
        layout_field_entry(
            "id",
            LayoutPlacementReport::Bits {
                container: 4,
                container_width: 8,
                destination_lsb: 0,
                source_lsb: 0,
                width: 8,
            },
        ),
        layout_field_entry("items", LayoutPlacementReport::At { offset: 8 }),
        layout_field_entry("items", LayoutPlacementReport::At { offset: 10 }),
        layout_field_entry("wide", LayoutPlacementReport::At { offset: 16 }),
        layout_field_entry("zz", LayoutPlacementReport::At { offset: 24 }),
    ]);
    let mut unchanged = [0xa5; 32];
    let error = materialize_aggregate_layout_into(
        &mixed,
        &fields,
        &values,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("mixing `At` and scalar placements must reject");
    assert!(
        error.0.contains("mixes `At` with scalar placements"),
        "{error}"
    );
    assert_eq!(unchanged, [0xa5; 32]);

    let fragmented_array = base(vec![
        layout_field_entry("id", LayoutPlacementReport::At { offset: 0 }),
        layout_field_entry(
            "items",
            LayoutPlacementReport::Bits {
                container: 4,
                container_width: 8,
                destination_lsb: 0,
                source_lsb: 0,
                width: 8,
            },
        ),
        layout_field_entry("wide", LayoutPlacementReport::At { offset: 16 }),
        layout_field_entry("zz", LayoutPlacementReport::At { offset: 24 }),
    ]);
    let error = materialize_aggregate_layout_into(
        &fragmented_array,
        &fields,
        &values,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("an outer fixed array cannot carry scalar placements");
    assert!(
        error
            .0
            .contains("carries scalar placements but is an outer fixed array"),
        "{error}"
    );
    assert_eq!(unchanged, [0xa5; 32]);

    let oversized_scalar = base(vec![
        layout_field_entry("id", LayoutPlacementReport::At { offset: 0 }),
        layout_field_entry("items", LayoutPlacementReport::At { offset: 8 }),
        layout_field_entry("items", LayoutPlacementReport::At { offset: 10 }),
        layout_field_entry(
            "wide",
            LayoutPlacementReport::Bits {
                container: 16,
                container_width: 64,
                destination_lsb: 0,
                source_lsb: 0,
                width: 64,
            },
        ),
        layout_field_entry("zz", LayoutPlacementReport::At { offset: 24 }),
    ]);
    let error = materialize_aggregate_layout_into(
        &oversized_scalar,
        &fields,
        &values,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("a field wider than the scalar carrier cannot fragment");
    assert!(
        error.0.contains("past the 64-bit scalar placement carrier"),
        "{error}"
    );
    assert_eq!(unchanged, [0xa5; 32]);

    // `zz` sorts after `items`, so the array's whole extents claim their bytes
    // before the stored-integer overlap check runs.
    let overlapping_stored = base(vec![
        layout_field_entry("id", LayoutPlacementReport::At { offset: 0 }),
        layout_field_entry("items", LayoutPlacementReport::At { offset: 8 }),
        layout_field_entry("items", LayoutPlacementReport::At { offset: 10 }),
        layout_field_entry("wide", LayoutPlacementReport::At { offset: 16 }),
        layout_field_entry(
            "zz",
            LayoutPlacementReport::IntegerAt {
                offset: 8,
                stored_width: 16,
                interpretation: IntegerInterpretation::Unsigned,
            },
        ),
    ]);
    // `zz` has exactly one placement, so no mixed-kind rejection preempts the
    // overlap check.
    let error = materialize_aggregate_layout_into(
        &overlapping_stored,
        &fields,
        &values,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("a stored integer cannot overlap another placement");
    assert!(
        error
            .0
            .contains("stored-integer placement overlaps an earlier placement"),
        "{error}"
    );
    assert_eq!(unchanged, [0xa5; 32]);

    let overflow_values = [
        AggregateFieldValue::new("id", [0, 0, 1, 0]).expect("id"),
        AggregateFieldValue::new("items", [1, 2, 3, 4]).expect("items"),
        AggregateFieldValue::new("wide", [7; 16]).expect("wide"),
        AggregateFieldValue::new("zz", [9, 9, 9, 9]).expect("zz"),
    ];
    let stored = base(vec![
        layout_field_entry(
            "id",
            LayoutPlacementReport::IntegerAt {
                offset: 0,
                stored_width: 16,
                interpretation: IntegerInterpretation::Unsigned,
            },
        ),
        layout_field_entry("items", LayoutPlacementReport::At { offset: 8 }),
        layout_field_entry("items", LayoutPlacementReport::At { offset: 10 }),
        layout_field_entry("wide", LayoutPlacementReport::At { offset: 16 }),
        layout_field_entry("zz", LayoutPlacementReport::At { offset: 24 }),
    ]);
    let error = materialize_aggregate_layout_into(
        &stored,
        &fields,
        &overflow_values,
        ByteOrder::LittleEndian,
        &mut unchanged,
    )
    .expect_err("a value exceeding its stored integer width must reject");
    assert!(
        error.0.contains("does not fit its 16-bit unsigned storage"),
        "{error}"
    );
    assert_eq!(unchanged, [0xa5; 32]);
}

#[test]
fn numbered_scalar_materialization_and_decode_rejoin_renamed_fields_by_identity() {
    let layout = layout_report(
        vec![
            numbered_layout_field_entry(
                "legacy_counter".into(),
                7,
                LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 8,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 8,
                },
            ),
            numbered_layout_field_entry(
                "legacy_counter".into(),
                7,
                LayoutPlacementReport::Bits {
                    container: 1,
                    container_width: 8,
                    destination_lsb: 0,
                    source_lsb: 8,
                    width: 8,
                },
            ),
            numbered_layout_field_entry(
                "legacy_status".into(),
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
    let error =
        materialize_scalar_layout_into(&drifted, &values, ByteOrder::LittleEndian, &mut unchanged)
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
                "signed".into(),
                LayoutPlacementReport::IntegerAt {
                    offset: 0,
                    stored_width: 8,
                    interpretation: IntegerInterpretation::Signed,
                },
            ),
            layout_field_entry(
                "unsigned".into(),
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
            "value".into(),
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
    let error = materialize_scalar_layout_into(
        &layout,
        &[ScalarFieldValue::new("value", 64, 128).expect("wide value")],
        ByteOrder::LittleEndian,
        &mut bytes,
    )
    .expect_err("a value outside signed byte storage must reject");
    assert!(error.0.contains("does not fit"), "{}", error.0);
    assert_eq!(bytes, [0xa5], "rejection must not partially materialize");

    let error = materialize_scalar_layout_into(
        &layout,
        &[ScalarFieldValue::new("value", 4, 7).expect("narrow value")],
        ByteOrder::LittleEndian,
        &mut bytes,
    )
    .expect_err("a semantic carrier narrower than storage must reject");
    assert!(error.0.contains("narrower"), "{}", error.0);
    assert_eq!(bytes, [0xa5]);
}

#[test]
fn scalar_materialization_is_complete_and_atomic() {
    let layout = layout_report(
        vec![
            layout_field_entry(
                "low".into(),
                LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 8,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 4,
                },
            ),
            layout_field_entry(
                "high".into(),
                LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 8,
                    destination_lsb: 4,
                    source_lsb: 0,
                    width: 4,
                },
            ),
        ],
        None,
        Some(1),
        1,
    );
    let mut bytes = [0xa5_u8];
    let error = materialize_scalar_layout_into(
        &layout,
        &[ScalarFieldValue::new("low", 4, 3).expect("low")],
        ByteOrder::LittleEndian,
        &mut bytes,
    )
    .expect_err("missing planned fields reject");
    assert!(error.0.contains("`high`"));
    assert_eq!(bytes, [0xa5]);

    let duplicate = ScalarFieldValue::new("low", 4, 3).expect("duplicate");
    let error = materialize_scalar_layout_into(
        &layout,
        &[duplicate.clone(), duplicate],
        ByteOrder::LittleEndian,
        &mut bytes,
    )
    .expect_err("duplicate supplied fields reject");
    assert!(error.0.contains("more than once"));
    assert_eq!(bytes, [0xa5]);

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
            layout_field_entry("first".into(), LayoutPlacementReport::At { offset: 0 }),
            layout_field_entry("second".into(), LayoutPlacementReport::At { offset: 16 }),
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
    let error =
        derive_symbolic_materialization(&layout, &symbolic_fields, post_handoff_context(), |_| {
            resolutions += 1;
            Some(0)
        })
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
            layout_field_entry("first".into(), LayoutPlacementReport::At { offset: 0 }),
            layout_field_entry("unrelated".into(), LayoutPlacementReport::At { offset: 8 }),
            layout_field_entry(
                "narrow".into(),
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
