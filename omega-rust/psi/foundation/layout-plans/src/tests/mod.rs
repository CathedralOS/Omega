//! Fixtures shared by the layout plan tests: entry layouts and nested
//! layouts.

mod conventional_materializations;
mod symbolic_materializations;
mod symbolic_sum_materializations;
mod writer_fragments;

use crate::{
    ByteOrder, ConsumptionInstant, ConventionalRecordArrayFieldLayoutReport,
    ConventionalRecordSumOccurrenceLayoutReport, ConventionalRecordSumPathsLayoutReport,
    ConventionalRecursiveRecordSumPathsLayoutReport, ConventionalSumArrayFieldLayoutReport,
    ConventionalSumCaseLayoutReport, ConventionalSumFieldLayoutReport, ConventionalSumLayoutReport,
    ConventionalSumPayloadFieldLayoutReport, DataSymbolId, EntryStubId, LayoutFieldEntryReport,
    LayoutPlacementReport, LayoutPlanReport, MaterializationContext, PlacementConstraints,
    PlacementPhase, RelocationTarget, SymbolicFieldInnerLayout, SymbolicFieldInteriorLayout,
};

fn entry() -> RelocationTarget {
    RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x55aa).expect("nonzero identity"),
    )
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

fn data() -> RelocationTarget {
    RelocationTarget::Data(
        DataSymbolId::from_normalized_identity(0x1234).expect("nonzero identity"),
    )
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

/// A nested record field occupies one whole `At` extent inside the flat outer
/// plan; its member offsets live in the interior carrier supplied beside it.
/// `slot` spans bytes 8..24 of the outer plan, and the carrier's `entry`/`flags`
/// members sit at inner offsets 0 and 8, so `slot.entry` composes to byte 8
/// and `slot.flags` to byte 16.
fn nested_layout() -> (LayoutPlanReport, SymbolicFieldInnerLayout) {
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
            size: Some(24),
            align: 8,
        },
        SymbolicFieldInnerLayout::new(
            "slot",
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
        ),
    )
}

/// Extracts the record interior a test carrier binds.
fn record_interior(carrier: &SymbolicFieldInnerLayout) -> LayoutPlanReport {
    let SymbolicFieldInteriorLayout::Record(layout) = &carrier.inner_layout else {
        panic!("the test carrier binds a record interior");
    };
    layout.clone()
}

fn post_handoff_context() -> MaterializationContext {
    MaterializationContext {
        consumption: ConsumptionInstant::AfterOmegaHandoff,
        byte_order: ByteOrder::LittleEndian,
        native_pointer_relocation_bits: None,
        placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
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

/// A record carrying one direct sum field beside scalar members: `header`
/// spans 0..8, `choice` spans 8..32, `tail` spans 32..40.
fn sum_field_layout() -> (LayoutPlanReport, SymbolicFieldInnerLayout) {
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
                    field: "choice".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 8 },
                },
                LayoutFieldEntryReport {
                    field: "tail".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 32 },
                },
            ],
            offsets: Some(vec![0, 8, 32]),
            size: Some(40),
            align: 8,
        },
        SymbolicFieldInnerLayout::new_sum("choice", sum_layout()),
    )
}

/// A recursive record/sum path report carrying record depth as data: the
/// outer plan places `header` at 0 and `middle` at 8 (a 64-byte record);
/// `middle`'s interior places `inner` at 0, `tag` at 32, and its own direct
/// sum `route` at 40; `inner`'s interior places the direct sum `choice` at 0
/// and `pad` at 24. The `middle` `Branch` level holds both child kinds — its
/// direct sum `route` beside the deeper `inner` record path — before the
/// `Leaf` holding `choice`'s complete sum overlay: the shape the recursive
/// projection emits for `Outer { middle: Middle }` where `Middle` co-locates
/// a direct sum with the record path reaching `inner`'s sum.
fn recursive_sum_report() -> ConventionalRecursiveRecordSumPathsLayoutReport {
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
    ConventionalRecursiveRecordSumPathsLayoutReport::Branch(
        ConventionalRecordSumPathsLayoutReport {
            outer_layout: record(1, &[("header", 0), ("middle", 8)], 72),
            child_sum_layouts: Vec::new(),
            child_sum_array_layouts: Vec::new(),
            child_record_array_layouts: Vec::new(),
            paths: vec![ConventionalRecordSumOccurrenceLayoutReport {
                outer_field: "middle".into(),
                outer_member_identity: None,
                inner: ConventionalRecursiveRecordSumPathsLayoutReport::Branch(
                    ConventionalRecordSumPathsLayoutReport {
                        outer_layout: record(2, &[("inner", 0), ("tag", 32), ("route", 40)], 64),
                        child_sum_layouts: vec![ConventionalSumFieldLayoutReport {
                            field: "route".into(),
                            member_identity: None,
                            layout: sum_layout(),
                        }],
                        child_sum_array_layouts: Vec::new(),
                        child_record_array_layouts: Vec::new(),
                        paths: vec![ConventionalRecordSumOccurrenceLayoutReport {
                            outer_field: "inner".into(),
                            outer_member_identity: None,
                            inner: ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
                                outer_layout: record(3, &[("choice", 0), ("pad", 24)], 32),
                                child_sum_layouts: vec![ConventionalSumFieldLayoutReport {
                                    field: "choice".into(),
                                    member_identity: None,
                                    layout: sum_layout(),
                                }],
                                child_sum_array_layouts: Vec::new(),
                                child_record_array_layouts: Vec::new(),
                            },
                        }],
                    },
                ),
            }],
        },
    )
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
    ConventionalRecursiveRecordSumPathsLayoutReport::Branch(
        ConventionalRecordSumPathsLayoutReport {
            outer_layout: record(1, &[("header", 0), ("middle", 8)], 168),
            child_sum_layouts: Vec::new(),
            child_sum_array_layouts: Vec::new(),
            child_record_array_layouts: Vec::new(),
            paths: vec![ConventionalRecordSumOccurrenceLayoutReport {
                outer_field: "middle".into(),
                outer_member_identity: None,
                inner: ConventionalRecursiveRecordSumPathsLayoutReport::Branch(
                    ConventionalRecordSumPathsLayoutReport {
                        outer_layout: record(
                            2,
                            &[("inner", 0), ("tag", 80), ("route", 88), ("batches", 112)],
                            160,
                        ),
                        child_sum_layouts: vec![ConventionalSumFieldLayoutReport {
                            field: "route".into(),
                            member_identity: None,
                            layout: sum_layout(),
                        }],
                        child_sum_array_layouts: vec![ConventionalSumArrayFieldLayoutReport {
                            field: "batches".into(),
                            member_identity: None,
                            element_count: 2,
                            element_stride: 24,
                            element_layout: sum_layout(),
                        }],
                        child_record_array_layouts: Vec::new(),
                        paths: vec![ConventionalRecordSumOccurrenceLayoutReport {
                            outer_field: "inner".into(),
                            outer_member_identity: None,
                            inner: ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
                                outer_layout: record(
                                    3,
                                    &[("choice", 0), ("choices", 24), ("pad", 72)],
                                    80,
                                ),
                                child_sum_layouts: vec![ConventionalSumFieldLayoutReport {
                                    field: "choice".into(),
                                    member_identity: None,
                                    layout: sum_layout(),
                                }],
                                child_sum_array_layouts: vec![
                                    ConventionalSumArrayFieldLayoutReport {
                                        field: "choices".into(),
                                        member_identity: None,
                                        element_count: 2,
                                        element_stride: 24,
                                        element_layout: sum_layout(),
                                    },
                                ],
                                child_record_array_layouts: Vec::new(),
                            },
                        }],
                    },
                ),
            }],
        },
    )
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
    let element = || ConventionalRecursiveRecordSumPathsLayoutReport::Leaf {
        outer_layout: record(3, &[("choice", 0), ("pad", 24)], 32),
        child_sum_layouts: vec![ConventionalSumFieldLayoutReport {
            field: "choice".into(),
            member_identity: None,
            layout: sum_layout(),
        }],
        child_sum_array_layouts: Vec::new(),
        child_record_array_layouts: Vec::new(),
    };
    ConventionalRecursiveRecordSumPathsLayoutReport::Branch(
        ConventionalRecordSumPathsLayoutReport {
            outer_layout: record(1, &[("header", 0), ("middle", 8)], 128),
            child_sum_layouts: Vec::new(),
            child_sum_array_layouts: Vec::new(),
            child_record_array_layouts: Vec::new(),
            paths: vec![ConventionalRecordSumOccurrenceLayoutReport {
                outer_field: "middle".into(),
                outer_member_identity: None,
                inner: ConventionalRecursiveRecordSumPathsLayoutReport::Branch(
                    ConventionalRecordSumPathsLayoutReport {
                        outer_layout: record(
                            2,
                            &[("inner", 0), ("route", 32), ("neighbors", 56)],
                            120,
                        ),
                        child_sum_layouts: vec![ConventionalSumFieldLayoutReport {
                            field: "route".into(),
                            member_identity: None,
                            layout: sum_layout(),
                        }],
                        child_sum_array_layouts: Vec::new(),
                        child_record_array_layouts: vec![
                            ConventionalRecordArrayFieldLayoutReport {
                                field: "neighbors".into(),
                                member_identity: None,
                                element_count: 2,
                                element_stride: 32,
                                inner: element(),
                            },
                        ],
                        paths: vec![ConventionalRecordSumOccurrenceLayoutReport {
                            outer_field: "inner".into(),
                            outer_member_identity: None,
                            inner: element(),
                        }],
                    },
                ),
            }],
        },
    )
}

/// `sums` repeats the shared sum interior twice at a 24-byte stride inside
/// one whole-extent `At` placement spanning 8..56, the shape the conventional
/// projection emits for a `[Choice; 2]` field.
fn sum_array_layout() -> (LayoutPlanReport, SymbolicFieldInnerLayout) {
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
                    field: "sums".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 8 },
                },
            ],
            offsets: Some(vec![0, 8]),
            size: Some(56),
            align: 8,
        },
        SymbolicFieldInnerLayout::new_sum_array("sums", sum_layout(), 2, 24),
    )
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
