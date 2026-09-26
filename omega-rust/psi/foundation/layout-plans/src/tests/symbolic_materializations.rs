use super::{
    data, deeply_nested_layout, entry, nested_layout, post_handoff_context, record_interior,
    repeated_layout, split_layout,
};
use crate::{
    ByteOrder, ConsumptionInstant, IntegerInterpretation, LayoutFieldEntryReport,
    LayoutPlacementReport, LayoutPlanReport, MaterializationAction, MaterializationContext,
    PlacementAddressRange, PlacementConstraints, PlacementPhase, PlacementSite, RelocationTarget,
    SymbolicFieldInnerLayout, SymbolicFieldPathSegment, SymbolicFieldValue,
    derive_symbolic_materialization, derive_symbolic_materialization_with_inner_layouts,
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

#[test]
fn symbolic_index_materialization_assigns_the_exact_element() {
    let symbolic = SymbolicFieldValue::new_indexed_numbered("handlers", 9, 2, 64, entry())
        .expect("indexed field");
    let plan = derive_symbolic_materialization(
        &repeated_layout(),
        &[symbolic],
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
        |_| None,
    )
    .expect("an indexed field/index path derives one element write");

    assert_eq!(plan.actions.len(), 1);
    let MaterializationAction::RuntimeWriter(write) = &plan.actions[0] else {
        panic!("an unresolved indexed symbolic derives a runtime writer");
    };
    assert_eq!(write.container_byte_offset, 16);

    let writer = plan.derive_post_handoff_writer().expect("writer");
    let mut bytes = [0xa5_u8; 32];
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
                Some(0x1122_3344_5566_7788)
            },
        )
        .expect("the indexed writer resolves the exact element");

    assert_eq!(&bytes[16..24], &0x1122_3344_5566_7788_u64.to_le_bytes());
    assert!(
        bytes[..16]
            .iter()
            .chain(&bytes[24..])
            .all(|byte| *byte == 0xa5),
        "index materialization writes only the addressed element"
    );
}

#[test]
fn symbolic_index_materialization_resolves_the_exact_element() {
    let symbolic = SymbolicFieldValue::new_indexed_numbered("handlers", 9, 3, 64, entry())
        .expect("indexed field");
    let plan = derive_symbolic_materialization(
        &repeated_layout(),
        &[symbolic],
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
        |_| Some(0xdead_beef),
    )
    .expect("a resolved indexed symbolic produces a resolved write");

    assert_eq!(plan.actions.len(), 1);
    assert!(matches!(
        plan.actions[0],
        MaterializationAction::ResolvedWrite { .. }
    ));
    let mut bytes = [0_u8; 32];
    plan.materialize_resolved_into(&mut bytes)
        .expect("resolved indexed write materializes");
    assert_eq!(&bytes[24..32], &0xdead_beef_u64.to_le_bytes());
    assert!(bytes[..24].iter().all(|byte| *byte == 0));
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
fn symbolic_index_materialization_coexists_across_elements() {
    let fields = [
        SymbolicFieldValue::new_indexed_numbered("handlers", 9, 0, 64, entry())
            .expect("first element"),
        SymbolicFieldValue::new_indexed_numbered("handlers", 9, 3, 64, data())
            .expect("last element"),
    ];
    let plan = derive_symbolic_materialization(
        &repeated_layout(),
        &fields,
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
        |_| None,
    )
    .expect("distinct element indices are distinct materialization slots");

    let offsets = plan
        .actions
        .iter()
        .map(|action| match action {
            MaterializationAction::RuntimeWriter(write) => write.container_byte_offset,
            other => panic!("expected runtime writers, found {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(offsets, [0, 24]);

    let error = derive_symbolic_materialization(
        &repeated_layout(),
        &[
            fields[0].clone(),
            SymbolicFieldValue::new_indexed_numbered("handlers", 9, 0, 64, data())
                .expect("duplicate element"),
        ],
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
        |_| None,
    )
    .expect_err("the same field/index slot cannot be supplied twice");
    assert!(
        error.0.contains("`handlers[0]` is supplied more than once"),
        "{}",
        error.0
    );
}

#[test]
fn symbolic_index_materialization_rejects_before_target_resolution() {
    let out_of_range = SymbolicFieldValue::new_indexed_numbered("handlers", 9, 4, 64, entry())
        .expect("indexed field");
    let mut resolutions = 0;
    let error = derive_symbolic_materialization(
        &repeated_layout(),
        &[out_of_range],
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
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: None,
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
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
fn symbolic_inner_materialization_assigns_the_exact_member() {
    let (layout, carrier) = nested_layout();
    let symbolic = [
        SymbolicFieldValue::new("slot", 64, entry())
            .expect("outer record field")
            .with_inner_segment(SymbolicFieldPathSegment::new("entry")),
        SymbolicFieldValue::new("slot", 64, data())
            .expect("outer record field")
            .with_inner_segment(SymbolicFieldPathSegment::new("flags")),
    ];
    let plan = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        &symbolic,
        post_handoff_context(),
        |_| None,
    )
    .expect("two inner paths share one interior carrier");

    let writes = plan
        .actions
        .iter()
        .map(|action| match action {
            MaterializationAction::RuntimeWriter(write) => {
                (write.field.as_str(), write.container_byte_offset)
            }
            other => panic!("expected runtime writers, found {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(writes, [("slot.entry", 8), ("slot.flags", 16)]);

    let writer = plan.derive_post_handoff_writer().expect("writer");
    let mut bytes = [0xa5_u8; 24];
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
                Some(match target {
                    RelocationTarget::Entry(_) => 0x1122_3344_5566_7788,
                    RelocationTarget::Data(_) => 0xdead_beef,
                })
            },
        )
        .expect("the inner writers resolve each exact member");
    assert_eq!(&bytes[8..16], &0x1122_3344_5566_7788_u64.to_le_bytes());
    assert_eq!(&bytes[16..24], &0xdead_beef_u64.to_le_bytes());
    assert!(
        bytes[..8].iter().all(|byte| *byte == 0xa5),
        "inner materialization writes only the addressed members"
    );
}

#[test]
fn symbolic_inner_materialization_composes_through_an_indexed_outer() {
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: (0..4)
            .map(|index| LayoutFieldEntryReport {
                field: "handlers".into(),
                member_identity: Some(9),
                placement: LayoutPlacementReport::At { offset: index * 16 },
            })
            .collect(),
        offsets: None,
        size: Some(64),
        align: 8,
    };
    let carrier = SymbolicFieldInnerLayout::new_numbered(
        "handlers",
        9,
        LayoutPlanReport {
            schema_report_fingerprint: 2,
            entries: vec![
                LayoutFieldEntryReport {
                    field: "entry".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 0 },
                },
                LayoutFieldEntryReport {
                    field: "flags".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 8 },
                },
            ],
            offsets: Some(vec![0, 8]),
            size: Some(16),
            align: 8,
        },
    );
    let symbolic = SymbolicFieldValue::new_indexed_numbered("handlers", 9, 2, 64, entry())
        .expect("indexed outer record")
        .with_inner_segment(SymbolicFieldPathSegment::new("flags"));
    let plan = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        &[carrier],
        std::slice::from_ref(&symbolic),
        post_handoff_context(),
        |_| None,
    )
    .expect("the inner hop composes onto the selected element's offset");

    assert_eq!(plan.actions.len(), 1);
    let MaterializationAction::RuntimeWriter(write) = &plan.actions[0] else {
        panic!("an indexed inner path derives a runtime writer");
    };
    assert_eq!(write.field, "handlers[2].flags");
    assert_eq!(write.container_byte_offset, 32 + 8);
}

#[test]
fn symbolic_inner_materialization_addresses_an_indexed_inner_member() {
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![LayoutFieldEntryReport {
            field: "slot".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset: 8 },
        }],
        offsets: Some(vec![8]),
        size: Some(40),
        align: 8,
    };
    let carrier = SymbolicFieldInnerLayout::new(
        "slot",
        LayoutPlanReport {
            schema_report_fingerprint: 2,
            entries: (0..4)
                .map(|index| LayoutFieldEntryReport {
                    field: "items".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: index * 8 },
                })
                .collect(),
            offsets: None,
            size: Some(32),
            align: 8,
        },
    );
    let symbolic = SymbolicFieldValue::new("slot", 64, entry())
        .expect("outer record field")
        .with_inner_segment(SymbolicFieldPathSegment::new_indexed("items", 1));
    let plan = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&symbolic),
        post_handoff_context(),
        |_| None,
    )
    .expect("the inner index selects one element of the inner repeated field");

    assert_eq!(plan.actions.len(), 1);
    let MaterializationAction::RuntimeWriter(write) = &plan.actions[0] else {
        panic!("an unresolved indexed inner path derives a runtime writer");
    };
    assert_eq!(write.field, "slot.items[1]");
    assert_eq!(write.container_byte_offset, 8 + 8);
}

#[test]
fn symbolic_inner_materialization_resolves_and_relocates_the_member() {
    let (layout, carrier) = nested_layout();
    let symbolic = SymbolicFieldValue::new("slot", 64, entry())
        .expect("outer record field")
        .with_inner_segment(SymbolicFieldPathSegment::new("entry"));

    let resolved = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&symbolic),
        post_handoff_context(),
        |_| Some(0xdead_beef),
    )
    .expect("a resolved inner symbolic produces a resolved write");
    let mut bytes = [0_u8; 24];
    resolved
        .materialize_resolved_into(&mut bytes)
        .expect("resolved inner write materializes");
    assert_eq!(&bytes[8..16], &0xdead_beef_u64.to_le_bytes());
    assert!(bytes[..8].iter().chain(&bytes[16..]).all(|byte| *byte == 0));

    let plan = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&symbolic),
        MaterializationContext {
            consumption: ConsumptionInstant::BeforeOmegaEntry,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: PlacementConstraints::unconstrained(PlacementPhase::Load),
        },
        |_| None,
    )
    .expect("an inner whole pointer uses the loader-native relocation");
    assert!(matches!(
        plan.actions.as_slice(),
        [MaterializationAction::NativePointerRelocation {
            field,
            destination_byte_offset: 8,
            width_bits: 64,
            ..
        }] if field == "slot.entry"
    ));
}

#[test]
fn symbolic_inner_materialization_requires_a_bound_traversed_carrier() {
    let (layout, carrier) = nested_layout();
    let inner_path = SymbolicFieldValue::new("slot", 64, entry())
        .expect("outer record field")
        .with_inner_segment(SymbolicFieldPathSegment::new("entry"));

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
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        &[flat],
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a carrier no symbolic path traverses must reject");
    assert!(
        error
            .0
            .contains("no symbolic field path traverses the supplied inner layout for `slot`"),
        "{}",
        error.0
    );

    let orphan = SymbolicFieldInnerLayout::new("missing", record_interior(&carrier));
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        &[orphan],
        std::slice::from_ref(&inner_path),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a carrier must bind to a field the outer plan contains");
    assert!(
        error.0.contains(
            "inner layout for `missing` binds to a field the validated layout plan does not contain"
        ),
        "{}",
        error.0
    );

    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        &[carrier.clone(), carrier.clone()],
        std::slice::from_ref(&inner_path),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("one outer field accepts one interior carrier");
    assert!(
        error
            .0
            .contains("inner layout for `slot` is supplied more than once"),
        "{}",
        error.0
    );

    let unsized_carrier = SymbolicFieldInnerLayout::new(
        "slot",
        LayoutPlanReport {
            size: None,
            ..record_interior(&carrier)
        },
    );
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        &[unsized_carrier],
        std::slice::from_ref(&inner_path),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("the interior layout must be fixed-size");
    assert!(
        error
            .0
            .contains("inner layout for `slot` requires a fixed-size interior layout plan"),
        "{}",
        error.0
    );
}

#[test]
fn symbolic_inner_materialization_rejects_ambiguous_or_missing_members() {
    let (layout, carrier) = nested_layout();

    // An unindexed path into a repeated outer record cannot name one element.
    let repeated_outer = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: (0..4)
            .map(|index| LayoutFieldEntryReport {
                field: "handlers".into(),
                member_identity: Some(9),
                placement: LayoutPlacementReport::At { offset: index * 16 },
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
    let error = derive_symbolic_materialization_with_inner_layouts(
        &repeated_outer,
        &[repeated_carrier],
        std::slice::from_ref(&ambiguous),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("an unindexed inner path cannot cover several elements");
    assert!(
        error
            .0
            .contains("requires the outer field `handlers` to resolve to exactly one element placement, found 4"),
        "{}",
        error.0
    );

    // A fragmented or stored-integer outer placement cannot enclose a record.
    let integer_outer = LayoutPlanReport {
        schema_report_fingerprint: 1,
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
    let integer_carrier = SymbolicFieldInnerLayout::new("address", record_interior(&carrier));
    let through_integer = SymbolicFieldValue::new("address", 64, entry())
        .expect("stored integer outer")
        .with_inner_segment(SymbolicFieldPathSegment::new("entry"));
    let error = derive_symbolic_materialization_with_inner_layouts(
        &integer_outer,
        &[integer_carrier],
        std::slice::from_ref(&through_integer),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("an inner hop requires a whole-record outer placement");
    assert!(
        error
            .0
            .contains("inner layout for `address` requires a whole `At` placement"),
        "{}",
        error.0
    );

    // A fragmented placement is the same dead end: bit containers describe a
    // scalar's storage, not the contiguous record extent an interior needs.
    let bits_outer = LayoutPlanReport {
        entries: vec![LayoutFieldEntryReport {
            field: "address".into(),
            member_identity: None,
            placement: LayoutPlacementReport::Bits {
                container: 0,
                container_width: 32,
                destination_lsb: 0,
                source_lsb: 0,
                width: 32,
            },
        }],
        ..integer_outer.clone()
    };
    let bits_carrier = SymbolicFieldInnerLayout::new("address", record_interior(&carrier));
    let error = derive_symbolic_materialization_with_inner_layouts(
        &bits_outer,
        &[bits_carrier],
        std::slice::from_ref(&through_integer),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a fragmented outer placement cannot enclose a record interior");
    assert!(
        error
            .0
            .contains("inner layout for `address` requires a whole `At` placement"),
        "{}",
        error.0
    );

    // The inner member must exist in the retained interior plan.
    let missing_member = SymbolicFieldValue::new("slot", 64, entry())
        .expect("outer record field")
        .with_inner_segment(SymbolicFieldPathSegment::new("missing"));
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&missing_member),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("an absent inner member cannot materialize");
    assert!(
        error
            .0
            .contains("`slot.missing` has no entry in the inner layout plan"),
        "{}",
        error.0
    );

    // Duplicate two-segment paths collide even though their outer fields match.
    let duplicate = SymbolicFieldValue::new("slot", 64, data())
        .expect("outer record field")
        .with_inner_segment(SymbolicFieldPathSegment::new("entry"));
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        &[
            SymbolicFieldValue::new("slot", 64, entry())
                .expect("outer record field")
                .with_inner_segment(SymbolicFieldPathSegment::new("entry")),
            duplicate,
        ],
        post_handoff_context(),
        |_| None,
    )
    .expect_err("the same inner path cannot be supplied twice");
    assert!(
        error.0.contains("`slot.entry` is supplied more than once"),
        "{}",
        error.0
    );
}

#[test]
fn symbolic_inner_materialization_bounds_writes_to_the_interior_extent() {
    let (layout, carrier) = nested_layout();
    // A malformed carrier whose member lands past its own declared size must
    // reject before composition: the composed write would land inside a
    // neighboring outer field while still passing the whole-plan bound.
    let mut oversized = record_interior(&carrier);
    oversized.entries.push(LayoutFieldEntryReport {
        field: "escape".into(),
        member_identity: None,
        placement: LayoutPlacementReport::At { offset: 24 },
    });
    let carrier = SymbolicFieldInnerLayout::new("slot", oversized);
    let symbolic = SymbolicFieldValue::new("slot", 64, entry())
        .expect("outer record field")
        .with_inner_segment(SymbolicFieldPathSegment::new("escape"));
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&symbolic),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("an inner write may not escape the interior record extent");
    assert!(
        error
            .0
            .contains("writes outside the 16-byte materialization"),
        "{}",
        error.0
    );
}

#[test]
fn symbolic_inner_materialization_composes_through_a_second_record_boundary() {
    let (layout, carrier) = deeply_nested_layout();
    let symbolic = [
        SymbolicFieldValue::new("slot", 64, entry())
            .expect("outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new("sub")
                    .with_inner_segment(SymbolicFieldPathSegment::new("entry")),
            ),
        SymbolicFieldValue::new("slot", 64, data())
            .expect("outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new("sub")
                    .with_inner_segment(SymbolicFieldPathSegment::new("flags")),
            ),
        SymbolicFieldValue::new("slot", 64, data())
            .expect("outer record field")
            .with_inner_segment(SymbolicFieldPathSegment::new("pad")),
    ];
    let plan = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        &symbolic,
        post_handoff_context(),
        |_| None,
    )
    .expect("deep paths compose one offset per crossed record boundary");

    let writes = plan
        .actions
        .iter()
        .map(|action| match action {
            MaterializationAction::RuntimeWriter(write) => {
                (write.field.as_str(), write.container_byte_offset)
            }
            other => panic!("expected runtime writers, found {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        writes,
        [
            ("slot.sub.entry", 16),
            ("slot.sub.flags", 24),
            ("slot.pad", 8)
        ]
    );

    let mut entry_resolutions = 0;
    let writer = plan.derive_post_handoff_writer().expect("writer");
    let mut bytes = [0xa5_u8; 40];
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
                Some(match target {
                    RelocationTarget::Entry(_) => {
                        entry_resolutions += 1;
                        0x1122_3344_5566_7788
                    }
                    RelocationTarget::Data(_) => 0xdead_beef,
                })
            },
        )
        .expect("the deeper writers resolve each exact member");
    assert_eq!(entry_resolutions, 1);
    assert_eq!(&bytes[16..24], &0x1122_3344_5566_7788_u64.to_le_bytes());
    assert_eq!(&bytes[24..32], &0xdead_beef_u64.to_le_bytes());
    assert_eq!(&bytes[8..16], &0xdead_beef_u64.to_le_bytes());
    assert!(
        bytes[..8]
            .iter()
            .chain(&bytes[32..])
            .all(|byte| *byte == 0xa5),
        "deeper materialization writes only the addressed members"
    );
}

#[test]
fn symbolic_inner_materialization_composes_indexed_boundaries_at_each_level() {
    // `slots[1].subs[0].flags` indexes at both crossed record boundaries: the
    // outer repeated record `slots` and the repeated record `subs` inside it.
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: (0..2)
            .map(|index| LayoutFieldEntryReport {
                field: "slots".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: index * 48 },
            })
            .collect(),
        offsets: None,
        size: Some(96),
        align: 8,
    };
    let carrier = SymbolicFieldInnerLayout::new(
        "slots",
        LayoutPlanReport {
            schema_report_fingerprint: 2,
            entries: (0..2)
                .map(|index| LayoutFieldEntryReport {
                    field: "subs".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: index * 24 },
                })
                .chain(std::iter::once(LayoutFieldEntryReport {
                    field: "mark".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 40 },
                }))
                .collect(),
            offsets: None,
            size: Some(48),
            align: 8,
        },
    )
    .with_inner_layout(SymbolicFieldInnerLayout::new(
        "subs",
        LayoutPlanReport {
            schema_report_fingerprint: 3,
            entries: vec![
                LayoutFieldEntryReport {
                    field: "entry".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 0 },
                },
                LayoutFieldEntryReport {
                    field: "flags".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 16 },
                },
            ],
            offsets: Some(vec![0, 16]),
            size: Some(24),
            align: 8,
        },
    ));
    let symbolic = [
        SymbolicFieldValue::new_indexed("slots", 1, 64, entry())
            .expect("indexed outer record element")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_indexed("subs", 0)
                    .with_inner_segment(SymbolicFieldPathSegment::new("flags")),
            ),
        SymbolicFieldValue::new_indexed("slots", 0, 64, data())
            .expect("indexed outer record element")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_indexed("subs", 1)
                    .with_inner_segment(SymbolicFieldPathSegment::new("entry")),
            ),
        SymbolicFieldValue::new_indexed("slots", 1, 64, data())
            .expect("indexed outer record element")
            .with_inner_segment(SymbolicFieldPathSegment::new("mark")),
    ];
    let plan = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        &symbolic,
        post_handoff_context(),
        |_| None,
    )
    .expect("each index hop selects the exact element at its boundary");

    let writes = plan
        .actions
        .iter()
        .map(|action| match action {
            MaterializationAction::RuntimeWriter(write) => {
                (write.field.as_str(), write.container_byte_offset)
            }
            other => panic!("expected runtime writers, found {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        writes,
        [
            ("slots[1].subs[0].flags", 48 + 16),
            ("slots[0].subs[1].entry", 24),
            ("slots[1].mark", 48 + 40),
        ]
    );
}

#[test]
fn symbolic_inner_materialization_relocates_a_deeper_member() {
    let (layout, carrier) = deeply_nested_layout();
    let symbolic = SymbolicFieldValue::new("slot", 64, entry())
        .expect("outer record field")
        .with_inner_segment(
            SymbolicFieldPathSegment::new("sub")
                .with_inner_segment(SymbolicFieldPathSegment::new("args")),
        );
    let plan = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&symbolic),
        MaterializationContext {
            consumption: ConsumptionInstant::BeforeOmegaEntry,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: PlacementConstraints::unconstrained(PlacementPhase::Load),
        },
        |_| None,
    )
    .expect("a deeper whole pointer uses the loader-native relocation");
    assert!(matches!(
        plan.actions.as_slice(),
        [MaterializationAction::NativePointerRelocation {
            field,
            destination_byte_offset: 32,
            width_bits: 64,
            ..
        }] if field == "slot.sub.args"
    ));
}

#[test]
fn symbolic_inner_materialization_rejects_deeper_path_faults() {
    let (layout, carrier) = deeply_nested_layout();
    let deep_path = |member: &str| {
        SymbolicFieldValue::new("slot", 64, entry())
            .expect("outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new("sub")
                    .with_inner_segment(SymbolicFieldPathSegment::new(member)),
            )
    };

    // A second record boundary needs its own carrier: `slot`'s carrier alone
    // cannot resolve `sub`'s interior.
    let shallow = SymbolicFieldInnerLayout::new("slot", record_interior(&carrier));
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        &[shallow],
        std::slice::from_ref(&deep_path("entry")),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a deeper hop needs the nested carrier");
    assert!(
        error
            .0
            .contains("`slot.sub.entry` has no supplied inner layout for `slot.sub`"),
        "{}",
        error.0
    );

    // A nested carrier no path traverses rejects, just like a top-level one.
    let shallow_path = SymbolicFieldValue::new("slot", 64, entry())
        .expect("outer record field")
        .with_inner_segment(SymbolicFieldPathSegment::new("pad"));
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&shallow_path),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a nested carrier no path traverses must reject");
    assert!(
        error
            .0
            .contains("no symbolic field path traverses the supplied inner layout for `slot.sub`"),
        "{}",
        error.0
    );

    // A nested carrier must bind to a field the enclosing interior contains.
    let stray = SymbolicFieldInnerLayout::new("slot", record_interior(&carrier)).with_inner_layout(
        SymbolicFieldInnerLayout::new(
            "missing",
            LayoutPlanReport {
                schema_report_fingerprint: 4,
                entries: vec![],
                offsets: Some(vec![]),
                size: Some(8),
                align: 8,
            },
        ),
    );
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        &[stray],
        std::slice::from_ref(&shallow_path),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a nested carrier must bind inside the enclosing interior");
    assert!(
        error.0.contains(
            "inner layout for `slot.missing` binds to a field the enclosing interior layout plan does not contain"
        ),
        "{}",
        error.0
    );

    // The same inner path cannot be supplied twice at any depth.
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        &[
            deep_path("entry"),
            SymbolicFieldValue::new("slot", 64, data())
                .expect("outer record field")
                .with_inner_segment(
                    SymbolicFieldPathSegment::new("sub")
                        .with_inner_segment(SymbolicFieldPathSegment::new("entry")),
                ),
        ],
        post_handoff_context(),
        |_| None,
    )
    .expect_err("the same deeper path cannot be supplied twice");
    assert!(
        error
            .0
            .contains("`slot.sub.entry` is supplied more than once"),
        "{}",
        error.0
    );
}

#[test]
fn symbolic_inner_materialization_bounds_each_record_boundary() {
    let (layout, carrier) = deeply_nested_layout();
    let deep_path = |member: &str| {
        SymbolicFieldValue::new("slot", 64, entry())
            .expect("outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new("sub")
                    .with_inner_segment(SymbolicFieldPathSegment::new(member)),
            )
    };

    // An unindexed hop into a repeated interior record cannot name one element.
    let repeated_middle = SymbolicFieldInnerLayout::new(
        "slot",
        LayoutPlanReport {
            schema_report_fingerprint: 2,
            entries: (0..2)
                .map(|index| LayoutFieldEntryReport {
                    field: "subs".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: index * 24 },
                })
                .collect(),
            offsets: None,
            size: Some(48),
            align: 8,
        },
    )
    .with_inner_layout(SymbolicFieldInnerLayout::new(
        "subs",
        LayoutPlanReport {
            schema_report_fingerprint: 3,
            entries: vec![LayoutFieldEntryReport {
                field: "entry".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            }],
            offsets: Some(vec![0]),
            size: Some(24),
            align: 8,
        },
    ));
    let ambiguous = SymbolicFieldValue::new("slot", 64, entry())
        .expect("outer record field")
        .with_inner_segment(
            SymbolicFieldPathSegment::new("subs")
                .with_inner_segment(SymbolicFieldPathSegment::new("entry")),
        );
    // `slot` spans offset 8 of the 40-byte outer plan, so its interior is
    // bounded to 32 bytes; the repeated-`subs` carrier legitimately claims 48,
    // so the outer plan must offer the room before the ambiguity check runs.
    let outer_56 = LayoutPlanReport {
        size: Some(56),
        ..layout.clone()
    };
    let error = derive_symbolic_materialization_with_inner_layouts(
        &outer_56,
        &[repeated_middle],
        std::slice::from_ref(&ambiguous),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("an unindexed deeper hop cannot cover several elements");
    assert!(
        error.0.contains(
            "requires the enclosing field `slot.subs` to resolve to exactly one element placement, found 2"
        ),
        "{}",
        error.0
    );

    // A stored-integer interior field cannot enclose another record.
    let integer_middle = SymbolicFieldInnerLayout::new(
        "slot",
        LayoutPlanReport {
            schema_report_fingerprint: 2,
            entries: vec![LayoutFieldEntryReport {
                field: "sub".into(),
                member_identity: None,
                placement: LayoutPlacementReport::IntegerAt {
                    offset: 8,
                    stored_width: 64,
                    interpretation: IntegerInterpretation::Unsigned,
                },
            }],
            offsets: None,
            size: Some(32),
            align: 8,
        },
    )
    .with_inner_layout(SymbolicFieldInnerLayout::new(
        "sub",
        record_interior(&carrier.inner_layouts()[0]),
    ));
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        &[integer_middle],
        std::slice::from_ref(&deep_path("entry")),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a deeper hop requires a whole-record enclosing placement");
    assert!(
        error
            .0
            .contains("inner layout for `slot.sub` requires a whole `At` placement"),
        "{}",
        error.0
    );

    // A nested carrier whose claimed interior overflows the enclosing element
    // must reject at the boundary, before any member offset is composed.
    let overflowing_interior = SymbolicFieldInnerLayout::new("slot", record_interior(&carrier))
        .with_inner_layout(SymbolicFieldInnerLayout::new(
            "sub",
            LayoutPlanReport {
                schema_report_fingerprint: 3,
                entries: vec![LayoutFieldEntryReport {
                    field: "entry".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 0 },
                }],
                offsets: Some(vec![0]),
                // `sub` sits at offset 8 of `slot`'s 32-byte interior; claiming 32
                // more bytes reaches 40 and escapes into the outer object.
                size: Some(32),
                align: 8,
            },
        ));
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        &[overflowing_interior],
        std::slice::from_ref(&deep_path("entry")),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a nested interior may not exceed its enclosing element");
    assert!(
        error
            .0
            .contains("interior layout for `slot.sub` exceeds the enclosing 32-byte record extent"),
        "{}",
        error.0
    );

    // A leaf member may not escape the innermost interior extent either.
    let mut deep_escape = record_interior(&carrier.inner_layouts()[0]);
    deep_escape.entries.push(LayoutFieldEntryReport {
        field: "escape".into(),
        member_identity: None,
        placement: LayoutPlacementReport::At { offset: 20 },
    });
    let escaping = SymbolicFieldInnerLayout::new("slot", record_interior(&carrier))
        .with_inner_layout(SymbolicFieldInnerLayout::new("sub", deep_escape));
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        &[escaping],
        std::slice::from_ref(&deep_path("escape")),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a deeper leaf may not escape its interior record extent");
    assert!(
        error
            .0
            .contains("writes outside the 24-byte materialization"),
        "{}",
        error.0
    );
}
