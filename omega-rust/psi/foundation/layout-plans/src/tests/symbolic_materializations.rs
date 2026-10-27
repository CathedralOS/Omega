use super::{
    data, deeply_nested_layout, entry, layout_field_entry, nested_layout,
    numbered_layout_field_entry, post_handoff_context, post_handoff_context_with, record_interior,
    reject_inner_layout_materialization, repeated_layout, split_layout, symbolic_path,
};
use crate::{
    ByteOrder, ConsumptionInstant, IntegerInterpretation, LayoutPlacementReport, LayoutPlanReport,
    MaterializationAction, MaterializationContext, PlacementAddressRange, PlacementConstraints,
    PlacementPhase, SymbolicFieldInnerLayout, SymbolicFieldPathSegment, SymbolicFieldValue,
    derive_symbolic_materialization,
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
    let error = reject_inner_layout_materialization(
        &layout,
        std::slice::from_ref(&carrier),
        &[flat],
        "a carrier no symbolic path traverses must reject",
    );
    assert!(
        error.contains("no symbolic field path traverses the supplied inner layout for `slot`"),
        "{}",
        error
    );

    let orphan = SymbolicFieldInnerLayout::new("missing", record_interior(&carrier));
    let error = reject_inner_layout_materialization(
        &layout,
        &[orphan],
        std::slice::from_ref(&inner_path),
        "a carrier must bind to a field the outer plan contains",
    );
    assert!(
        error.contains(
            "inner layout for `missing` binds to a field the validated layout plan does not contain"
        ),
        "{}",
        error
    );

    let error = reject_inner_layout_materialization(
        &layout,
        &[carrier.clone(), carrier.clone()],
        std::slice::from_ref(&inner_path),
        "one outer field accepts one interior carrier",
    );
    assert!(
        error.contains("inner layout for `slot` is supplied more than once"),
        "{}",
        error
    );

    let unsized_carrier = SymbolicFieldInnerLayout::new(
        "slot",
        LayoutPlanReport {
            size: None,
            ..record_interior(&carrier)
        },
    );
    let error = reject_inner_layout_materialization(
        &layout,
        &[unsized_carrier],
        std::slice::from_ref(&inner_path),
        "the interior layout must be fixed-size",
    );
    assert!(
        error.contains("inner layout for `slot` requires a fixed-size interior layout plan"),
        "{}",
        error
    );
}

#[test]
fn symbolic_inner_materialization_rejects_ambiguous_or_missing_members() {
    let (layout, carrier) = nested_layout();

    // An unindexed path into a repeated outer record cannot name one element.
    let repeated_outer = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: (0..4)
            .map(|index| {
                numbered_layout_field_entry(
                    "handlers".into(),
                    9,
                    LayoutPlacementReport::At { offset: index * 16 },
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
    let error = reject_inner_layout_materialization(
        &repeated_outer,
        &[repeated_carrier],
        std::slice::from_ref(&ambiguous),
        "an unindexed inner path cannot cover several elements",
    );
    assert!(
        error
            .contains("requires the outer field `handlers` to resolve to exactly one element placement, found 4"),
        "{}",
        error
    );

    // A fragmented or stored-integer outer placement cannot enclose a record.
    let integer_outer = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![layout_field_entry(
            "address".into(),
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
    let integer_carrier = SymbolicFieldInnerLayout::new("address", record_interior(&carrier));
    let through_integer = SymbolicFieldValue::new("address", 64, entry())
        .expect("stored integer outer")
        .with_inner_segment(SymbolicFieldPathSegment::new("entry"));
    let error = reject_inner_layout_materialization(
        &integer_outer,
        &[integer_carrier],
        std::slice::from_ref(&through_integer),
        "an inner hop requires a whole-record outer placement",
    );
    assert!(
        error.contains("inner layout for `address` requires a whole `At` placement"),
        "{}",
        error
    );

    // A fragmented placement is the same dead end: bit containers describe a
    // scalar's storage, not the contiguous record extent an interior needs.
    let bits_outer = LayoutPlanReport {
        entries: vec![layout_field_entry(
            "address".into(),
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
    let bits_carrier = SymbolicFieldInnerLayout::new("address", record_interior(&carrier));
    let error = reject_inner_layout_materialization(
        &bits_outer,
        &[bits_carrier],
        std::slice::from_ref(&through_integer),
        "a fragmented outer placement cannot enclose a record interior",
    );
    assert!(
        error.contains("inner layout for `address` requires a whole `At` placement"),
        "{}",
        error
    );

    // The inner member must exist in the retained interior plan.
    let missing_member = SymbolicFieldValue::new("slot", 64, entry())
        .expect("outer record field")
        .with_inner_segment(SymbolicFieldPathSegment::new("missing"));
    let error = reject_inner_layout_materialization(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&missing_member),
        "an absent inner member cannot materialize",
    );
    assert!(
        error.contains("`slot.missing` has no entry in the inner layout plan"),
        "{}",
        error
    );

    // Duplicate two-segment paths collide even though their outer fields match.
    let duplicate = SymbolicFieldValue::new("slot", 64, data())
        .expect("outer record field")
        .with_inner_segment(SymbolicFieldPathSegment::new("entry"));
    let error = reject_inner_layout_materialization(
        &layout,
        std::slice::from_ref(&carrier),
        &[
            SymbolicFieldValue::new("slot", 64, entry())
                .expect("outer record field")
                .with_inner_segment(SymbolicFieldPathSegment::new("entry")),
            duplicate,
        ],
        "the same inner path cannot be supplied twice",
    );
    assert!(
        error.contains("`slot.entry` is supplied more than once"),
        "{}",
        error
    );
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
    let error = reject_inner_layout_materialization(
        &layout,
        &[shallow],
        std::slice::from_ref(&deep_path("entry")),
        "a deeper hop needs the nested carrier",
    );
    assert!(
        error.contains("`slot.sub.entry` has no supplied inner layout for `slot.sub`"),
        "{}",
        error
    );

    // A nested carrier no path traverses rejects, just like a top-level one.
    let shallow_path = SymbolicFieldValue::new("slot", 64, entry())
        .expect("outer record field")
        .with_inner_segment(SymbolicFieldPathSegment::new("pad"));
    let error = reject_inner_layout_materialization(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&shallow_path),
        "a nested carrier no path traverses must reject",
    );
    assert!(
        error.contains("no symbolic field path traverses the supplied inner layout for `slot.sub`"),
        "{}",
        error
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
    let error = reject_inner_layout_materialization(
        &layout,
        &[stray],
        std::slice::from_ref(&shallow_path),
        "a nested carrier must bind inside the enclosing interior",
    );
    assert!(
        error.contains(
            "inner layout for `slot.missing` binds to a field the enclosing interior layout plan does not contain"
        ),
        "{}",
        error
    );

    // The same inner path cannot be supplied twice at any depth.
    let error = reject_inner_layout_materialization(
        &layout,
        std::slice::from_ref(&carrier),
        &[
            deep_path("entry"),
            SymbolicFieldValue::new("slot", 64, data())
                .expect("outer record field")
                .with_inner_segment(symbolic_path(&["sub", "entry"])),
        ],
        "the same deeper path cannot be supplied twice",
    );
    assert!(
        error.contains("`slot.sub.entry` is supplied more than once"),
        "{}",
        error
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
                .map(|index| {
                    layout_field_entry(
                        "subs".into(),
                        LayoutPlacementReport::At { offset: index * 24 },
                    )
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
            entries: vec![layout_field_entry(
                "entry".into(),
                LayoutPlacementReport::At { offset: 0 },
            )],
            offsets: Some(vec![0]),
            size: Some(24),
            align: 8,
        },
    ));
    let ambiguous = SymbolicFieldValue::new("slot", 64, entry())
        .expect("outer record field")
        .with_inner_segment(symbolic_path(&["subs", "entry"]));
    // `slot` spans offset 8 of the 40-byte outer plan, so its interior is
    // bounded to 32 bytes; the repeated-`subs` carrier legitimately claims 48,
    // so the outer plan must offer the room before the ambiguity check runs.
    let outer_56 = LayoutPlanReport {
        size: Some(56),
        ..layout.clone()
    };
    let error = reject_inner_layout_materialization(
        &outer_56,
        &[repeated_middle],
        std::slice::from_ref(&ambiguous),
        "an unindexed deeper hop cannot cover several elements",
    );
    assert!(
        error.contains(
            "requires the enclosing field `slot.subs` to resolve to exactly one element placement, found 2"
        ),
        "{}",
        error
    );

    // A stored-integer interior field cannot enclose another record.
    let integer_middle = SymbolicFieldInnerLayout::new(
        "slot",
        LayoutPlanReport {
            schema_report_fingerprint: 2,
            entries: vec![layout_field_entry(
                "sub".into(),
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
    let error = reject_inner_layout_materialization(
        &layout,
        &[integer_middle],
        std::slice::from_ref(&deep_path("entry")),
        "a deeper hop requires a whole-record enclosing placement",
    );
    assert!(
        error.contains("inner layout for `slot.sub` requires a whole `At` placement"),
        "{}",
        error
    );

    // A nested carrier whose claimed interior overflows the enclosing element
    // must reject at the boundary, before any member offset is composed.
    let overflowing_interior = SymbolicFieldInnerLayout::new("slot", record_interior(&carrier))
        .with_inner_layout(SymbolicFieldInnerLayout::new(
            "sub",
            LayoutPlanReport {
                schema_report_fingerprint: 3,
                entries: vec![layout_field_entry(
                    "entry".into(),
                    LayoutPlacementReport::At { offset: 0 },
                )],
                offsets: Some(vec![0]),
                // `sub` sits at offset 8 of `slot`'s 32-byte interior; claiming 32
                // more bytes reaches 40 and escapes into the outer object.
                size: Some(32),
                align: 8,
            },
        ));
    let error = reject_inner_layout_materialization(
        &layout,
        &[overflowing_interior],
        std::slice::from_ref(&deep_path("entry")),
        "a nested interior may not exceed its enclosing element",
    );
    assert!(
        error
            .contains("interior layout for `slot.sub` exceeds the enclosing 32-byte record extent"),
        "{}",
        error
    );

    // A leaf member may not escape the innermost interior extent either.
    let mut deep_escape = record_interior(&carrier.inner_layouts()[0]);
    deep_escape.entries.push(layout_field_entry(
        "escape".into(),
        LayoutPlacementReport::At { offset: 20 },
    ));
    let escaping = SymbolicFieldInnerLayout::new("slot", record_interior(&carrier))
        .with_inner_layout(SymbolicFieldInnerLayout::new("sub", deep_escape));
    let error = reject_inner_layout_materialization(
        &layout,
        &[escaping],
        std::slice::from_ref(&deep_path("escape")),
        "a deeper leaf may not escape its interior record extent",
    );
    assert!(
        error.contains("writes outside the 24-byte materialization"),
        "{}",
        error
    );
}
