//! Fixtures shared by the layout plan tests: entry layouts and nested
//! layouts.

use crate::test_support::{
    data, deeply_nested_layout, entry, layout_field_entry, layout_report,
    numbered_layout_field_entry, post_handoff_context, post_handoff_context_with, record_layout,
    recursive_record_array_report, repeated_layout, split_layout, stored_integer_layout,
    sum_layout, symbolic_path, writer_field_offsets,
};

mod conventional_materializations;
mod symbolic_materializations;
mod symbolic_sum_materializations;
mod writer_fragments;

use crate::{
    ConventionalRecordSumChildHop, ConventionalRecordSumChildInterior,
    ConventionalRecordSumChildLayoutReport, ConventionalRecursiveRecordSumPathsLayoutReport,
    LayoutPlacementReport, LayoutPlanReport, SymbolicFieldInnerLayout, SymbolicFieldValue,
};

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
                layout_field_entry("header".into(), LayoutPlacementReport::At { offset: 0 }),
                layout_field_entry("slot".into(), LayoutPlacementReport::At { offset: 8 }),
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
                    layout_field_entry("entry".into(), LayoutPlacementReport::At { offset: 0 }),
                    layout_field_entry("flags".into(), LayoutPlacementReport::At { offset: 8 }),
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
    let crate::SymbolicFieldInterior::Record(layout) = &carrier.inner_layout.interior else {
        panic!("the test carrier binds a record interior");
    };
    layout.clone()
}

/// A record carrying one direct sum field beside scalar members: `header`
/// spans 0..8, `choice` spans 8..32, `tail` spans 32..40.
fn sum_field_layout() -> (LayoutPlanReport, SymbolicFieldInnerLayout) {
    (
        LayoutPlanReport {
            schema_report_fingerprint: 1,
            entries: vec![
                layout_field_entry("header".into(), LayoutPlacementReport::At { offset: 0 }),
                layout_field_entry("choice".into(), LayoutPlacementReport::At { offset: 8 }),
                layout_field_entry("tail".into(), LayoutPlacementReport::At { offset: 32 }),
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
/// and `pad` at 24. The `middle` level holds both child kinds on one channel
/// — the deeper `inner` record path beside its direct sum `route` — above
/// the level holding `choice`'s complete sum overlay: the shape the
/// recursive projection emits for `Outer { middle: Middle }` where `Middle`
/// co-locates a direct sum with the record path reaching `inner`'s sum.
fn recursive_sum_report() -> ConventionalRecursiveRecordSumPathsLayoutReport {
    ConventionalRecursiveRecordSumPathsLayoutReport {
        outer_layout: record_layout(1, &[("header", 0), ("middle", 8)], 72),
        children: vec![ConventionalRecordSumChildLayoutReport {
            field: "middle".into(),
            member_identity: None,
            hop: ConventionalRecordSumChildHop::Field,
            interior: ConventionalRecordSumChildInterior::Record(
                ConventionalRecursiveRecordSumPathsLayoutReport {
                    outer_layout: record_layout(2, &[("inner", 0), ("tag", 32), ("route", 40)], 64),
                    children: vec![
                        ConventionalRecordSumChildLayoutReport {
                            field: "inner".into(),
                            member_identity: None,
                            hop: ConventionalRecordSumChildHop::Field,
                            interior: ConventionalRecordSumChildInterior::Record(
                                ConventionalRecursiveRecordSumPathsLayoutReport {
                                    outer_layout: record_layout(
                                        3,
                                        &[("choice", 0), ("pad", 24)],
                                        32,
                                    ),
                                    children: vec![ConventionalRecordSumChildLayoutReport {
                                        field: "choice".into(),
                                        member_identity: None,
                                        hop: ConventionalRecordSumChildHop::Field,
                                        interior: ConventionalRecordSumChildInterior::Sum(
                                            sum_layout(),
                                        ),
                                    }],
                                },
                            ),
                        },
                        ConventionalRecordSumChildLayoutReport {
                            field: "route".into(),
                            member_identity: None,
                            hop: ConventionalRecordSumChildHop::Field,
                            interior: ConventionalRecordSumChildInterior::Sum(sum_layout()),
                        },
                    ],
                },
            ),
        }],
    }
}

/// `sums` repeats the shared sum interior twice at a 24-byte stride inside
/// one whole-extent `At` placement spanning 8..56, the shape the conventional
/// projection emits for a `[Choice; 2]` field.
fn sum_array_layout() -> (LayoutPlanReport, SymbolicFieldInnerLayout) {
    (
        LayoutPlanReport {
            schema_report_fingerprint: 1,
            entries: vec![
                layout_field_entry("header".into(), LayoutPlacementReport::At { offset: 0 }),
                layout_field_entry("sums".into(), LayoutPlacementReport::At { offset: 8 }),
            ],
            offsets: Some(vec![0, 8]),
            size: Some(56),
            align: 8,
        },
        SymbolicFieldInnerLayout::new_sum_array("sums", sum_layout(), 2, 24),
    )
}

/// A post-handoff inner-layout materialization that must reject: returns the
/// diagnostic text so each fixture asserts the reason it carries.
pub(super) fn reject_inner_layout_materialization(
    layout: &LayoutPlanReport,
    inner_layouts: &[SymbolicFieldInnerLayout],
    symbolic_fields: &[SymbolicFieldValue],
    expectation: &str,
) -> String {
    crate::derive_symbolic_materialization_with_inner_layouts(
        layout,
        inner_layouts,
        symbolic_fields,
        post_handoff_context(),
        |_| None,
    )
    .expect_err(expectation)
    .0
}
