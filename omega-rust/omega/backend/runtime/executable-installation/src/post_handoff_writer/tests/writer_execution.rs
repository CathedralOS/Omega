//! Writer-program execution controls moved with the execution owner: direct
//! destination application, atomic rejection before any write, single
//! resolution per repeated target, and end-to-end byte checks over portable
//! materialization plans. Derivation and reusable-fragment lowering controls
//! live in `post-handoff-writer-plan`; portable description controls stay in
//! `layout-plans`.

use crate::post_handoff_writer::{
    apply_post_handoff_writes_atomically, execute_post_handoff_writer_plan,
};
use layout_plans::{
    ArtifactInstallationScopeId, ByteOrder, ConsumptionInstant, ConventionalRecordSumChildHop,
    ConventionalRecordSumChildInterior, ConventionalRecordSumChildLayoutReport,
    ConventionalRecursiveRecordSumPathsLayoutReport, ConventionalSumCaseLayoutReport,
    ConventionalSumLayoutReport, ConventionalSumPayloadFieldLayoutReport, DataSymbolId,
    EntryStubId, IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport,
    LayoutPlanReport, MachineRegimeId, MaterializationAction, MaterializationContext,
    MaterializationWrite, PlacementAddressRange, PlacementConstraints, PlacementPhase,
    PlacementSite, RelocationTarget, StoredIntegerFit, SymbolicFieldInnerLayout,
    SymbolicFieldPathSegment, SymbolicFieldValue, derive_symbolic_materialization,
    derive_symbolic_materialization_with_inner_layouts,
};
use post_handoff_writer_plan::{
    PostHandoffWriterPlan, PostHandoffWriterSource, PostHandoffWriterStep,
    derive_post_handoff_writer,
};

fn entry() -> RelocationTarget {
    RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x55aa).expect("nonzero identity"),
    )
}

fn data() -> RelocationTarget {
    RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x1234).expect("nonzero identity"),
    )
}

fn post_handoff_site() -> PlacementSite {
    PlacementSite {
        base_address: 0,
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        installation_scope: None,
    }
}

fn post_handoff_context() -> MaterializationContext {
    MaterializationContext {
        consumption: ConsumptionInstant::AfterOmegaHandoff,
        byte_order: ByteOrder::LittleEndian,
        native_pointer_relocation_bits: None,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
    }
}

fn split_layout() -> LayoutPlanReport {
    LayoutPlanReport {
        schema_report_fingerprint: 1,
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

/// A repeated field materializes one `At` placement per element. Every entry
/// shares the field name and stable member identity; the element index is the
/// second hop of the field/index path, so the symbolic value selects exactly
/// one of these placements.
fn repeated_layout() -> LayoutPlanReport {
    LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: (0..4)
            .map(|index| LayoutFieldEntryReport {
                field: "handlers".into(),
                member_identity: Some(9),
                placement: LayoutPlacementReport::At { offset: index * 8 },
            })
            .collect(),
        offsets: Some(vec![0, 8, 16, 24]),
        size: Some(32),
        align: 8,
    }
}

/// A second record boundary below `slot`: the outer plan places `slot` as one
/// 32-byte `At` extent at offset 8; `slot`'s interior places `pad` at 0 and
/// `sub` at 8; `sub`'s interior places `entry`, `flags`, and `args` at 0, 8,
/// and 16. The carrier tree mirrors the record boundaries: `sub`'s interior
/// rides inside `slot`'s carrier, so `slot.sub.entry` composes to byte 16 and
/// `slot.sub.args` to byte 32 of the outer object.
fn deeply_nested_layout() -> (LayoutPlanReport, SymbolicFieldInnerLayout) {
    (
        LayoutPlanReport {
            schema_report_fingerprint: 1,
            entries: vec![
                LayoutFieldEntryReport {
                    field: "header".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 0 },
                },
                LayoutFieldEntryReport {
                    field: "slot".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 8 },
                },
            ],
            offsets: Some(vec![0, 8]),
            size: Some(40),
            align: 8,
        },
        SymbolicFieldInnerLayout::new(
            "slot",
            LayoutPlanReport {
                schema_report_fingerprint: 2,
                entries: vec![
                    LayoutFieldEntryReport {
                        field: "pad".into(),
                        member_identity: None,
                        placement: LayoutPlacementReport::At { offset: 0 },
                    },
                    LayoutFieldEntryReport {
                        field: "sub".into(),
                        member_identity: None,
                        placement: LayoutPlacementReport::At { offset: 8 },
                    },
                ],
                offsets: Some(vec![0, 8]),
                size: Some(32),
                align: 8,
            },
        )
        .with_inner_layout(SymbolicFieldInnerLayout::new(
            "sub",
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
                        placement: LayoutPlacementReport::At { offset: 8 },
                    },
                    LayoutFieldEntryReport {
                        field: "args".into(),
                        member_identity: None,
                        placement: LayoutPlacementReport::At { offset: 16 },
                    },
                ],
                offsets: Some(vec![0, 8, 16]),
                size: Some(24),
                align: 8,
            },
        )),
    )
}

/// A conventional sum interior shared by the symbolic sum tests: the tag sits
/// at 0, `Run` carries two u64 payload fields at 8 and 16, and `Small`
/// overlays one u8 payload at 8 — distinct cases deliberately share payload
/// bytes, so the path's exact case spelling is what picks the slot.
fn sum_layout() -> ConventionalSumLayoutReport {
    ConventionalSumLayoutReport {
        schema_report_fingerprint: 7,
        tag_offset: 0,
        tag_size: 4,
        tag_align: 4,
        common_fields: Vec::new(),
        cases: vec![
            ConventionalSumCaseLayoutReport {
                case: "Empty".into(),
                member_identity: None,
                ordinal: 0,
                payload_fields: Vec::new(),
            },
            ConventionalSumCaseLayoutReport {
                case: "Run".into(),
                member_identity: None,
                ordinal: 1,
                payload_fields: vec![
                    ConventionalSumPayloadFieldLayoutReport {
                        field: "callback".into(),
                        member_identity: None,
                        offset: 8,
                        size: 8,
                        align: 8,
                    },
                    ConventionalSumPayloadFieldLayoutReport {
                        field: "clock".into(),
                        member_identity: None,
                        offset: 16,
                        size: 8,
                        align: 8,
                    },
                ],
            },
            ConventionalSumCaseLayoutReport {
                case: "Small".into(),
                member_identity: None,
                ordinal: 2,
                payload_fields: vec![ConventionalSumPayloadFieldLayoutReport {
                    field: "flags".into(),
                    member_identity: None,
                    offset: 8,
                    size: 1,
                    align: 1,
                }],
            },
        ],
        size: 24,
        align: 8,
    }
}

/// The same recursive record/sum shape, but both direct sum-array kind now
/// coexist beside the other child kinds: `middle`'s interior places `inner`
/// at 0 (an 80-byte record), `tag` at 80, its own direct sum `route` at 88,
/// and its own `batches` array (two elements at a 24-byte stride) spanning
/// 112..160; `inner`'s interior places `choice` at 0, `choices` (two
/// elements at a 24-byte stride) spanning 24..72, and `pad` at 72. Each
/// level carries every child kind, so `middle.inner.choices[i].Run.<payload>`
/// resolves the same indexed boundary the standalone sum-array rung spells,
/// folded through the recursive report instead of a top-level-only carrier.
fn recursive_sum_array_report() -> ConventionalRecursiveRecordSumPathsLayoutReport {
    fn record(fingerprint: u64, fields: &[(&str, u64)], size: u64) -> LayoutPlanReport {
        LayoutPlanReport {
            schema_report_fingerprint: fingerprint,
            entries: fields
                .iter()
                .map(|&(field, offset)| LayoutFieldEntryReport {
                    field: field.into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset },
                })
                .collect(),
            offsets: Some(fields.iter().map(|&(_, offset)| offset).collect()),
            size: Some(size),
            align: 8,
        }
    }
    ConventionalRecursiveRecordSumPathsLayoutReport {
        outer_layout: record(1, &[("header", 0), ("middle", 8)], 168),
        children: vec![ConventionalRecordSumChildLayoutReport {
            field: "middle".into(),
            member_identity: None,
            hop: ConventionalRecordSumChildHop::Field,
            interior: ConventionalRecordSumChildInterior::Record(
                ConventionalRecursiveRecordSumPathsLayoutReport {
                    outer_layout: record(
                        2,
                        &[("inner", 0), ("tag", 80), ("route", 88), ("batches", 112)],
                        160,
                    ),
                    children: vec![
                        ConventionalRecordSumChildLayoutReport {
                            field: "inner".into(),
                            member_identity: None,
                            hop: ConventionalRecordSumChildHop::Field,
                            interior: ConventionalRecordSumChildInterior::Record(
                                ConventionalRecursiveRecordSumPathsLayoutReport {
                                    outer_layout: record(
                                        3,
                                        &[("choice", 0), ("choices", 24), ("pad", 72)],
                                        80,
                                    ),
                                    children: vec![
                                        ConventionalRecordSumChildLayoutReport {
                                            field: "choice".into(),
                                            member_identity: None,
                                            hop: ConventionalRecordSumChildHop::Field,
                                            interior: ConventionalRecordSumChildInterior::Sum(
                                                sum_layout(),
                                            ),
                                        },
                                        ConventionalRecordSumChildLayoutReport {
                                            field: "choices".into(),
                                            member_identity: None,
                                            hop: ConventionalRecordSumChildHop::Index {
                                                element_count: 2,
                                                element_stride: 24,
                                            },
                                            interior: ConventionalRecordSumChildInterior::Sum(
                                                sum_layout(),
                                            ),
                                        },
                                    ],
                                },
                            ),
                        },
                        ConventionalRecordSumChildLayoutReport {
                            field: "route".into(),
                            member_identity: None,
                            hop: ConventionalRecordSumChildHop::Field,
                            interior: ConventionalRecordSumChildInterior::Sum(sum_layout()),
                        },
                        ConventionalRecordSumChildLayoutReport {
                            field: "batches".into(),
                            member_identity: None,
                            hop: ConventionalRecordSumChildHop::Index {
                                element_count: 2,
                                element_stride: 24,
                            },
                            interior: ConventionalRecordSumChildInterior::Sum(sum_layout()),
                        },
                    ],
                },
            ),
        }],
    }
}

/// A recursive record/sum path report carrying a direct array of records that
/// still reach sums: `middle`'s interior places the deeper record `inner` at
/// 0 (a 32-byte leaf holding `choice` at 0 and `pad` at 24), its own direct
/// sum `route` at 32, and `neighbors` — two 32-byte elements of the same
/// leaf record at a 32-byte stride — spanning 56..120. The `neighbors` row's
/// `inner` is the element record's own `Leaf` report, retained once for
/// every index the field spells, so `middle.neighbors[1].choice.Run.<p>`
/// composes the element hop and the record boundary inside it under the same
/// bounded traversal the standalone record carrier walks.
fn recursive_record_array_report() -> ConventionalRecursiveRecordSumPathsLayoutReport {
    fn record(fingerprint: u64, fields: &[(&str, u64)], size: u64) -> LayoutPlanReport {
        LayoutPlanReport {
            schema_report_fingerprint: fingerprint,
            entries: fields
                .iter()
                .map(|&(field, offset)| LayoutFieldEntryReport {
                    field: field.into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset },
                })
                .collect(),
            offsets: Some(fields.iter().map(|&(_, offset)| offset).collect()),
            size: Some(size),
            align: 8,
        }
    }
    let element = || ConventionalRecursiveRecordSumPathsLayoutReport {
        outer_layout: record(3, &[("choice", 0), ("pad", 24)], 32),
        children: vec![ConventionalRecordSumChildLayoutReport {
            field: "choice".into(),
            member_identity: None,
            hop: ConventionalRecordSumChildHop::Field,
            interior: ConventionalRecordSumChildInterior::Sum(sum_layout()),
        }],
    };
    ConventionalRecursiveRecordSumPathsLayoutReport {
        outer_layout: record(1, &[("header", 0), ("middle", 8)], 128),
        children: vec![ConventionalRecordSumChildLayoutReport {
            field: "middle".into(),
            member_identity: None,
            hop: ConventionalRecordSumChildHop::Field,
            interior: ConventionalRecordSumChildInterior::Record(
                ConventionalRecursiveRecordSumPathsLayoutReport {
                    outer_layout: record(2, &[("inner", 0), ("route", 32), ("neighbors", 56)], 120),
                    children: vec![
                        ConventionalRecordSumChildLayoutReport {
                            field: "inner".into(),
                            member_identity: None,
                            hop: ConventionalRecordSumChildHop::Field,
                            interior: ConventionalRecordSumChildInterior::Record(element()),
                        },
                        ConventionalRecordSumChildLayoutReport {
                            field: "route".into(),
                            member_identity: None,
                            hop: ConventionalRecordSumChildHop::Field,
                            interior: ConventionalRecordSumChildInterior::Sum(sum_layout()),
                        },
                        ConventionalRecordSumChildLayoutReport {
                            field: "neighbors".into(),
                            member_identity: None,
                            hop: ConventionalRecordSumChildHop::Index {
                                element_count: 2,
                                element_stride: 32,
                            },
                            interior: ConventionalRecordSumChildInterior::Record(element()),
                        },
                    ],
                },
            ),
        }],
    }
}

/// A mixed common-field/case interior: the tag sits at 0, `sequence` packs at
/// 4, the shared payload base aligns to 8 under `Run.callback`'s u64
/// alignment, and `Empty` contributes nothing past the common floor. The
/// complete element extent is 16 bytes at 8-byte alignment.
fn mixed_sum_layout() -> ConventionalSumLayoutReport {
    ConventionalSumLayoutReport {
        schema_report_fingerprint: 9,
        tag_offset: 0,
        tag_size: 4,
        tag_align: 4,
        common_fields: vec![ConventionalSumPayloadFieldLayoutReport {
            field: "sequence".into(),
            member_identity: None,
            offset: 4,
            size: 1,
            align: 1,
        }],
        cases: vec![
            ConventionalSumCaseLayoutReport {
                case: "Run".into(),
                member_identity: None,
                ordinal: 0,
                payload_fields: vec![ConventionalSumPayloadFieldLayoutReport {
                    field: "callback".into(),
                    member_identity: None,
                    offset: 8,
                    size: 8,
                    align: 8,
                }],
            },
            ConventionalSumCaseLayoutReport {
                case: "Empty".into(),
                member_identity: None,
                ordinal: 1,
                payload_fields: Vec::new(),
            },
        ],
        size: 16,
        align: 8,
    }
}

/// A record carrying a literal array of mixed elements: `header` spans 0..8,
/// `mixed` repeats two 16-byte elements at a 16-byte stride from offset 8.
fn mixed_sum_array_layout() -> (LayoutPlanReport, SymbolicFieldInnerLayout) {
    (
        LayoutPlanReport {
            schema_report_fingerprint: 1,
            entries: vec![
                LayoutFieldEntryReport {
                    field: "header".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 0 },
                },
                LayoutFieldEntryReport {
                    field: "mixed".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 8 },
                },
            ],
            offsets: Some(vec![0, 8]),
            size: Some(40),
            align: 8,
        },
        SymbolicFieldInnerLayout::new_sum_array("mixed", mixed_sum_layout(), 2, 16),
    )
}

#[test]
fn empty_post_handoff_writer_rejects_execution() {
    let writer = PostHandoffWriterPlan {
        byte_len: 8,
        byte_order: ByteOrder::LittleEndian,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        steps: Vec::new(),
    };
    let mut bytes = [0xa5; 8];
    let mut resolutions = 0;
    let error = execute_post_handoff_writer_plan(&writer, &mut bytes, post_handoff_site(), |_| {
        resolutions += 1;
        Some(0)
    })
    .expect_err("empty direct execution cannot report writer success");
    assert!(error.0.contains("at least one fragment"), "{}", error.0);
    assert_eq!(resolutions, 0);
    assert_eq!(bytes, [0xa5; 8]);
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
    let error = execute_post_handoff_writer_plan(&writer, &mut bytes, post_handoff_site(), |_| {
        resolutions += 1;
        Some(0)
    })
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
        post_handoff_context(),
        |_| None,
    )
    .expect("writer plan");
    let mut writer =
        derive_post_handoff_writer(&plan).expect("runtime actions form a writer program");
    let substituted = RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x66bb).expect("second entry identity"),
    );
    writer.steps[0].source = PostHandoffWriterSource::Resolve(substituted);

    let mut bytes = [0xa5_u8; 16];
    let error = execute_post_handoff_writer_plan(&writer, &mut bytes, post_handoff_site(), |_| {
        panic!("mismatched writer target must reject before resolution")
    })
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
    let error = execute_post_handoff_writer_plan(&writer, &mut bytes, post_handoff_site(), |_| {
        Some(0x1122_3344_5566_7788)
    })
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
fn writer_execution_replays_stored_integer_fit_before_writes() {
    let layout = LayoutPlanReport {
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
    let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
    let plan = derive_symbolic_materialization(
        &layout,
        std::slice::from_ref(&symbolic),
        post_handoff_context(),
        |_| None,
    )
    .expect("a post-handoff resolver can discharge stored-integer fit");
    let writer =
        derive_post_handoff_writer(&plan).expect("stored-integer runtime action derives a writer");

    let mut bytes = [0xa5_u8; 4];
    execute_post_handoff_writer_plan(&writer, &mut bytes, post_handoff_site(), |_| {
        Some(0x1234_5678)
    })
    .expect("resolved post-handoff value fits stored width");
    assert_eq!(bytes, [0x78, 0x56, 0x34, 0x12]);

    bytes.fill(0xa5);
    let error = execute_post_handoff_writer_plan(&writer, &mut bytes, post_handoff_site(), |_| {
        Some(1_u64 << 32)
    })
    .expect_err("post-handoff resolution must reject an out-of-range value");
    assert!(error.0.contains("does not fit"), "{}", error.0);
    assert_eq!(bytes, [0xa5; 4], "fit rejection must precede every write");
}

#[test]
fn unresolved_writer_targets_leave_the_destination_unchanged() {
    let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
    let plan = derive_symbolic_materialization(
        &split_layout(),
        &[symbolic],
        post_handoff_context(),
        |_| None,
    )
    .expect("writer plan");
    let writer = derive_post_handoff_writer(&plan).expect("runtime actions form a writer program");
    let mut missing = [0xa5_u8; 16];
    assert!(
        execute_post_handoff_writer_plan(&writer, &mut missing, post_handoff_site(), |_| None,)
            .is_err()
    );
    assert_eq!(missing, [0xa5; 16]);
}

#[test]
fn writer_execution_validates_the_placement_site_before_writes() {
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
    let writer = derive_post_handoff_writer(&plan).expect("runtime actions form a writer program");

    let wrong_phase = PlacementSite {
        base_address: 0x8000,
        phase: PlacementPhase::Load,
        machine_regime: Some(regime),
        installation_scope: Some(scope),
    };
    let mut unchanged = [0xa5_u8; 16];
    let mut resolutions = 0;
    assert!(
        execute_post_handoff_writer_plan(&writer, &mut unchanged, wrong_phase, |_| {
            resolutions += 1;
            Some(1)
        })
        .is_err()
    );
    assert_eq!(resolutions, 0);
    assert_eq!(unchanged, [0xa5; 16]);
}

#[test]
fn symbolic_index_materialization_assigns_the_exact_element() {
    let symbolic = SymbolicFieldValue::new_indexed_numbered("handlers", 9, 2, 64, entry())
        .expect("indexed field");
    let plan = derive_symbolic_materialization(
        &repeated_layout(),
        &[symbolic],
        post_handoff_context(),
        |_| None,
    )
    .expect("an indexed field/index path derives one element write");

    assert_eq!(plan.actions.len(), 1);
    let MaterializationAction::RuntimeWriter(write) = &plan.actions[0] else {
        panic!("an unresolved indexed symbolic derives a runtime writer");
    };
    assert_eq!(write.container_byte_offset, 16);

    let writer = derive_post_handoff_writer(&plan).expect("writer");
    let mut bytes = [0xa5_u8; 32];
    execute_post_handoff_writer_plan(&writer, &mut bytes, post_handoff_site(), |target| {
        assert_eq!(target, entry());
        Some(0x1122_3344_5566_7788)
    })
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
    let writer = derive_post_handoff_writer(&plan).expect("writer");
    let mut bytes = [0xa5_u8; 40];
    execute_post_handoff_writer_plan(&writer, &mut bytes, post_handoff_site(), |target| {
        Some(match target {
            RelocationTarget::Entry(_) => {
                entry_resolutions += 1;
                0x1122_3344_5566_7788
            }
            RelocationTarget::Data(_) => 0xdead_beef,
        })
    })
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
fn symbolic_mixed_sum_array_materialization_assigns_common_and_case_members() {
    let (layout, carrier) = mixed_sum_array_layout();
    // `mixed[i].common` spells a mixed shape's common field directly — one
    // hop below the element boundary — while `mixed[i].Run.callback` keeps
    // the two-hop case/payload spelling inside the shared overlay.
    let symbolic = [
        SymbolicFieldValue::new_indexed("mixed", 1, 8, data())
            .expect("repeated mixed field")
            .with_inner_segment(SymbolicFieldPathSegment::new("sequence")),
        SymbolicFieldValue::new_indexed("mixed", 0, 64, entry())
            .expect("repeated mixed field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new("Run")
                    .with_inner_segment(SymbolicFieldPathSegment::new("callback")),
            ),
    ];
    let plan = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        &symbolic,
        post_handoff_context(),
        |_| None,
    )
    .expect("a repeated mixed interior derives common-field and case payload writes");

    let writes = plan
        .actions
        .iter()
        .map(|action| match action {
            MaterializationAction::RuntimeWriter(write) => (
                write.field.as_str(),
                write.container_byte_offset,
                write.width,
            ),
            _ => panic!("an unresolved symbolic derives a runtime writer"),
        })
        .collect::<Vec<_>>();
    // `mixed` spans 8..40 at a 16-byte stride: element 1's `sequence` lands at
    // 8 + 16 + 4 = 28, element 0's `Run.callback` at 8 + 0 + 8 = 16.
    assert_eq!(
        writes,
        vec![
            ("mixed[1].sequence", 28, 8),
            ("mixed[0].Run.callback", 16, 64)
        ]
    );

    let writer = derive_post_handoff_writer(&plan).expect("writer");
    let mut bytes = [0xa5_u8; 40];
    execute_post_handoff_writer_plan(&writer, &mut bytes, post_handoff_site(), |target| {
        if target == entry() {
            Some(0x1122_3344_5566_7788)
        } else {
            assert_eq!(target, data());
            Some(0x7f)
        }
    })
    .expect("the mixed writer resolves each element member slot");

    assert_eq!(&bytes[16..24], &0x1122_3344_5566_7788_u64.to_le_bytes());
    assert_eq!(bytes[28], 0x7f);
    assert!(
        bytes[0..16]
            .iter()
            .chain(&bytes[24..28])
            .chain(&bytes[29..])
            .all(|byte| *byte == 0xa5),
        "the mixed writer only realizes the addressed common and payload slots"
    );

    // A common-field spelling that continues below the member rejects: a
    // common field is the leaf of a mixed sum path, same as a case payload.
    let continues = SymbolicFieldValue::new_indexed("mixed", 0, 8, data())
        .expect("repeated mixed field")
        .with_inner_segment(
            SymbolicFieldPathSegment::new("sequence")
                .with_inner_segment(SymbolicFieldPathSegment::new("deeper")),
        );
    let error = derive_symbolic_materialization_with_inner_layouts(
        &layout,
        std::slice::from_ref(&carrier),
        std::slice::from_ref(&continues),
        post_handoff_context(),
        |_| None,
    )
    .expect_err("no interior exists below a common field");
    assert!(
        error.0.contains(
            "continues below sum common field `mixed[0].sequence`; a common field is the leaf of a sum path"
        ),
        "{}",
        error.0
    );
}

#[test]
fn symbolic_recursive_sum_array_materialization_composes_indexed_boundaries() {
    // Nested sum arrays under the general recursive rule: the `middle` branch
    // level carries its own `route` sum and `batches` sum array beside the
    // deeper `inner` record path, and the `inner` leaf level carries `choice`
    // beside `choices`. Every folded carrier joins the same field-keyed
    // namespace, so `middle.inner.choices[i].Run.<payload>` composes
    // `field At + index * element stride + payload offset` two record
    // boundaries down exactly as the standalone rung spells it at the top.
    let report = recursive_sum_array_report();
    let carriers = SymbolicFieldInnerLayout::from_recursive_sum_paths(&report)
        .expect("the recursive sum-array report folds into inner layout carriers");
    assert_eq!(carriers.len(), 1);
    let middle = &carriers[0];
    assert_eq!(middle.field, "middle");
    // One channel in authored member order: `inner`, `route`, `batches`.
    let inner_carriers = middle.inner_layouts();
    assert_eq!(inner_carriers.len(), 3);
    assert_eq!(inner_carriers[0].field, "inner");
    assert_eq!(inner_carriers[1].field, "route");
    assert_eq!(inner_carriers[2].field, "batches");
    let leaf_carriers = inner_carriers[0].inner_layouts();
    assert_eq!(leaf_carriers.len(), 2);
    assert_eq!(leaf_carriers[0].field, "choice");
    assert_eq!(leaf_carriers[1].field, "choices");

    let symbolic = [
        SymbolicFieldValue::new("header", 64, data()).expect("scalar field"),
        SymbolicFieldValue::new("middle", 64, data())
            .expect("outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new("inner").with_inner_segment(
                    SymbolicFieldPathSegment::new("choice").with_inner_segment(
                        SymbolicFieldPathSegment::new("Run")
                            .with_inner_segment(SymbolicFieldPathSegment::new("callback")),
                    ),
                ),
            ),
        SymbolicFieldValue::new("middle", 64, entry())
            .expect("outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new("inner").with_inner_segment(
                    SymbolicFieldPathSegment::new_indexed("choices", 0).with_inner_segment(
                        SymbolicFieldPathSegment::new("Run")
                            .with_inner_segment(SymbolicFieldPathSegment::new("callback")),
                    ),
                ),
            ),
        SymbolicFieldValue::new("middle", 64, data())
            .expect("outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new("inner").with_inner_segment(
                    SymbolicFieldPathSegment::new_indexed("choices", 1).with_inner_segment(
                        SymbolicFieldPathSegment::new("Run")
                            .with_inner_segment(SymbolicFieldPathSegment::new("clock")),
                    ),
                ),
            ),
        SymbolicFieldValue::new("middle", 64, data())
            .expect("outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new("inner")
                    .with_inner_segment(SymbolicFieldPathSegment::new("pad")),
            ),
        SymbolicFieldValue::new("middle", 64, data())
            .expect("outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new("route").with_inner_segment(
                    SymbolicFieldPathSegment::new("Run")
                        .with_inner_segment(SymbolicFieldPathSegment::new("callback")),
                ),
            ),
        SymbolicFieldValue::new("middle", 64, entry())
            .expect("outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_indexed("batches", 0).with_inner_segment(
                    SymbolicFieldPathSegment::new("Run")
                        .with_inner_segment(SymbolicFieldPathSegment::new("clock")),
                ),
            ),
        SymbolicFieldValue::new("middle", 64, data())
            .expect("outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_indexed("batches", 1).with_inner_segment(
                    SymbolicFieldPathSegment::new("Run")
                        .with_inner_segment(SymbolicFieldPathSegment::new("callback")),
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
    .expect("recursive record/sum-array paths compose every crossed boundary");

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
    // `middle` spans 8..168; inside it `inner` sits at 0 holding `choice` at
    // 0, `choices` at 24 (24-byte stride), and `pad` at 72 — while the level's
    // own `route` sum sits at 88 and `batches` repeats at 112.
    assert_eq!(
        writes,
        vec![
            ("header", 0),
            ("middle.inner.choice.Run.callback", 16),
            ("middle.inner.choices[0].Run.callback", 40),
            ("middle.inner.choices[1].Run.clock", 72),
            ("middle.inner.pad", 80),
            ("middle.route.Run.callback", 104),
            ("middle.batches[0].Run.clock", 136),
            ("middle.batches[1].Run.callback", 152),
        ]
    );

    let writer = derive_post_handoff_writer(&plan).expect("writer");
    let mut bytes = [0xa5_u8; 168];
    execute_post_handoff_writer_plan(&writer, &mut bytes, post_handoff_site(), |target| {
        if target == entry() {
            Some(0x1122_3344_5566_7788)
        } else {
            assert_eq!(target, data());
            Some(0xdead_beef_cafe_f00d)
        }
    })
    .expect("the recursive sum-array writer resolves each exact slot");

    assert_eq!(&bytes[0..8], &0xdead_beef_cafe_f00d_u64.to_le_bytes());
    assert_eq!(&bytes[16..24], &0xdead_beef_cafe_f00d_u64.to_le_bytes());
    assert_eq!(&bytes[40..48], &0x1122_3344_5566_7788_u64.to_le_bytes());
    assert_eq!(&bytes[72..80], &0xdead_beef_cafe_f00d_u64.to_le_bytes());
    assert_eq!(&bytes[80..88], &0xdead_beef_cafe_f00d_u64.to_le_bytes());
    assert_eq!(&bytes[104..112], &0xdead_beef_cafe_f00d_u64.to_le_bytes());
    assert_eq!(&bytes[136..144], &0x1122_3344_5566_7788_u64.to_le_bytes());
    assert_eq!(&bytes[152..160], &0xdead_beef_cafe_f00d_u64.to_le_bytes());
}

#[test]
fn symbolic_recursive_record_array_materialization_composes_element_boundaries() {
    // A direct `[R; N]` field whose record element still reaches sums folds
    // one `RecordArray` carrier per row: the path `neighbors[1].choice` spells
    // the element index on the field hop, then resolves `choice` inside the
    // addressed element's own record interior — the element report is
    // retained once for every index, and the folded carriers inside it join
    // the same field-keyed namespace as a single nested record's.
    let report = recursive_record_array_report();
    let carriers = SymbolicFieldInnerLayout::from_recursive_sum_paths(&report)
        .expect("the recursive record-array report folds into inner layout carriers");
    assert_eq!(carriers.len(), 1);
    let middle = &carriers[0];
    assert_eq!(middle.field, "middle");
    // One channel in authored member order: `inner`, `route`, `neighbors`.
    let inner_carriers = middle.inner_layouts();
    assert_eq!(inner_carriers.len(), 3);
    assert_eq!(inner_carriers[0].field, "inner");
    assert_eq!(inner_carriers[1].field, "route");
    assert_eq!(inner_carriers[2].field, "neighbors");
    assert!(
        matches!(
            inner_carriers[2].inner_layout.hop,
            ConventionalRecordSumChildHop::Index {
                element_count: 2,
                element_stride: 32,
            }
        ) && matches!(
            inner_carriers[2].inner_layout.interior,
            layout_plans::SymbolicFieldInterior::Record(_)
        ),
        "the record-array row folds into an index hop over a record interior"
    );
    // The element record's own carriers ride inside the `neighbors`
    // carrier exactly as `inner`'s leaf carriers ride inside its `Record`
    // carrier.
    let element_carriers = inner_carriers[2].inner_layouts();
    assert_eq!(element_carriers.len(), 1);
    assert_eq!(element_carriers[0].field, "choice");
    assert_eq!(inner_carriers[0].inner_layouts().len(), 1);
    assert_eq!(inner_carriers[0].inner_layouts()[0].field, "choice");

    let symbolic = [
        SymbolicFieldValue::new("header", 64, data()).expect("scalar field"),
        SymbolicFieldValue::new("middle", 64, data())
            .expect("outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_indexed("neighbors", 1).with_inner_segment(
                    SymbolicFieldPathSegment::new("choice").with_inner_segment(
                        SymbolicFieldPathSegment::new("Run")
                            .with_inner_segment(SymbolicFieldPathSegment::new("callback")),
                    ),
                ),
            ),
        SymbolicFieldValue::new("middle", 64, entry())
            .expect("outer record field")
            .with_inner_segment(
                SymbolicFieldPathSegment::new_indexed("neighbors", 0)
                    .with_inner_segment(SymbolicFieldPathSegment::new("pad")),
            ),
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
    ];
    let plan = derive_symbolic_materialization_with_inner_layouts(
        &report.outer_layout,
        &carriers,
        &symbolic,
        post_handoff_context(),
        |_| None,
    )
    .expect("recursive record-array paths compose every crossed boundary");

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
    // `middle` spans 8..128; inside it `inner` sits at 0, `route` at 32, and
    // `neighbors` repeats at 56 on a 32-byte stride, so `neighbors[1]` lands
    // at 56+32=88 inside the interior and its `choice.Run.callback` resolves
    // at the element's payload offset.
    assert_eq!(
        writes,
        vec![
            ("header", 0),
            ("middle.neighbors[1].choice.Run.callback", 104),
            ("middle.neighbors[0].pad", 88),
            ("middle.inner.choice.Run.clock", 24),
            ("middle.route.Run.callback", 48),
        ]
    );

    let writer = derive_post_handoff_writer(&plan).expect("writer");
    let mut bytes = [0xa5_u8; 128];
    execute_post_handoff_writer_plan(&writer, &mut bytes, post_handoff_site(), |target| {
        if target == entry() {
            Some(0x1122_3344_5566_7788)
        } else {
            assert_eq!(target, data());
            Some(0xdead_beef_cafe_f00d)
        }
    })
    .expect("the recursive record-array writer resolves each exact slot");

    assert_eq!(&bytes[0..8], &0xdead_beef_cafe_f00d_u64.to_le_bytes());
    assert_eq!(&bytes[24..32], &0xdead_beef_cafe_f00d_u64.to_le_bytes());
    assert_eq!(&bytes[48..56], &0xdead_beef_cafe_f00d_u64.to_le_bytes());
    assert_eq!(&bytes[88..96], &0x1122_3344_5566_7788_u64.to_le_bytes());
    assert_eq!(&bytes[104..112], &0xdead_beef_cafe_f00d_u64.to_le_bytes());
}
