use super::{
    data, deeply_nested_layout, entry, nested_layout, post_handoff_context, record_interior,
    recursive_record_array_report, recursive_sum_report, sum_array_layout, sum_field_layout,
    sum_layout,
};
use crate::{
    CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT, ConventionalRecordSumChildHop,
    ConventionalRecordSumChildInterior, ConventionalRecordSumChildLayoutReport,
    ConventionalRecursiveRecordSumPathsLayoutReport, ConventionalSumLayoutReport,
    LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport, MaterializationAction,
    SymbolicFieldInnerLayout, SymbolicFieldPathSegment, SymbolicFieldValue,
    derive_symbolic_materialization, derive_symbolic_materialization_with_inner_layouts,
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

    // A zero-count row is not permission to skip validating its evidence.
    for mutation in 0..4 {
        let mut malformed = layout.clone();
        match mutation {
            0 => malformed.entries[1].placement = LayoutPlacementReport::At { offset: 9 },
            1 => malformed.entries.push(malformed.entries[1].clone()),
            2 => {
                malformed.entries[1].placement = LayoutPlacementReport::IntegerAt {
                    offset: 8,
                    stored_width: 8,
                    interpretation: crate::IntegerInterpretation::Unsigned,
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
    assert!(derive(&layout, &[carrier.clone(), carrier.clone()]).is_err());
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

    let interior = LayoutPlanReport {
        schema_report_fingerprint: 2,
        entries: vec![LayoutFieldEntryReport {
            field: "choice".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset: 0 },
        }],
        offsets: Some(vec![0]),
        size: Some(24),
        align: 8,
    };
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

    // The shared 64-segment record path bound is a compiler resource limit,
    // not a language limit. A 65-segment path rejects before any field
    // membership or carrier check; a 64-segment path still resolves normally
    // past the bound check — here it reaches the third hop and fails ordinary
    // interior membership.
    let mut chain = SymbolicFieldPathSegment::new("leaf");
    for _ in 1..CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT - 1 {
        chain = SymbolicFieldPathSegment::new("sub").with_inner_segment(chain);
    }
    let at_bound = SymbolicFieldValue::new("slot", 64, entry())
        .expect("outer record field")
        .with_inner_segment(chain);
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&at_bound),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("the second `sub` hop is not a member of `sub`'s interior");
    assert!(
        error.0.contains("has no entry in the inner layout plan"),
        "{}",
        error.0
    );

    let mut chain = SymbolicFieldPathSegment::new("leaf");
    for _ in 1..CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT {
        chain = SymbolicFieldPathSegment::new("sub").with_inner_segment(chain);
    }
    let over_bound = SymbolicFieldValue::new("slot", 64, entry())
        .expect("outer record field")
        .with_inner_segment(chain);
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&over_bound),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a path past the record depth bound must reject");
    assert!(
        error
            .0
            .contains("exceeds the compiler's 64-segment record path bound"),
        "{}",
        error.0
    );

    // The carrier tree obeys the same bound: carriers nested past it cannot be
    // traversed by any admitted path, so they reject during preparation.
    let self_similar = || {
        SymbolicFieldInnerLayout::new(
            "slot",
            LayoutPlanReport {
                schema_report_fingerprint: 2,
                entries: vec![LayoutFieldEntryReport {
                    field: "slot".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 0 },
                }],
                offsets: Some(vec![0]),
                size: Some(8),
                align: 8,
            },
        )
    };
    let mut tree = self_similar();
    for _ in 1..CONVENTIONAL_RECORD_PATH_DEPTH_LIMIT + 1 {
        tree = self_similar().with_inner_layout(tree);
    }
    let symbolic = SymbolicFieldValue::new("slot", 64, entry())
        .expect("outer record field")
        .with_inner_segment(SymbolicFieldPathSegment::new("slot"));
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        &[tree],
        std::slice::from_ref(&symbolic),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a carrier tree deeper than the bound must reject");
    assert!(
        error
            .0
            .contains("nests beyond the compiler's 64-segment record path bound"),
        "{}",
        error.0
    );
}

#[test]
fn symbolic_sum_materialization_joins_case_and_payload_by_identity() {
    // Numbered sum schemas join case and payload hops on stable member
    // identities; every spelled name remains diagnostic presentation, so the
    // path may spell names the schema later renamed.
    let numbered_layout = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![
            LayoutFieldEntryReport {
                field: "header".into(),
                member_identity: Some(4),
                placement: LayoutPlacementReport::At { offset: 0 },
            },
            LayoutFieldEntryReport {
                field: "choice".into(),
                member_identity: Some(5),
                placement: LayoutPlacementReport::At { offset: 8 },
            },
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
            SymbolicFieldPathSegment::new_numbered("RenamedCase", 11)
                .with_inner_segment(SymbolicFieldPathSegment::new_numbered("RenamedPayload", 21)),
        );
    let plan = derive_symbolic_materialization_with_inner_layouts(
        &numbered_layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&symbolic),
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

    // A valid case spelled without its payload hop cannot resolve a leaf.
    let caseless = SymbolicFieldValue::new("choice", 64, entry())
        .expect("direct sum field")
        .with_inner_segment(SymbolicFieldPathSegment::new("Run"));
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&caseless),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a case hop that ends the path cannot resolve a payload");
    assert!(
        error
            .0
            .contains("requires a payload field below case `choice.Run`"),
        "{}",
        error.0
    );

    // A spelled case must exist in the bound interior.
    let missing_case = SymbolicFieldValue::new("choice", 64, entry())
        .expect("direct sum field")
        .with_inner_segment(
            SymbolicFieldPathSegment::new("Bogus")
                .with_inner_segment(SymbolicFieldPathSegment::new("callback")),
        );
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&missing_case),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a spelled case the sum interior lacks must reject");
    assert!(
        error.0.contains(
            "spells no case or common field `Bogus` of the inner sum layout for `choice`"
        ),
        "{}",
        error.0
    );

    // A spelled payload must be a member of the selected case, not merely of
    // the sum: `flags` only exists on `Small`, so `choice.Run.flags` rejects.
    let wrong_case_payload = SymbolicFieldValue::new("choice", 64, entry())
        .expect("direct sum field")
        .with_inner_segment(
            SymbolicFieldPathSegment::new("Run")
                .with_inner_segment(SymbolicFieldPathSegment::new("flags")),
        );
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&wrong_case_payload),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a payload spelled on the wrong case must reject");
    assert!(
        error
            .0
            .contains("spells no payload field `flags` of case `choice.Run`"),
        "{}",
        error.0
    );

    // A case payload is always the leaf of a sum path: no hop may continue
    // below it.
    let continues = SymbolicFieldValue::new("choice", 64, entry())
        .expect("direct sum field")
        .with_inner_segment(
            SymbolicFieldPathSegment::new("Run").with_inner_segment(
                SymbolicFieldPathSegment::new("callback")
                    .with_inner_segment(SymbolicFieldPathSegment::new("deeper")),
            ),
        );
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&continues),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("no interior exists below a case payload");
    assert!(
        error.0.contains(
            "continues below sum payload `choice.Run.callback`; a case payload is the leaf of a sum path"
        ),
        "{}",
        error.0
    );

    // Neither the case hop nor the payload hop carries an element index: case
    // geometry is fixed, and a payload retains one extent per field.
    let indexed_case = SymbolicFieldValue::new("choice", 64, entry())
        .expect("direct sum field")
        .with_inner_segment(
            SymbolicFieldPathSegment::new_indexed("Run", 0)
                .with_inner_segment(SymbolicFieldPathSegment::new("callback")),
        );
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&indexed_case),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a case hop cannot carry an element index");
    assert!(
        error
            .0
            .contains("case `choice.Run` cannot carry an element index"),
        "{}",
        error.0
    );

    let indexed_payload = SymbolicFieldValue::new("choice", 64, entry())
        .expect("direct sum field")
        .with_inner_segment(
            SymbolicFieldPathSegment::new("Run")
                .with_inner_segment(SymbolicFieldPathSegment::new_indexed("callback", 0)),
        );
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&indexed_payload),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a payload hop cannot carry an element index");
    assert!(
        error.0.contains(
            "payload `choice.Run.callback` cannot carry an element index; a sum payload retains one extent per field"
        ),
        "{}",
        error.0
    );

    // The write slot is the payload field's own extent: a 64-bit symbolic
    // cannot address `Small.flags`'s single byte.
    let oversized = SymbolicFieldValue::new("choice", 64, entry())
        .expect("direct sum field")
        .with_inner_segment(
            SymbolicFieldPathSegment::new("Small")
                .with_inner_segment(SymbolicFieldPathSegment::new("flags")),
        );
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&oversized),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a write wider than its payload slot must reject");
    assert!(
        error
            .0
            .contains("width 64 exceeds the 8-bit sum member `choice.Small.flags`"),
        "{}",
        error.0
    );
}

#[test]
fn symbolic_sum_materialization_rejects_malformed_sum_carriers() {
    let (layout, carrier) = sum_field_layout();

    // A sum path still needs its carrier, and a supplied carrier a symbolic
    // path never crosses still rejects as stale.
    let path = SymbolicFieldValue::new("choice", 64, entry())
        .expect("direct sum field")
        .with_inner_segment(
            SymbolicFieldPathSegment::new("Run")
                .with_inner_segment(SymbolicFieldPathSegment::new("callback")),
        );
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
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&flat),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("an untraversed sum carrier must reject");
    assert!(
        error
            .0
            .contains("no symbolic field path traverses the supplied inner layout for `choice`"),
        "{}",
        error.0
    );

    // A sum interior carries no field namespace below it, so it cannot hold
    // nested carriers.
    let nested_under_sum =
        SymbolicFieldInnerLayout::new_sum("choice", sum_layout()).with_inner_layout(
            SymbolicFieldInnerLayout::new("callback", record_interior(&nested_layout().1)),
        );
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        &[nested_under_sum],
        std::slice::from_ref(&path),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a sum interior cannot bind nested carriers");
    assert!(
        error.0.contains(
            "inner layout for `choice` binds a sum interior; its case payload fields carry no nested record carriers"
        ),
        "{}",
        error.0
    );

    // A carrier whose payload geometry escapes its own claimed extent rejects
    // during preparation, before any offset composes.
    let mut escaping_layout = sum_layout();
    escaping_layout.cases[1].payload_fields[1].offset = 20;
    let escaping = SymbolicFieldInnerLayout::new_sum("choice", escaping_layout);
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        &[escaping],
        std::slice::from_ref(&path),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a payload outside the sum extent must reject");
    assert!(
        error.0.contains(
            "inner layout for `choice` places payload field `clock` of case `Run` outside the sum's 24-byte extent"
        ),
        "{}",
        error.0
    );

    // A claimed interior too large for its enclosing element rejects at the
    // boundary, before any member offset composes.
    let oversized = SymbolicFieldInnerLayout::new_sum(
        "choice",
        ConventionalSumLayoutReport {
            size: 40,
            ..sum_layout()
        },
    );
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        &[oversized],
        std::slice::from_ref(&path),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a sum interior may not exceed its enclosing element");
    assert!(
        error
            .0
            .contains("interior layout for `choice` exceeds the enclosing 40-byte record extent"),
        "{}",
        error.0
    );

    // Empty interiors have no addressable elements. Every retained stride
    // still covers the complete element extent.
    let (array_layout, _) = sum_array_layout();
    let zero_count = SymbolicFieldInnerLayout::new_sum_array("sums", sum_layout(), 0, 24);
    let indexed_path = SymbolicFieldValue::new_indexed("sums", 0, 64, entry())
        .expect("repeated sum field")
        .with_inner_segment(
            SymbolicFieldPathSegment::new("Run")
                .with_inner_segment(SymbolicFieldPathSegment::new("callback")),
        );
    let error = derive_symbolic_materialization_with_inner_layouts(
        &array_layout,
        &[zero_count],
        std::slice::from_ref(&indexed_path),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("indexing a zero-element repeated sum interior must reject");
    assert!(
        error
            .0
            .contains("element index 0 is outside its 0 element placements"),
        "{}",
        error.0
    );

    let overlapping = SymbolicFieldInnerLayout::new_sum_array("sums", sum_layout(), 2, 16);
    let error = derive_symbolic_materialization_with_inner_layouts(
        &array_layout,
        &[overlapping],
        std::slice::from_ref(&indexed_path),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a stride smaller than the element extent would overlap elements");
    assert!(
        error.0.contains(
            "inner layout for `sums` strides repeated sum elements by 16 bytes inside their 24-byte extent"
        ),
        "{}",
        error.0
    );
}

#[test]
fn symbolic_sum_array_materialization_rejects_malformed_element_hops() {
    let (layout, carrier) = sum_array_layout();

    // The whole-extent `At` placement keeps no per-element entries, so the
    // field hop must carry the index that composes the element's stride
    // offset.
    let unindexed = SymbolicFieldValue::new("sums", 64, entry())
        .expect("repeated sum field")
        .with_inner_segment(
            SymbolicFieldPathSegment::new("Run")
                .with_inner_segment(SymbolicFieldPathSegment::new("callback")),
        );
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&unindexed),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a repeated sum path requires an element index");
    assert!(
        error
            .0
            .contains("requires an element index into the repeated sum field `sums`"),
        "{}",
        error.0
    );

    // The exact index bound is the carrier's element count, checked before
    // any offset composes.
    let out_of_range = SymbolicFieldValue::new_indexed("sums", 2, 64, entry())
        .expect("repeated sum field")
        .with_inner_segment(
            SymbolicFieldPathSegment::new("Run")
                .with_inner_segment(SymbolicFieldPathSegment::new("callback")),
        );
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&out_of_range),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("an element index beyond the repeated extent must reject");
    assert!(
        error
            .0
            .contains("element index 2 is outside its 2 element placements"),
        "{}",
        error.0
    );

    // A repeated-sum carrier also admits the field's per-element `At`
    // placements — the other repeated vocabulary a validated plan produces.
    // Sorted by offset they replay the carrier's exact count and constant
    // stride, so `sums[1]` composes the same element base the whole-extent
    // placement spells.
    let mut per_element = layout.clone();
    per_element.entries = (0..2)
        .map(|index| LayoutFieldEntryReport {
            field: "sums".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At {
                offset: 8 + index * 24,
            },
        })
        .collect();
    let indexed = SymbolicFieldValue::new_indexed("sums", 1, 64, entry())
        .expect("repeated sum field")
        .with_inner_segment(
            SymbolicFieldPathSegment::new("Run")
                .with_inner_segment(SymbolicFieldPathSegment::new("callback")),
        );
    let materialization = derive_symbolic_materialization_with_inner_layouts(
        &per_element,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&indexed),
        post_handoff_context(),
        |_| None,
    )
    .expect("per-element `At` placements carry the same repeated boundary");
    let offsets = materialization
        .actions
        .iter()
        .map(|action| match action {
            MaterializationAction::RuntimeWriter(write) => {
                (write.field.as_str(), write.container_byte_offset)
            }
            other => panic!("an unresolved symbolic derives a runtime writer, found {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(offsets, [("sums[1].Run.callback", 40)]);

    // An extra placement entry is not extra coverage: three `At` entries for
    // a two-element carrier is drift, so the boundary rejects before any
    // element offset composes.
    let mut surplus = layout.clone();
    surplus.entries = (0..3)
        .map(|index| LayoutFieldEntryReport {
            field: "sums".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At {
                offset: 8 + index * 16,
            },
        })
        .collect();
    let error = derive_symbolic_materialization_with_inner_layouts(
        &surplus,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&indexed),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("more element placements than the carrier's count is drift");
    assert!(
        error
            .0
            .contains("inner layout for `sums` element placements drift from its count or stride"),
        "{}",
        error.0
    );

    // Two placements at a stride the carrier does not claim is the same
    // drift: one of the two pieces of evidence is stale.
    let mut drifted = layout.clone();
    drifted.entries = vec![
        LayoutFieldEntryReport {
            field: "sums".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset: 8 },
        },
        LayoutFieldEntryReport {
            field: "sums".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset: 30 },
        },
    ];
    let error = derive_symbolic_materialization_with_inner_layouts(
        &drifted,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&indexed),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("element placements drifting from the carrier's stride must reject");
    assert!(
        error
            .0
            .contains("inner layout for `sums` element placements drift from its count or stride"),
        "{}",
        error.0
    );

    // The carrier's claimed array extent is whole-field evidence: two 24-byte
    // elements at stride 24 starting at offset 8 claim bytes 8..56, which
    // cannot fit a record that only spans 40 — a stale carrier rejects before
    // serving an element offset.
    let shrunken_outer = LayoutPlanReport {
        size: Some(40),
        ..layout.clone()
    };
    let error = derive_symbolic_materialization_with_inner_layouts(
        &shrunken_outer,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&indexed),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a repeated interior escaping the record extent must reject");
    assert!(
        error
            .0
            .contains("interior layout for `sums` exceeds the enclosing 40-byte record extent"),
        "{}",
        error.0
    );
}

#[test]
fn symbolic_recursive_sum_materialization_joins_boundaries_by_identity() {
    // A numbered schema joins every folded carrier on stable member identity,
    // so the spelled path may use names the schema later renamed — including
    // the selected case and its payload field inside the sum interior.
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
    // Children follow the authored member order: `inner` beside `route`.
    let [inner, route] = mid.children.as_mut_slice() else {
        unreachable!("the middle level spells two children")
    };
    inner.member_identity = Some(6);
    // The level's own direct sum joins the same numbered namespace as the
    // deeper record path.
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

    let symbolic = [
        SymbolicFieldValue::new_numbered("middle", 5, 64, entry())
            .expect("numbered outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_numbered("inner", 6).with_inner_segment(
                    SymbolicFieldPathSegment::new_numbered("choice", 8).with_inner_segment(
                        SymbolicFieldPathSegment::new_numbered("RenamedCase", 11)
                            .with_inner_segment(SymbolicFieldPathSegment::new_numbered(
                                "RenamedPayload",
                                21,
                            )),
                    ),
                ),
            ),
        SymbolicFieldValue::new_numbered("middle", 5, 64, data())
            .expect("numbered outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_numbered("route", 10).with_inner_segment(
                    SymbolicFieldPathSegment::new_numbered("RenamedCase", 13).with_inner_segment(
                        SymbolicFieldPathSegment::new_numbered("RenamedPayload", 23),
                    ),
                ),
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
    let writes = plan
        .actions
        .iter()
        .map(|action| match action {
            MaterializationAction::RuntimeWriter(write) => {
                (write.field.as_str(), write.container_byte_offset)
            }
            _ => panic!("an unresolved symbolic derives a runtime writer"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        writes,
        vec![
            ("middle.inner.choice.RenamedCase.RenamedPayload", 16),
            ("middle.route.RenamedCase.RenamedPayload", 64),
        ]
    );
}

#[test]
fn symbolic_recursive_sum_fold_bounds_report_depth() {
    // The fold walks the report's own recursion under the same
    // 64-segment resource bound carrier preparation enforces: a report
    // nesting past the deepest admissible carrier rejects during the fold
    // rather than overflowing it. A report at the boundary still folds.
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
    let wrap = |inner| ConventionalRecursiveRecordSumPathsLayoutReport {
        outer_layout: LayoutPlanReport {
            schema_report_fingerprint: 9,
            entries: vec![LayoutFieldEntryReport {
                field: "sub".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            }],
            offsets: Some(vec![0]),
            size: Some(8),
            align: 8,
        },
        children: vec![ConventionalRecordSumChildLayoutReport {
            field: "sub".into(),
            member_identity: None,
            hop: ConventionalRecordSumChildHop::Field,
            interior: ConventionalRecordSumChildInterior::Record(inner),
        }],
    };
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
    // The same record-array carrier set bounds every indexed hop: a missing
    // index, an out-of-range index, an interior escaping the enclosing
    // extent, or an unbound member all reject before any write offset is
    // assigned, and a carrier no path traverses rejects at the end.
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
                segment.with_inner_segment(
                    SymbolicFieldPathSegment::new("choice").with_inner_segment(
                        SymbolicFieldPathSegment::new("Run")
                            .with_inner_segment(SymbolicFieldPathSegment::new("callback")),
                    ),
                ),
            )
    };
    // The record hop still needs its own carriers traversed: `inner` and
    // `route` ride beside `neighbors`, so covering paths cross them too.
    let cover_rest = || {
        [
            SymbolicFieldValue::new("middle", 64, data())
                .expect("outer record field")
                .with_inner_segment(
                    SymbolicFieldPathSegment::new("inner").with_inner_segment(
                        SymbolicFieldPathSegment::new("choice").with_inner_segment(
                            SymbolicFieldPathSegment::new("Run")
                                .with_inner_segment(SymbolicFieldPathSegment::new("clock")),
                        ),
                    ),
                ),
            SymbolicFieldValue::new("middle", 64, data())
                .expect("outer record field")
                .with_inner_segment(
                    SymbolicFieldPathSegment::new("route").with_inner_segment(
                        SymbolicFieldPathSegment::new("Run")
                            .with_inner_segment(SymbolicFieldPathSegment::new("callback")),
                    ),
                ),
        ]
    };

    let error = derive_symbolic_materialization_with_inner_layouts(
        &report.outer_layout,
        &carriers,
        &std::iter::once(neighbors_choice(None))
            .chain(cover_rest())
            .collect::<Vec<_>>(),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("an unindexed hop into a repeated record interior cannot name one element");
    assert!(
        error.0.contains(
            "requires an element index into the repeated record field `middle.neighbors`"
        ),
        "{}",
        error.0
    );

    let error = derive_symbolic_materialization_with_inner_layouts(
        &report.outer_layout,
        &carriers,
        &std::iter::once(neighbors_choice(Some(2)))
            .chain(cover_rest())
            .collect::<Vec<_>>(),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("an element index beyond the repeated extent must reject");
    assert!(
        error
            .0
            .contains("element index 2 is outside its 2 element placements"),
        "{}",
        error.0
    );

    // A carrier striding repeated record elements inside their own extent
    // rejects at preparation, before any path resolves a write offset.
    let mut shrunken = carriers.clone();
    let crate::ConventionalRecordSumChildHop::Index { element_stride, .. } =
        &mut shrunken[0].inner_layouts[2].inner_layout.hop
    else {
        unreachable!()
    };
    *element_stride = 16;
    let error = derive_symbolic_materialization_with_inner_layouts(
        &report.outer_layout,
        &shrunken,
        &std::iter::once(neighbors_choice(Some(1)))
            .chain(cover_rest())
            .collect::<Vec<_>>(),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a record array striding inside its element extent must reject");
    assert!(
        error
            .0
            .contains("strides repeated record elements by 16 bytes inside their 32-byte extent"),
        "{}",
        error.0
    );

    // And a repeated record interior no symbolic path traverses still
    // rejects: the carrier must not outlive the semantic path it describes.
    let error = derive_symbolic_materialization_with_inner_layouts(
        &report.outer_layout,
        &carriers,
        &cover_rest().into_iter().collect::<Vec<_>>(),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("an untraversed repeated record interior must reject");
    assert!(
        error.0.contains(
            "no symbolic field path traverses the supplied inner layout for `middle.neighbors`"
        ),
        "{}",
        error.0
    );
}

#[test]
fn symbolic_record_array_materialization_crosses_per_element_placements() {
    // The second repeated vocabulary at a record-array boundary: one `At`
    // entry per element rather than one whole-extent `At`. `members` repeats
    // the shared record interior twice at a 24-byte stride from offset 8, so
    // `members[1].entry` composes the same destination the whole-extent
    // spelling would.
    let (_, interior_carrier) = nested_layout();
    let element = record_interior(&interior_carrier);
    let outer = LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![
            LayoutFieldEntryReport {
                field: "header".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            },
            LayoutFieldEntryReport {
                field: "members".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 8 },
            },
            LayoutFieldEntryReport {
                field: "members".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 32 },
            },
        ],
        offsets: None,
        size: Some(56),
        align: 8,
    };
    let carrier = SymbolicFieldInnerLayout::new_record_array("members", element, 2, 24);
    let indexed = SymbolicFieldValue::new_indexed("members", 1, 64, entry())
        .expect("repeated record field")
        .with_inner_segment(SymbolicFieldPathSegment::new("entry"));
    let plan = derive_symbolic_materialization_with_inner_layouts(
        &outer,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&indexed),
        post_handoff_context(),
        |_| None,
    )
    .expect("per-element `At` placements carry the repeated record boundary");

    let writes = plan
        .actions
        .iter()
        .map(|action| match action {
            MaterializationAction::RuntimeWriter(write) => {
                (write.field.as_str(), write.container_byte_offset)
            }
            other => panic!("an unresolved symbolic derives a runtime writer, found {other:?}"),
        })
        .collect::<Vec<_>>();
    // Element 1 begins at 8 + 24 = 32 and its `entry` member sits at the
    // element's offset 0.
    assert_eq!(writes, [("members[1].entry", 32)]);

    // Unindexed hops still cannot name one element, and per-element entries
    // keep the exact index bound from the carrier's count.
    let unindexed = SymbolicFieldValue::new("members", 64, entry())
        .expect("repeated record field")
        .with_inner_segment(SymbolicFieldPathSegment::new("entry"));
    let error = derive_symbolic_materialization_with_inner_layouts(
        &outer,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&unindexed),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("a repeated record path requires an element index");
    assert!(
        error
            .0
            .contains("requires an element index into the repeated record field `members`"),
        "{}",
        error.0
    );

    let out_of_range = SymbolicFieldValue::new_indexed("members", 2, 64, entry())
        .expect("repeated record field")
        .with_inner_segment(SymbolicFieldPathSegment::new("entry"));
    let error = derive_symbolic_materialization_with_inner_layouts(
        &outer,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&out_of_range),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("an element index beyond the repeated extent must reject");
    assert!(
        error
            .0
            .contains("element index 2 is outside its 2 element placements"),
        "{}",
        error.0
    );
}
