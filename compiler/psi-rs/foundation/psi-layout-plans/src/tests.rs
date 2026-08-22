use super::*;

fn entry() -> RelocationTarget {
    RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x55aa).expect("nonzero identity"),
    )
}

fn split_layout() -> LayoutPlanReport {
    LayoutPlanReport {
        schema_identity: 1,
        entries: vec![
            LayoutFieldEntryReport {
                field: "address".into(),
                member_identity: None,
                placement: LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 16,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 16,
                },
            },
            LayoutFieldEntryReport {
                field: "address".into(),
                member_identity: None,
                placement: LayoutPlacementReport::Bits {
                    container: 2,
                    container_width: 16,
                    destination_lsb: 0,
                    source_lsb: 16,
                    width: 16,
                },
            },
            LayoutFieldEntryReport {
                field: "address".into(),
                member_identity: None,
                placement: LayoutPlacementReport::Bits {
                    container: 8,
                    container_width: 64,
                    destination_lsb: 0,
                    source_lsb: 32,
                    width: 32,
                },
            },
        ],
        offsets: None,
        size: Some(16),
        align: 1,
    }
}

#[test]
fn normalized_layout_identity_is_order_independent_and_geometry_bound() {
    let forward = split_layout();
    let mut reversed = forward.clone();
    reversed.entries.reverse();
    assert_eq!(
        normalized_layout_plan_fingerprint(&forward),
        normalized_layout_plan_fingerprint(&reversed)
    );

    let mut shifted = forward.clone();
    let LayoutPlacementReport::Bits { container, .. } = &mut shifted.entries[0].placement else {
        unreachable!("split layout uses bit fragments")
    };
    *container = 4;
    assert_ne!(
        normalized_layout_plan_fingerprint(&forward),
        normalized_layout_plan_fingerprint(&shifted)
    );
}

#[test]
fn stable_member_identity_makes_source_rename_presentation_only() {
    let mut original = split_layout();
    original.schema_identity = 0x44;
    for entry in &mut original.entries {
        entry.member_identity = Some(7);
    }
    let mut renamed = original.clone();
    for entry in &mut renamed.entries {
        entry.field = "renamed_address".into();
    }
    assert_eq!(
        normalized_layout_plan_fingerprint(&original),
        normalized_layout_plan_fingerprint(&renamed)
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
    changed_schema.schema_identity = 0x45;
    assert_ne!(
        normalized_layout_plan_fingerprint(&original),
        normalized_layout_plan_fingerprint(&changed_schema)
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
        schema_identity: 1,
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
        &mut aggregate_bytes,
    )
    .expect_err("aggregate materialization must reject one identity under two names");
    assert!(error.0.contains("identity names both"), "{}", error.0);
    assert_eq!(aggregate_bytes, [0xa5; 8]);
}

#[test]
fn normalized_layout_identity_distinguishes_dynamic_from_full_width_size() {
    let dynamic = LayoutPlanReport {
        schema_identity: 1,
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
        normalized_layout_plan_fingerprint(&dynamic),
        normalized_layout_plan_fingerprint(&fixed)
    );
}

#[test]
fn owned_aggregate_materializer_places_complete_values_atomically() {
    let layout = LayoutPlanReport {
        schema_identity: 1,
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
    materialize_aggregate_layout_into(&layout, &fields, &values, &mut bytes)
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
    let error = materialize_aggregate_layout_into(&layout, &fields, &short, &mut unchanged)
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
    let error = materialize_aggregate_layout_into(&fragmented, &fields, &values, &mut unchanged)
        .expect_err("aggregate fields cannot enter scalar fragment placement");
    assert!(error.0.contains("requires one whole `At` placement"));
    assert_eq!(unchanged, [0xa5; 20]);
}

#[test]
fn numbered_aggregate_materialization_rejoins_renamed_fields_by_identity() {
    let layout = LayoutPlanReport {
        schema_identity: 1,
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
    materialize_aggregate_layout_into(&layout, &schema, &values, &mut bytes)
        .expect("stable identity should rejoin a renamed aggregate field");
    assert_eq!(bytes, [0, 0, 0, 0, 1, 2, 3, 4, 0, 0, 0, 0]);

    let mut drifted = layout;
    drifted.entries[0].member_identity = Some(8);
    let mut unchanged = [0x5a; 12];
    let error = materialize_aggregate_layout_into(&drifted, &schema, &values, &mut unchanged)
        .expect_err("stable member identity drift must reject before mutation");
    assert!(error.0.contains("same stable identity"));
    assert_eq!(unchanged, [0x5a; 12]);

    let repeated_layout = LayoutPlanReport {
        schema_identity: 2,
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
        schema_identity: 1,
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
        let error =
            materialize_aggregate_layout_into(&layout(offsets), &schema, &values, &mut unchanged)
                .expect_err("invalid repeated aggregate geometry must reject");
        assert!(
            error.0.contains(expected),
            "unexpected diagnostic: {error:?}"
        );
        assert_eq!(unchanged, [0xa5; 32]);
    }
}

#[test]
fn ordinary_scalar_materializer_packs_a_fragmented_control_word() {
    let layout = LayoutPlanReport {
        schema_identity: 1,
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
        schema_identity: 1,
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
        schema_identity: 1,
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
        schema_identity: 1,
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
        schema_identity: 1,
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
        current_fragment.fragment.fingerprint(),
        legacy_fragment.fragment.fingerprint(),
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
        schema_identity: 1,
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
        schema_identity: 1,
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

#[test]
fn reusable_writer_fragment_separates_static_geometry_from_invocation_evidence() {
    let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
    let plan = derive_symbolic_materialization(
        &split_layout(),
        &[symbolic],
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
        |_| None,
    )
    .expect("symbolic materialization");
    let writer = plan
        .derive_post_handoff_writer()
        .expect("post-handoff writer");
    let lowering = writer
        .lower_reusable_fragment()
        .expect("address-free reusable fragment");

    assert_eq!(
        lowering.fragment.context_abi(),
        POST_HANDOFF_WRITER_CONTEXT_ABI_V1
    );
    assert_eq!(lowering.fragment.source_slot_count(), 1);
    assert_eq!(lowering.sources.len(), 1);
    assert_eq!(lowering.sources[0].target, entry());
    assert_eq!(
        lowering.sources[0].source,
        PostHandoffWriterSource::Resolve(entry())
    );
    assert!(
        lowering
            .fragment
            .steps()
            .iter()
            .all(|step| step.source_slot == 0),
        "all three fragments of one symbolic target share one private slot"
    );
    assert_eq!(post_handoff_writer_context_byte_len(1), Some(16));

    let replacement = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x66bb).expect("replacement entry"),
    );
    let mut rebound = writer.clone();
    for step in &mut rebound.steps {
        step.write.target = replacement;
        step.source = PostHandoffWriterSource::Resolve(replacement);
    }
    rebound.placement =
        PlacementConstraints::new(None, 16, PlacementPhase::PostHandoff, None, None)
            .expect("stronger invocation placement");
    let rebound = rebound
        .lower_reusable_fragment()
        .expect("same reusable geometry");

    assert_eq!(
        rebound.fragment.fingerprint(),
        lowering.fragment.fingerprint(),
        "target identity and concrete placement are invocation evidence"
    );
    assert_eq!(rebound.fragment, lowering.fragment);
    assert_ne!(rebound.sources, lowering.sources);
    assert_ne!(rebound.placement, lowering.placement);
}

#[test]
fn reusable_writer_fragment_rejects_inconsistent_values_for_one_target() {
    let write = MaterializationWrite {
        field: "address".into(),
        target: entry(),
        container_byte_offset: 0,
        container_width_bits: 64,
        destination_lsb: 0,
        source_lsb: 0,
        width: 32,
        stored_integer_fit: None,
    };
    let writer = PostHandoffWriterPlan {
        byte_len: 8,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: vec![
            PostHandoffWriterStep {
                write: write.clone(),
                source: PostHandoffWriterSource::Resolved(1),
            },
            PostHandoffWriterStep {
                write: MaterializationWrite {
                    destination_lsb: 32,
                    source_lsb: 32,
                    ..write
                },
                source: PostHandoffWriterSource::Resolved(2),
            },
        ],
    };

    let error = writer
        .lower_reusable_fragment()
        .expect_err("one symbolic source cannot change between fragments");
    assert!(error.0.contains("inconsistent invocation values"));
    let error = writer
        .validate(
            8,
            PlacementSite {
                base_address: 0,
                phase: PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: None,
            },
        )
        .expect_err("direct execution validates the same source invariant");
    assert!(error.0.contains("inconsistent invocation values"));
}

#[test]
fn invocation_source_validation_rejects_preresolved_value_substitution() {
    let target = entry();
    let writer = PostHandoffWriterPlan {
        byte_len: 8,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: vec![PostHandoffWriterStep {
            write: MaterializationWrite {
                field: "address".into(),
                target,
                container_byte_offset: 0,
                container_width_bits: 64,
                destination_lsb: 0,
                source_lsb: 0,
                width: 64,
                stored_integer_fit: None,
            },
            source: PostHandoffWriterSource::Resolved(0x1234),
        }],
    };
    let invocation = writer
        .lower_reusable_fragment()
        .expect("valid pre-resolved writer invocation");
    invocation
        .validate_source_values(&[0x1234])
        .expect("the exact retained pre-resolved value remains valid");

    let error = invocation
        .validate_source_values(&[0x5678])
        .expect_err("source values cannot substitute pre-resolved invocation evidence");
    assert!(error.0.contains("source slot 0"), "{}", error.0);
    assert!(error.0.contains("0x5678"), "{}", error.0);
    assert!(error.0.contains("0x1234"), "{}", error.0);
}

#[test]
fn invocation_structure_replay_rejects_tamper_and_preserves_retry() {
    let target = entry();
    let writer = PostHandoffWriterPlan {
        byte_len: 8,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: vec![PostHandoffWriterStep {
            write: MaterializationWrite {
                field: "address".into(),
                target,
                container_byte_offset: 0,
                container_width_bits: 64,
                destination_lsb: 0,
                source_lsb: 0,
                width: 64,
                stored_integer_fit: None,
            },
            source: PostHandoffWriterSource::Resolve(target),
        }],
    };
    let invocation = writer
        .lower_reusable_fragment()
        .expect("valid reusable invocation");
    invocation
        .validate_structure()
        .expect("lowering produces canonical structure");

    let mut wrong_abi = invocation.clone();
    wrong_abi.fragment.context_abi ^= 1;
    assert!(
        wrong_abi
            .validate_structure()
            .expect_err("context ABI drift must reject")
            .0
            .contains("context ABI")
    );

    let mut missing_slot = invocation.clone();
    missing_slot.fragment.steps[0].source_slot = 1;
    assert!(
        missing_slot
            .validate_structure()
            .expect_err("missing source slot must reject")
            .0
            .contains("missing source slot")
    );

    let mut zero_alignment = invocation.clone();
    zero_alignment.placement.alignment = 0;
    assert!(
        zero_alignment
            .validate_structure()
            .expect_err("zero alignment must reject")
            .0
            .contains("alignment")
    );

    let mut wrong_fingerprint = invocation.clone();
    wrong_fingerprint.fragment.fingerprint ^= 1;
    assert!(
        wrong_fingerprint
            .validate_structure()
            .expect_err("fragment fingerprint drift must reject")
            .0
            .contains("fingerprint")
    );

    invocation
        .validate_source_values(&[0x1234])
        .expect("the untouched invocation remains valid for retry");
}

#[test]
fn empty_post_handoff_writer_rejects_every_execution_path() {
    let symbolic = SymbolicMaterializationPlan {
        byte_len: 8,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        actions: Vec::new(),
    };
    let error = symbolic
        .derive_post_handoff_writer()
        .expect_err("empty symbolic actions cannot claim a generated writer");
    assert!(error.0.contains("at least one fragment"), "{}", error.0);

    let writer = PostHandoffWriterPlan {
        byte_len: 8,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: Vec::new(),
    };
    let error = writer
        .lower_reusable_fragment()
        .expect_err("empty direct plans cannot claim reusable lowering");
    assert!(error.0.contains("at least one fragment"), "{}", error.0);

    let mut bytes = [0xa5; 8];
    let mut resolutions = 0;
    let error = writer
        .execute(
            &mut bytes,
            PlacementSite {
                base_address: 0,
                phase: PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: None,
            },
            |_| {
                resolutions += 1;
                Some(0)
            },
        )
        .expect_err("empty direct execution cannot report writer success");
    assert!(error.0.contains("at least one fragment"), "{}", error.0);
    assert_eq!(resolutions, 0);
    assert_eq!(bytes, [0xa5; 8]);
}

#[test]
fn writer_stops_resolving_after_one_target_fails_value_validation() {
    let first = entry();
    let second = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x66bb).expect("second entry identity"),
    );
    let writer = PostHandoffWriterPlan {
        byte_len: 16,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: vec![
            PostHandoffWriterStep {
                write: MaterializationWrite {
                    field: "narrow".into(),
                    target: first,
                    container_byte_offset: 0,
                    container_width_bits: 8,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 8,
                    stored_integer_fit: Some(StoredIntegerFit {
                        source_width_bits: 64,
                        stored_width_bits: 8,
                        interpretation: IntegerInterpretation::Unsigned,
                    }),
                },
                source: PostHandoffWriterSource::Resolve(first),
            },
            PostHandoffWriterStep {
                write: MaterializationWrite {
                    field: "later".into(),
                    target: second,
                    container_byte_offset: 8,
                    container_width_bits: 64,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 64,
                    stored_integer_fit: None,
                },
                source: PostHandoffWriterSource::Resolve(second),
            },
        ],
    };
    let mut bytes = [0xa5; 16];
    let mut resolutions = Vec::new();
    let error = writer
        .execute(
            &mut bytes,
            PlacementSite {
                base_address: 0,
                phase: PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: None,
            },
            |target| {
                resolutions.push(target);
                Some(if target == first { 0x100 } else { 0 })
            },
        )
        .expect_err("the first target does not fit its retained stored width");
    assert!(error.0.contains("does not fit"), "{}", error.0);
    assert_eq!(resolutions, vec![first]);
    assert_eq!(bytes, [0xa5; 16]);
}

#[test]
fn writer_rejects_invalid_preresolved_value_before_any_resolution() {
    let dynamic = entry();
    let pre_resolved = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x66bb).expect("pre-resolved entry identity"),
    );
    let writer = PostHandoffWriterPlan {
        byte_len: 16,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: vec![
            PostHandoffWriterStep {
                write: MaterializationWrite {
                    field: "dynamic".into(),
                    target: dynamic,
                    container_byte_offset: 0,
                    container_width_bits: 64,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 64,
                    stored_integer_fit: None,
                },
                source: PostHandoffWriterSource::Resolve(dynamic),
            },
            PostHandoffWriterStep {
                write: MaterializationWrite {
                    field: "narrow".into(),
                    target: pre_resolved,
                    container_byte_offset: 8,
                    container_width_bits: 8,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 8,
                    stored_integer_fit: Some(StoredIntegerFit {
                        source_width_bits: 64,
                        stored_width_bits: 8,
                        interpretation: IntegerInterpretation::Unsigned,
                    }),
                },
                source: PostHandoffWriterSource::Resolved(0x100),
            },
        ],
    };

    let error = writer
        .lower_reusable_fragment()
        .expect_err("lowering must reject known-invalid invocation evidence");
    assert!(error.0.contains("does not fit"), "{}", error.0);

    let mut bytes = [0xa5; 16];
    let mut resolutions = 0;
    let error = writer
        .execute(
            &mut bytes,
            PlacementSite {
                base_address: 0,
                phase: PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: None,
            },
            |_| {
                resolutions += 1;
                Some(0)
            },
        )
        .expect_err("known-invalid invocation evidence must reject during static preflight");
    assert!(error.0.contains("does not fit"), "{}", error.0);
    assert_eq!(resolutions, 0);
    assert_eq!(bytes, [0xa5; 16]);
}

#[test]
fn writer_rejects_a_resolved_source_that_does_not_match_its_write_target() {
    let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
    let plan = derive_symbolic_materialization(
        &split_layout(),
        &[symbolic],
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
        |_| None,
    )
    .expect("writer plan");
    let mut writer = plan
        .derive_post_handoff_writer()
        .expect("runtime actions form a writer program");
    let substituted = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x66bb).expect("second entry identity"),
    );
    writer.steps[0].source = PostHandoffWriterSource::Resolve(substituted);

    let mut bytes = [0xa5_u8; 16];
    let error = writer
        .execute(
            &mut bytes,
            PlacementSite {
                base_address: 0,
                phase: PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: None,
            },
            |_| panic!("mismatched writer target must reject before resolution"),
        )
        .expect_err("writer source substitution must reject");
    assert!(error.0.contains("does not match write target"));
    assert_eq!(bytes, [0xa5; 16]);
}

#[test]
fn writer_validates_every_step_before_direct_destination_writes() {
    let valid = MaterializationWrite {
        field: "valid".into(),
        target: entry(),
        container_byte_offset: 0,
        container_width_bits: 64,
        destination_lsb: 0,
        source_lsb: 0,
        width: 64,
        stored_integer_fit: None,
    };
    let invalid = MaterializationWrite {
        field: "outside".into(),
        container_byte_offset: 16,
        ..valid.clone()
    };
    let writer = PostHandoffWriterPlan {
        byte_len: 16,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: vec![
            PostHandoffWriterStep {
                write: valid,
                source: PostHandoffWriterSource::Resolve(entry()),
            },
            PostHandoffWriterStep {
                write: invalid,
                source: PostHandoffWriterSource::Resolve(entry()),
            },
        ],
    };
    let mut bytes = [0xa5_u8; 16];
    let error = writer
        .execute(
            &mut bytes,
            PlacementSite {
                base_address: 0,
                phase: PlacementPhase::PostHandoff,
                machine_regime: None,
                installation_scope: None,
            },
            |_| Some(0x1122_3344_5566_7788),
        )
        .expect_err("invalid later step must reject before direct writes begin");
    assert!(error.0.contains("outside"));
    assert_eq!(bytes, [0xa5; 16]);
}

#[test]
fn writer_application_stages_late_failure_and_retry_atomically() {
    let first = PostHandoffWriterStep {
        write: MaterializationWrite {
            field: "first".into(),
            target: entry(),
            container_byte_offset: 0,
            container_width_bits: 64,
            destination_lsb: 0,
            source_lsb: 0,
            width: 64,
            stored_integer_fit: None,
        },
        source: PostHandoffWriterSource::Resolve(entry()),
    };
    let outside = PostHandoffWriterStep {
        write: MaterializationWrite {
            field: "outside".into(),
            container_byte_offset: 16,
            ..first.write.clone()
        },
        source: first.source,
    };
    let values = [0x1122_3344_5566_7788, 0x99aa_bbcc_ddee_ff00];
    let mut bytes = [0xa5_u8; 16];

    let error = apply_post_handoff_writes_atomically(
        &mut bytes,
        ByteOrder::LittleEndian,
        &[first.clone(), outside.clone()],
        &values,
    )
    .expect_err("late application failure must reject the staged image");
    assert!(error.0.contains("outside"));
    assert_eq!(bytes, [0xa5; 16]);

    let mut repaired = outside;
    repaired.write.container_byte_offset = 8;
    apply_post_handoff_writes_atomically(
        &mut bytes,
        ByteOrder::LittleEndian,
        &[first, repaired],
        &values,
    )
    .expect("repaired staged image commits once");
    assert_eq!(
        bytes,
        [
            0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0x00, 0xff, 0xee, 0xdd, 0xcc, 0xbb,
            0xaa, 0x99,
        ]
    );
}

#[test]
fn fixed_entry_constant_folds_split_little_endian_bytes() {
    let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
    let plan = derive_symbolic_materialization(
        &split_layout(),
        &[symbolic],
        MaterializationContext {
            consumption: ConsumptionInstant::BeforeOmegaEntry,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: PlacementConstraints::unconstrained(PlacementPhase::Load),
        },
        |_| Some(0x1122_3344_5566_7788),
    )
    .expect("fixed-address fragments constant-fold");
    let mut bytes = [0_u8; 16];
    plan.materialize_resolved_into(&mut bytes)
        .expect("all writes resolved");

    assert_eq!(&bytes[0..4], &[0x88, 0x77, 0x66, 0x55]);
    assert_eq!(&bytes[8..12], &[0x44, 0x33, 0x22, 0x11]);
}

#[test]
fn symbolic_stored_integer_fit_is_enforced_before_each_consumption_phase() {
    let layout = LayoutPlanReport {
        schema_identity: 1,
        entries: vec![LayoutFieldEntryReport {
            field: "address".into(),
            member_identity: None,
            placement: LayoutPlacementReport::IntegerAt {
                offset: 0,
                stored_width: 32,
                interpretation: IntegerInterpretation::Unsigned,
            },
        }],
        offsets: None,
        size: Some(4),
        align: 4,
    };
    let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
    let context = MaterializationContext {
        consumption: ConsumptionInstant::BeforeOmegaEntry,
        byte_order: ByteOrder::LittleEndian,
        native_pointer_relocation_bits: Some(64),
        placement: PlacementConstraints::unconstrained(PlacementPhase::Load),
    };
    let plan =
        derive_symbolic_materialization(&layout, std::slice::from_ref(&symbolic), context, |_| {
            Some(0x1234_5678)
        })
        .expect("a resolved fitting symbolic value supplies a concrete fit proof");
    let mut bytes = [0xa5_u8; 4];

    let mut tampered = plan.clone();
    let MaterializationAction::ResolvedWrite { source_value, .. } = &mut tampered.actions[0] else {
        panic!("resolved derivation must retain a resolved write")
    };
    *source_value = 1_u64 << 32;
    let error = tampered
        .materialize_resolved_into(&mut bytes)
        .expect_err("resolved execution must replay retained stored-integer fit evidence");
    assert!(error.0.contains("does not fit"), "{}", error.0);
    assert_eq!(
        bytes, [0xa5; 4],
        "tampered resolved values reject before destination mutation"
    );

    plan.materialize_resolved_into(&mut bytes)
        .expect("the resolved value should use the exact stored width");
    assert_eq!(bytes, [0x78, 0x56, 0x34, 0x12]);

    let error =
        derive_symbolic_materialization(&layout, std::slice::from_ref(&symbolic), context, |_| {
            Some(1_u64 << 32)
        })
        .expect_err("an out-of-range symbolic value must reject");
    assert!(error.0.contains("does not fit"), "{}", error.0);

    let post_handoff = derive_symbolic_materialization(
        &layout,
        std::slice::from_ref(&symbolic),
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
        |_| None,
    )
    .expect("a post-handoff resolver can discharge stored-integer fit");
    assert!(matches!(
        post_handoff.actions.as_slice(),
        [MaterializationAction::RuntimeWriter(write)]
            if write.stored_integer_fit.is_some()
    ));
    let writer = post_handoff
        .derive_post_handoff_writer()
        .expect("stored-integer runtime action derives a writer");
    let invocation = writer
        .lower_reusable_fragment()
        .expect("stored-integer fit remains invocation evidence");
    assert_eq!(invocation.fit_constraints().len(), 1);
    invocation
        .validate_structure()
        .expect("stored-integer fit binds exact generated geometry");
    let mut drifted_fit = invocation.clone();
    drifted_fit.fit_constraints[0].source_slot = 1;
    let error = drifted_fit
        .validate_structure()
        .expect_err("fit evidence cannot move to a missing source slot");
    assert!(error.0.contains("does not bind"), "{}", error.0);
    invocation
        .validate_structure()
        .expect("fit rejection leaves the original invocation reusable");

    let site = PlacementSite {
        base_address: 0,
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        installation_scope: None,
    };
    let mut bytes = [0xa5_u8; 4];
    writer
        .execute(&mut bytes, site, |_| Some(0x1234_5678))
        .expect("resolved post-handoff value fits stored width");
    assert_eq!(bytes, [0x78, 0x56, 0x34, 0x12]);

    bytes.fill(0xa5);
    let error = writer
        .execute(&mut bytes, site, |_| Some(1_u64 << 32))
        .expect_err("post-handoff resolution must reject an out-of-range value");
    assert!(error.0.contains("does not fit"), "{}", error.0);
    assert_eq!(bytes, [0xa5; 4], "fit rejection must precede every write");

    let error = derive_symbolic_materialization(&layout, &[symbolic], context, |_| None)
        .expect_err("a loader cannot defer stored-integer fit to Omega");
    assert!(error.0.contains("before Omega entry"), "{}", error.0);
}

#[test]
fn unresolved_loader_consumed_fragments_reject() {
    let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
    let error = derive_symbolic_materialization(
        &split_layout(),
        &[symbolic],
        MaterializationContext {
            consumption: ConsumptionInstant::BeforeOmegaEntry,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: PlacementConstraints::unconstrained(PlacementPhase::Load),
        },
        |_| None,
    )
    .expect_err("a loader cannot apply split pointer relocations");

    assert!(error.0.contains("before Omega entry"));
}

#[test]
fn whole_pointer_uses_loader_native_relocation() {
    let layout = LayoutPlanReport {
        schema_identity: 1,
        entries: vec![LayoutFieldEntryReport {
            field: "entry".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset: 8 },
        }],
        offsets: Some(vec![8]),
        size: Some(16),
        align: 8,
    };
    let symbolic = SymbolicFieldValue::new("entry", 64, entry()).expect("symbolic field");
    let plan = derive_symbolic_materialization(
        &layout,
        &[symbolic],
        MaterializationContext {
            consumption: ConsumptionInstant::BeforeOmegaEntry,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: PlacementConstraints::unconstrained(PlacementPhase::Load),
        },
        |_| None,
    )
    .expect("whole-pointer native relocation is available");

    assert!(matches!(
        plan.actions.as_slice(),
        [MaterializationAction::NativePointerRelocation {
            destination_byte_offset: 8,
            width_bits: 64,
            ..
        }]
    ));
    assert!(
        plan.derive_post_handoff_writer()
            .expect_err("loader relocation is not a writer instruction")
            .0
            .contains("loader-native")
    );
}

#[test]
fn unresolved_action_cannot_partially_materialize() {
    let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
    let plan = derive_symbolic_materialization(
        &split_layout(),
        &[symbolic],
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
        |_| None,
    )
    .expect("writer plan");
    let mut bytes = [0xa5_u8; 16];
    assert!(plan.materialize_resolved_into(&mut bytes).is_err());
    assert_eq!(bytes, [0xa5; 16]);

    let writer = plan
        .derive_post_handoff_writer()
        .expect("runtime actions form a writer program");
    let mut missing = [0xa5_u8; 16];
    assert!(
        writer
            .execute(
                &mut missing,
                PlacementSite {
                    base_address: 0,
                    phase: PlacementPhase::PostHandoff,
                    machine_regime: None,
                    installation_scope: None,
                },
                |_| None,
            )
            .is_err()
    );
    assert_eq!(missing, [0xa5; 16]);
}

#[test]
fn placement_constraints_join_layout_alignment_and_validate_all_axes() {
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
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: constraints,
        },
        |_| None,
    )
    .expect("constrained post-handoff plan");

    assert_eq!(plan.placement.alignment(), 4096);
    plan.placement
        .validate_site(
            plan.byte_len,
            PlacementSite {
                base_address: 0x8000,
                phase: PlacementPhase::PostHandoff,
                machine_regime: Some(regime),
                installation_scope: Some(scope),
            },
        )
        .expect("all concrete placement facts match");

    let wrong_phase = PlacementSite {
        base_address: 0x8000,
        phase: PlacementPhase::Load,
        machine_regime: Some(regime),
        installation_scope: Some(scope),
    };
    assert!(
        plan.placement
            .validate_site(plan.byte_len, wrong_phase)
            .expect_err("phase is part of the normalized constraint")
            .0
            .contains("phase")
    );
    let writer = plan
        .derive_post_handoff_writer()
        .expect("runtime actions form a writer program");
    let mut unchanged = [0xa5_u8; 16];
    assert!(
        writer
            .execute(&mut unchanged, wrong_phase, |_| Some(1))
            .is_err()
    );
    assert_eq!(unchanged, [0xa5; 16]);

    let misaligned = PlacementSite {
        base_address: 0x8001,
        phase: PlacementPhase::PostHandoff,
        ..wrong_phase
    };
    assert!(
        plan.placement
            .validate_site(plan.byte_len, misaligned)
            .expect_err("layout and policy alignment are mandatory")
            .0
            .contains("aligned")
    );

    let wrong_regime = PlacementSite {
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        ..wrong_phase
    };
    assert!(
        plan.placement
            .validate_site(plan.byte_len, wrong_regime)
            .expect_err("machine regime is required")
            .0
            .contains("regime")
    );

    let wrong_scope = PlacementSite {
        machine_regime: Some(regime),
        installation_scope: None,
        ..wrong_regime
    };
    assert!(
        plan.placement
            .validate_site(plan.byte_len, wrong_scope)
            .expect_err("installation scope is required")
            .0
            .contains("scope")
    );

    let outside_range = PlacementSite {
        base_address: 0x10_0000,
        installation_scope: Some(scope),
        ..wrong_scope
    };
    assert!(
        plan.placement
            .validate_site(plan.byte_len, outside_range)
            .expect_err("complete placement must fit the range")
            .0
            .contains("outside")
    );
}

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
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: constraints,
        },
        |_| None,
    )
    .expect_err("sixteen bytes cannot fit an eight-byte range");

    assert!(error.0.contains("cannot fit"));
}
