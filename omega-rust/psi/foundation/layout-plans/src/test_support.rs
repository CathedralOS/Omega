//! Fresh owned layout fixtures for tests of portable plans and their writer consumers.

use crate::{
    ArtifactInstallationScopeId, ByteOrder, ConsumptionInstant, ConventionalRecordSumChildHop,
    ConventionalRecordSumChildInterior, ConventionalRecordSumChildLayoutReport,
    ConventionalRecursiveRecordSumPathsLayoutReport, ConventionalSumCaseLayoutReport,
    ConventionalSumLayoutReport, ConventionalSumPayloadFieldLayoutReport, DataSymbolId,
    EntryStubId, IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport,
    LayoutPlanReport, MachineRegimeId, MaterializationContext, PlacementAddressRange,
    PlacementConstraints, PlacementPhase, RelocationTarget, SymbolicFieldInnerLayout,
    SymbolicFieldPathSegment, SymbolicFieldValue,
};

/// A nested symbolic path: `symbolic_path(&["a", "b"])` is `a` containing `b`.
pub fn symbolic_path(segments: &[&str]) -> SymbolicFieldPathSegment {
    let (innermost, outer) = segments.split_last().expect("a path needs a head");
    let mut segment = SymbolicFieldPathSegment::new(*innermost);
    for name in outer.iter().rev() {
        segment = SymbolicFieldPathSegment::new(*name).with_inner_segment(segment);
    }
    segment
}

/// The post-handoff materialization context with a caller-chosen placement.
pub fn post_handoff_context_with(placement: PlacementConstraints) -> MaterializationContext {
    MaterializationContext {
        consumption: ConsumptionInstant::AfterOmegaHandoff,
        byte_order: ByteOrder::LittleEndian,
        native_pointer_relocation_bits: None,
        placement,
    }
}

/// The post-handoff materialization context with no placement constraints.
pub fn post_handoff_context() -> MaterializationContext {
    post_handoff_context_with(PlacementConstraints::unconstrained(
        PlacementPhase::PostHandoff,
    ))
}

/// A field entry row with no member identity — the common fixture row.
pub fn layout_field_entry(field: &str, placement: LayoutPlacementReport) -> LayoutFieldEntryReport {
    LayoutFieldEntryReport {
        field: field.into(),
        member_identity: None,
        placement,
    }
}

/// A field entry row carrying its compiler-retained stable member identity,
/// for fixtures whose rows must rejoin renamed schemas by identity.
pub fn numbered_layout_field_entry(
    field: &str,
    member_identity: u64,
    placement: LayoutPlacementReport,
) -> LayoutFieldEntryReport {
    LayoutFieldEntryReport {
        field: field.into(),
        member_identity: Some(member_identity),
        placement,
    }
}

/// A report shell at fingerprint 1 — fixture tests vary only the rows and
/// the derived geometry, never the schema fingerprint.
pub fn layout_report(
    entries: Vec<LayoutFieldEntryReport>,
    offsets: Option<Vec<u64>>,
    size: Option<u64>,
    align: u64,
) -> LayoutPlanReport {
    LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries,
        offsets,
        size,
        align,
    }
}

pub fn split_layout() -> LayoutPlanReport {
    LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: vec![
            layout_field_entry(
                "address".into(),
                LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 16,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 16,
                },
            ),
            layout_field_entry(
                "address".into(),
                LayoutPlacementReport::Bits {
                    container: 2,
                    container_width: 16,
                    destination_lsb: 0,
                    source_lsb: 16,
                    width: 16,
                },
            ),
            layout_field_entry(
                "address".into(),
                LayoutPlacementReport::Bits {
                    container: 8,
                    container_width: 64,
                    destination_lsb: 0,
                    source_lsb: 32,
                    width: 32,
                },
            ),
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
pub fn repeated_layout() -> LayoutPlanReport {
    LayoutPlanReport {
        schema_report_fingerprint: 1,
        entries: (0..4)
            .map(|index| {
                numbered_layout_field_entry(
                    "handlers".into(),
                    9,
                    LayoutPlacementReport::At { offset: index * 8 },
                )
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
pub fn deeply_nested_layout() -> (LayoutPlanReport, SymbolicFieldInnerLayout) {
    (
        LayoutPlanReport {
            schema_report_fingerprint: 1,
            entries: vec![
                layout_field_entry("header".into(), LayoutPlacementReport::At { offset: 0 }),
                layout_field_entry("slot".into(), LayoutPlacementReport::At { offset: 8 }),
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
                    layout_field_entry("pad".into(), LayoutPlacementReport::At { offset: 0 }),
                    layout_field_entry("sub".into(), LayoutPlacementReport::At { offset: 8 }),
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
                    layout_field_entry("entry".into(), LayoutPlacementReport::At { offset: 0 }),
                    layout_field_entry("flags".into(), LayoutPlacementReport::At { offset: 8 }),
                    layout_field_entry("args".into(), LayoutPlacementReport::At { offset: 16 }),
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
pub fn sum_layout() -> ConventionalSumLayoutReport {
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

/// A recursive record/sum path report carrying a direct array of records that
/// still reach sums: `middle`'s interior places the deeper record `inner` at
/// 0 (a 32-byte leaf holding `choice` at 0 and `pad` at 24), its own direct
/// sum `route` at 32, and `neighbors` — two 32-byte elements of the same
/// leaf record at a 32-byte stride — spanning 56..120. The `neighbors` row's
/// `inner` is the element record's own `Leaf` report, retained once for
/// every index the field spells, so `middle.neighbors[1].choice.Run.<p>`
/// composes the element hop and the record boundary inside it under the same
/// bounded traversal the standalone record carrier walks.
/// The `address`-field writer plan over the split layout with no
/// resolutions: the plan every post-handoff writer test starts from.
pub fn post_handoff_split_writer_plan() -> crate::SymbolicMaterializationPlan {
    let symbolic = SymbolicFieldValue::new("address", 64, entry()).expect("symbolic field");
    crate::derive_symbolic_materialization(
        &split_layout(),
        &[symbolic],
        post_handoff_context(),
        |_| None,
    )
    .expect("writer plan")
}

/// The `(field, byte offset)` projection of a plan's runtime writers, in
/// action order.
pub fn writer_field_offsets(plan: &crate::SymbolicMaterializationPlan) -> Vec<(&str, u64)> {
    plan.actions
        .iter()
        .map(|action| match action {
            crate::MaterializationAction::RuntimeWriter(write) => {
                (write.field.as_str(), write.container_byte_offset)
            }
            other => panic!("an unresolved symbolic derives a runtime writer, found {other:?}"),
        })
        .collect()
}

/// A single `address` field stored as a 32-bit unsigned integer.
pub fn stored_integer_layout() -> LayoutPlanReport {
    LayoutPlanReport {
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
    }
}

/// A flat record layout: `record_layout(1, &[("a", 0), ("b", 8)], 16)` places
/// each field at its offset in declaration order.
pub fn record_layout(fingerprint: u64, fields: &[(&str, u64)], size: u64) -> LayoutPlanReport {
    LayoutPlanReport {
        schema_report_fingerprint: fingerprint,
        entries: fields
            .iter()
            .map(|&(field, offset)| {
                layout_field_entry(field.into(), LayoutPlacementReport::At { offset })
            })
            .collect(),
        offsets: Some(fields.iter().map(|&(_, offset)| offset).collect()),
        size: Some(size),
        align: 8,
    }
}

pub fn recursive_record_array_report() -> ConventionalRecursiveRecordSumPathsLayoutReport {
    let element = || ConventionalRecursiveRecordSumPathsLayoutReport {
        outer_layout: record_layout(3, &[("choice", 0), ("pad", 24)], 32),
        children: vec![ConventionalRecordSumChildLayoutReport {
            field: "choice".into(),
            member_identity: None,
            hop: ConventionalRecordSumChildHop::Field,
            interior: ConventionalRecordSumChildInterior::Sum(sum_layout()),
        }],
    };
    ConventionalRecursiveRecordSumPathsLayoutReport {
        outer_layout: record_layout(1, &[("header", 0), ("middle", 8)], 128),
        children: vec![ConventionalRecordSumChildLayoutReport {
            field: "middle".into(),
            member_identity: None,
            hop: ConventionalRecordSumChildHop::Field,
            interior: ConventionalRecordSumChildInterior::Record(
                ConventionalRecursiveRecordSumPathsLayoutReport {
                    outer_layout: record_layout(
                        2,
                        &[("inner", 0), ("route", 32), ("neighbors", 56)],
                        120,
                    ),
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

/// The relocation target every symbolic test field uses.
pub fn entry() -> RelocationTarget {
    RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x55aa).expect("nonzero identity"),
    )
}

/// The data relocation target used beside `entry` in test layouts.
pub fn data() -> RelocationTarget {
    RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x1234).expect("nonzero identity"),
    )
}

/// The constrained post-handoff plan over a `0x1000..0x10_0000` range:
/// the materialization plan plus the regime and scope identities the
/// constraint carries, for tests that build placement sites from them.
pub fn constrained_post_handoff_plan() -> (
    crate::SymbolicMaterializationPlan,
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
    let plan = crate::derive_symbolic_materialization(
        &layout,
        &[symbolic],
        post_handoff_context_with(constraints),
        |_| None,
    )
    .expect("constrained post-handoff plan");
    (plan, regime, scope)
}
