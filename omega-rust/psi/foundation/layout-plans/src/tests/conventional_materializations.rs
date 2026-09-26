use super::{entry, split_layout};
use crate::{
    AggregateFieldSchema, AggregateFieldValue, ByteOrder, ConsumptionInstant,
    ConventionalSumCaseLayoutReport, ConventionalSumLayoutReport,
    ConventionalSumPayloadFieldLayoutReport, EntryStubId, IntegerInterpretation,
    LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport, MaterializationAction,
    MaterializationContext, PlacementConstraints, PlacementPhase, PlacementSite, RelocationTarget,
    ScalarFieldSchema, ScalarFieldValue, SymbolicFieldValue,
    conventional_sum_layout_reports_match_for_replay, decode_scalar_layout,
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
    let error = derive_symbolic_materialization(
        &aliased,
        &[symbolic],
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
        |_| {
            resolutions += 1;
            None
        },
    )
    .expect_err("symbolic derivation must reject one identity under two names");
    assert!(error.0.contains("identity names both"), "{}", error.0);
    assert_eq!(resolutions, 0);

    let aggregate_layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![
            LayoutFieldEntryReport {
                field: "legacy_payload".into(),
                member_identity: Some(9),
                placement: LayoutPlacementReport::At { offset: 0 },
            },
            LayoutFieldEntryReport {
                field: "forged_payload_alias".into(),
                member_identity: Some(9),
                placement: LayoutPlacementReport::At { offset: 4 },
            },
        ],
        offsets: Some(vec![0, 4]),
        size: Some(8),
        align: 4,
    };
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
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![
            LayoutFieldEntryReport {
                field: "header".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 4 },
            },
            LayoutFieldEntryReport {
                field: "payload".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 12 },
            },
        ],
        offsets: Some(vec![4, 12]),
        size: Some(20),
        align: 4,
    };
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
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![LayoutFieldEntryReport {
            field: "legacy_payload".into(),
            member_identity: Some(7),
            placement: LayoutPlacementReport::At { offset: 4 },
        }],
        offsets: Some(vec![4]),
        size: Some(12),
        align: 4,
    };
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
            .map(|offset| LayoutFieldEntryReport {
                field: "legacy_items".into(),
                member_identity: Some(9),
                placement: LayoutPlacementReport::At { offset },
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
            .map(|offset| LayoutFieldEntryReport {
                field: "items".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: *offset },
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
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![
            LayoutFieldEntryReport {
                field: "id".into(),
                member_identity: None,
                placement: LayoutPlacementReport::IntegerAt {
                    offset: 0,
                    stored_width: 16,
                    interpretation: IntegerInterpretation::Unsigned,
                },
            },
            LayoutFieldEntryReport {
                field: "flags".into(),
                member_identity: None,
                placement: LayoutPlacementReport::Bits {
                    container: 2,
                    container_width: 16,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 10,
                },
            },
            LayoutFieldEntryReport {
                field: "pair".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 4 },
            },
            LayoutFieldEntryReport {
                field: "items".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 8 },
            },
            LayoutFieldEntryReport {
                field: "items".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 10 },
            },
        ],
        offsets: None,
        size: Some(12),
        align: 4,
    };
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
    let shared = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![
            LayoutFieldEntryReport {
                field: "low".into(),
                member_identity: None,
                placement: LayoutPlacementReport::Bits {
                    container: 4,
                    container_width: 8,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 4,
                },
            },
            LayoutFieldEntryReport {
                field: "high".into(),
                member_identity: None,
                placement: LayoutPlacementReport::Bits {
                    container: 4,
                    container_width: 8,
                    destination_lsb: 4,
                    source_lsb: 0,
                    width: 4,
                },
            },
        ],
        offsets: None,
        size: Some(8),
        align: 1,
    };
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
    let entry = |field: &str, placement| LayoutFieldEntryReport {
        field: field.to_owned(),
        member_identity: None,
        placement,
    };
    let base = |entries: Vec<LayoutFieldEntryReport>| LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries,
        offsets: None,
        size: Some(32),
        align: 4,
    };

    let mixed = base(vec![
        entry("id", LayoutPlacementReport::At { offset: 0 }),
        entry(
            "id",
            LayoutPlacementReport::Bits {
                container: 4,
                container_width: 8,
                destination_lsb: 0,
                source_lsb: 0,
                width: 8,
            },
        ),
        entry("items", LayoutPlacementReport::At { offset: 8 }),
        entry("items", LayoutPlacementReport::At { offset: 10 }),
        entry("wide", LayoutPlacementReport::At { offset: 16 }),
        entry("zz", LayoutPlacementReport::At { offset: 24 }),
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
        entry("id", LayoutPlacementReport::At { offset: 0 }),
        entry(
            "items",
            LayoutPlacementReport::Bits {
                container: 4,
                container_width: 8,
                destination_lsb: 0,
                source_lsb: 0,
                width: 8,
            },
        ),
        entry("wide", LayoutPlacementReport::At { offset: 16 }),
        entry("zz", LayoutPlacementReport::At { offset: 24 }),
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
        entry("id", LayoutPlacementReport::At { offset: 0 }),
        entry("items", LayoutPlacementReport::At { offset: 8 }),
        entry("items", LayoutPlacementReport::At { offset: 10 }),
        entry(
            "wide",
            LayoutPlacementReport::Bits {
                container: 16,
                container_width: 64,
                destination_lsb: 0,
                source_lsb: 0,
                width: 64,
            },
        ),
        entry("zz", LayoutPlacementReport::At { offset: 24 }),
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
        entry("id", LayoutPlacementReport::At { offset: 0 }),
        entry("items", LayoutPlacementReport::At { offset: 8 }),
        entry("items", LayoutPlacementReport::At { offset: 10 }),
        entry("wide", LayoutPlacementReport::At { offset: 16 }),
        entry(
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
        entry(
            "id",
            LayoutPlacementReport::IntegerAt {
                offset: 0,
                stored_width: 16,
                interpretation: IntegerInterpretation::Unsigned,
            },
        ),
        entry("items", LayoutPlacementReport::At { offset: 8 }),
        entry("items", LayoutPlacementReport::At { offset: 10 }),
        entry("wide", LayoutPlacementReport::At { offset: 16 }),
        entry("zz", LayoutPlacementReport::At { offset: 24 }),
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
fn ordinary_scalar_materializer_packs_a_fragmented_control_word() {
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![
            LayoutFieldEntryReport {
                field: "enabled".into(),
                member_identity: None,
                placement: LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 64,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 1,
                },
            },
            LayoutFieldEntryReport {
                field: "mode".into(),
                member_identity: None,
                placement: LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 64,
                    destination_lsb: 1,
                    source_lsb: 0,
                    width: 1,
                },
            },
            LayoutFieldEntryReport {
                field: "payload".into(),
                member_identity: None,
                placement: LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 64,
                    destination_lsb: 12,
                    source_lsb: 0,
                    width: 40,
                },
            },
            LayoutFieldEntryReport {
                field: "high_guard".into(),
                member_identity: None,
                placement: LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 64,
                    destination_lsb: 63,
                    source_lsb: 0,
                    width: 1,
                },
            },
        ],
        offsets: None,
        size: Some(8),
        align: 8,
    };
    let values = [
        ScalarFieldValue::new("enabled", 1, 1).expect("enabled"),
        ScalarFieldValue::new("mode", 1, 1).expect("mode"),
        ScalarFieldValue::new("payload", 40, 0x12345).expect("payload"),
        ScalarFieldValue::new("high_guard", 1, 1).expect("high guard"),
    ];
    let mut bytes = [0xa5_u8; 8];
    materialize_scalar_layout_into(&layout, &values, ByteOrder::LittleEndian, &mut bytes)
        .expect("validated scalar layout materializes");

    assert_eq!(
        u64::from_le_bytes(bytes),
        (1_u64 << 63) | (0x12345_u64 << 12) | 0b11
    );

    let decoded = decode_scalar_layout(
        &layout,
        &[
            ScalarFieldSchema::new("enabled", 1).expect("enabled"),
            ScalarFieldSchema::new("mode", 1).expect("mode"),
            ScalarFieldSchema::new("payload", 40).expect("payload"),
            ScalarFieldSchema::new("high_guard", 1).expect("high guard"),
        ],
        ByteOrder::LittleEndian,
        &bytes,
    )
    .expect("the same plan decodes the materialized bytes");
    let values = decoded
        .iter()
        .map(|field| (field.field.as_str(), field.value))
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(values["enabled"], 1);
    assert_eq!(values["mode"], 1);
    assert_eq!(values["payload"], 0x12345);
    assert_eq!(values["high_guard"], 1);
}

#[test]
fn numbered_scalar_materialization_and_decode_rejoin_renamed_fields_by_identity() {
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![
            LayoutFieldEntryReport {
                field: "legacy_counter".into(),
                member_identity: Some(7),
                placement: LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 8,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 8,
                },
            },
            LayoutFieldEntryReport {
                field: "legacy_counter".into(),
                member_identity: Some(7),
                placement: LayoutPlacementReport::Bits {
                    container: 1,
                    container_width: 8,
                    destination_lsb: 0,
                    source_lsb: 8,
                    width: 8,
                },
            },
            LayoutFieldEntryReport {
                field: "legacy_status".into(),
                member_identity: Some(9),
                placement: LayoutPlacementReport::At { offset: 2 },
            },
        ],
        offsets: None,
        size: Some(4),
        align: 2,
    };
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
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![
            LayoutFieldEntryReport {
                field: "signed".into(),
                member_identity: None,
                placement: LayoutPlacementReport::IntegerAt {
                    offset: 0,
                    stored_width: 8,
                    interpretation: IntegerInterpretation::Signed,
                },
            },
            LayoutFieldEntryReport {
                field: "unsigned".into(),
                member_identity: None,
                placement: LayoutPlacementReport::IntegerAt {
                    offset: 1,
                    stored_width: 16,
                    interpretation: IntegerInterpretation::Unsigned,
                },
            },
        ],
        offsets: None,
        size: Some(3),
        align: 1,
    };
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
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![LayoutFieldEntryReport {
            field: "value".into(),
            member_identity: None,
            placement: LayoutPlacementReport::IntegerAt {
                offset: 0,
                stored_width: 8,
                interpretation: IntegerInterpretation::Signed,
            },
        }],
        offsets: None,
        size: Some(1),
        align: 1,
    };
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
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![
            LayoutFieldEntryReport {
                field: "low".into(),
                member_identity: None,
                placement: LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 8,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 4,
                },
            },
            LayoutFieldEntryReport {
                field: "high".into(),
                member_identity: None,
                placement: LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 8,
                    destination_lsb: 4,
                    source_lsb: 0,
                    width: 4,
                },
            },
        ],
        offsets: None,
        size: Some(1),
        align: 1,
    };
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
fn numbered_symbolic_materialization_rejoins_renamed_fields_by_identity() {
    let mut layout = split_layout();
    for entry in &mut layout.entries {
        entry.field = "legacy_address".into();
        entry.member_identity = Some(7);
    }
    let context = MaterializationContext {
        consumption: ConsumptionInstant::AfterOmegaHandoff,
        byte_order: ByteOrder::LittleEndian,
        native_pointer_relocation_bits: None,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
    };
    let symbolic =
        SymbolicFieldValue::new_numbered("handler", 7, 64, entry()).expect("numbered target");
    let plan =
        derive_symbolic_materialization(&layout, std::slice::from_ref(&symbolic), context, |_| {
            None
        })
        .expect("stable identity should rejoin renamed symbolic fragments");
    assert_eq!(plan.actions.len(), 3);
    assert!(plan.actions.iter().all(|action| {
        matches!(action, MaterializationAction::RuntimeWriter(write) if write.field == "handler")
    }));

    let current_fragment = plan
        .derive_post_handoff_writer()
        .expect("renamed symbolic plan derives a writer")
        .lower_reusable_fragment()
        .expect("renamed symbolic writer lowers");
    let legacy = SymbolicFieldValue::new_numbered("legacy_address", 7, 64, entry())
        .expect("legacy numbered target");
    let legacy_fragment = derive_symbolic_materialization(&layout, &[legacy], context, |_| None)
        .expect("matching presentation spelling also derives")
        .derive_post_handoff_writer()
        .expect("legacy symbolic plan derives a writer")
        .lower_reusable_fragment()
        .expect("legacy symbolic writer lowers");
    assert_eq!(
        current_fragment.fragment.report_fingerprint(),
        legacy_fragment.fragment.report_fingerprint(),
        "presentation spelling must not change generated writer identity"
    );

    let mut drifted = layout.clone();
    drifted.entries[1].member_identity = Some(8);
    let mut resolutions = 0;
    let error =
        derive_symbolic_materialization(&drifted, std::slice::from_ref(&symbolic), context, |_| {
            resolutions += 1;
            None
        })
        .expect_err("fragment identity drift must reject before resolution");
    assert!(error.0.contains("same stable identity"), "{}", error.0);
    assert_eq!(resolutions, 0);

    let alias = SymbolicFieldValue::new_numbered("alias", 7, 64, entry()).expect("identity alias");
    let error = derive_symbolic_materialization(&layout, &[symbolic, alias], context, |_| {
        resolutions += 1;
        None
    })
    .expect_err("one stable identity cannot name two supplied symbolic values");
    assert!(error.0.contains("repeats stable member identity #7"));
    assert_eq!(resolutions, 0);
}

#[test]
fn symbolic_write_geometry_rejects_before_any_resolver_invocation() {
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![
            LayoutFieldEntryReport {
                field: "first".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            },
            LayoutFieldEntryReport {
                field: "second".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 16 },
            },
        ],
        offsets: Some(vec![0, 16]),
        size: Some(16),
        align: 8,
    };
    let symbolic_fields = [
        SymbolicFieldValue::new("first", 64, entry()).expect("first symbolic field"),
        SymbolicFieldValue::new("second", 64, entry()).expect("second symbolic field"),
    ];
    let mut resolutions = 0;
    let error = derive_symbolic_materialization(
        &layout,
        &symbolic_fields,
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
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
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![
            LayoutFieldEntryReport {
                field: "first".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            },
            LayoutFieldEntryReport {
                field: "unrelated".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 8 },
            },
            LayoutFieldEntryReport {
                field: "narrow".into(),
                member_identity: None,
                placement: LayoutPlacementReport::IntegerAt {
                    offset: 16,
                    stored_width: 8,
                    interpretation: IntegerInterpretation::Unsigned,
                },
            },
        ],
        offsets: None,
        size: Some(24),
        align: 8,
    };
    let symbolic_fields = [
        SymbolicFieldValue::new("first", 64, first).expect("first symbolic field"),
        SymbolicFieldValue::new("unrelated", 64, unrelated).expect("unrelated symbolic field"),
        SymbolicFieldValue::new("narrow", 64, first).expect("narrow symbolic field"),
    ];
    let mut resolutions = Vec::new();
    let error = derive_symbolic_materialization(
        &layout,
        &symbolic_fields,
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
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
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
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

#[test]
fn unresolved_post_handoff_entry_derives_split_writer() {
    let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
    let plan = derive_symbolic_materialization(
        &split_layout(),
        &[symbolic],
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
        |_| None,
    )
    .expect("post-handoff fragments have a writer path");

    assert_eq!(plan.actions.len(), 3);
    assert!(
        plan.actions
            .iter()
            .all(|action| matches!(action, MaterializationAction::RuntimeWriter(_)))
    );

    let writer = plan
        .derive_post_handoff_writer()
        .expect("runtime actions form a writer program");
    let mut bytes = [0_u8; 16];
    let mut resolutions = 0;
    writer
        .execute(
            &mut bytes,
            PlacementSite {
                base_address: 0,
                phase: PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: None,
            },
            |target| {
                assert_eq!(target, entry());
                resolutions += 1;
                Some(0x1122_3344_5566_7788)
            },
        )
        .expect("provider resolves and executes the writer");

    assert_eq!(resolutions, 1, "three fragments share one resolution");
    assert_eq!(&bytes[0..4], &[0x88, 0x77, 0x66, 0x55]);
    assert_eq!(&bytes[8..12], &[0x44, 0x33, 0x22, 0x11]);
}
