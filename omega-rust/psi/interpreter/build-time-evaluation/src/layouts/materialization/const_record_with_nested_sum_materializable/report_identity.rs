//! Hash-free replay equality and non-authoritative report fingerprints.

use super::{
    BuildTimeValue, ByteOrder, ConventionalNestedRecordSumPathLayoutReport,
    ConventionalNestedRecordSumPathsLayoutReport, ConventionalRecordSumChildHop,
    ConventionalRecordSumChildInterior, ConventionalRecordSumChildLayoutReport,
    ConventionalRecursiveRecordSumPathsLayoutReport,
    ValidatedConstNestedSumRecordOccurrenceMaterialization,
    ValidatedConstRecordArrayElementSelection, ValidatedConstRecordSumArrayElementSelection,
    ValidatedConstRecordSumChildMaterialization, ValidatedConstRecordWithSumMaterialization,
    conventional_sum_layout_reports_match_for_replay, field_occurrence_matches, hash_byte,
    hash_bytes, hash_text, hash_u64, hash_value, layout_plan_reports_match_for_replay,
    normalized_conventional_sum_layout_report_fingerprint,
    normalized_layout_plan_report_fingerprint,
};
pub(super) fn nested_path_reports_match_for_replay(
    left: &ConventionalNestedRecordSumPathLayoutReport,
    right: &ConventionalNestedRecordSumPathLayoutReport,
) -> bool {
    layout_plan_reports_match_for_replay(&left.outer_layout, &right.outer_layout)
        && field_occurrence_matches(
            &left.outer_field,
            left.outer_member_identity,
            &right.outer_field,
            right.outer_member_identity,
        )
        && layout_plan_reports_match_for_replay(&left.inner_layout, &right.inner_layout)
        && left.child_sum_layouts.len() == right.child_sum_layouts.len()
        && left
            .child_sum_layouts
            .iter()
            .zip(&right.child_sum_layouts)
            .all(|(left, right)| {
                field_occurrence_matches(
                    &left.field,
                    left.member_identity,
                    &right.field,
                    right.member_identity,
                ) && conventional_sum_layout_reports_match_for_replay(&left.layout, &right.layout)
            })
}

pub(super) trait RecordSumPathsReplay {
    fn matches_for_replay(&self, other: &Self) -> bool;
}

/// Hash-free replay equality for one classified child row: the same field
/// occurrence, the same hop — `Field` or a literal `Index` carrying equal
/// count and stride — and recursively equal interiors.
fn child_rows_match_for_replay(
    left: &ConventionalRecordSumChildLayoutReport,
    right: &ConventionalRecordSumChildLayoutReport,
) -> bool {
    field_occurrence_matches(
        &left.field,
        left.member_identity,
        &right.field,
        right.member_identity,
    ) && left.hop == right.hop
        && match (&left.interior, &right.interior) {
            (
                ConventionalRecordSumChildInterior::Sum(left),
                ConventionalRecordSumChildInterior::Sum(right),
            ) => conventional_sum_layout_reports_match_for_replay(left, right),
            (
                ConventionalRecordSumChildInterior::Record(left),
                ConventionalRecordSumChildInterior::Record(right),
            ) => left.matches_for_replay(right),
            _ => false,
        }
}

fn record_array_selections_match(
    left: &[ValidatedConstRecordArrayElementSelection],
    right: &[ValidatedConstRecordArrayElementSelection],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.literal_index == right.literal_index
                && left.value == right.value
                && left.bytes == right.bytes
                && left.non_authoritative_materialization_report_fingerprint
                    == right.non_authoritative_materialization_report_fingerprint
        })
}

fn sum_array_element_selections_match(
    left: &[ValidatedConstRecordSumArrayElementSelection],
    right: &[ValidatedConstRecordSumArrayElementSelection],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.literal_index == right.literal_index
                && left.non_authoritative_schema_report_fingerprint
                    == right.non_authoritative_schema_report_fingerprint
                && left.value == right.value
                && left.non_authoritative_layout_report_fingerprint
                    == right.non_authoritative_layout_report_fingerprint
                && left.selected_case_identity == right.selected_case_identity
                && left.selected_case_ordinal == right.selected_case_ordinal
                && left.bytes == right.bytes
                && left.non_authoritative_materialization_report_fingerprint
                    == right.non_authoritative_materialization_report_fingerprint
        })
}

/// Exact replay comparison for one retained child custody row: the same
/// custody kind carrying equal field identity and equal nested coordinates.
/// A record child's deeper level replays pairwise at the call site before
/// this fingerprint coordinate compares.
pub(super) fn child_materializations_match_for_replay(
    left: &ValidatedConstRecordSumChildMaterialization,
    right: &ValidatedConstRecordSumChildMaterialization,
) -> bool {
    match (left, right) {
        (
            ValidatedConstRecordSumChildMaterialization::Sum(left),
            ValidatedConstRecordSumChildMaterialization::Sum(right),
        ) => {
            field_occurrence_matches(
                &left.field,
                left.field_identity,
                &right.field,
                right.field_identity,
            ) && left.nested_sum.schema_name() == right.nested_sum.schema_name()
                && left.nested_sum.value() == right.nested_sum.value()
                && left.nested_sum.selected_case_identity()
                    == right.nested_sum.selected_case_identity()
                && left.nested_sum.selected_case_ordinal()
                    == right.nested_sum.selected_case_ordinal()
                && left.nested_sum.bytes() == right.nested_sum.bytes()
                && conventional_sum_layout_reports_match_for_replay(
                    left.nested_sum.layout(),
                    right.nested_sum.layout(),
                )
                && left
                    .nested_sum
                    .non_authoritative_materialization_report_fingerprint()
                    == right
                        .nested_sum
                        .non_authoritative_materialization_report_fingerprint()
        }
        (
            ValidatedConstRecordSumChildMaterialization::SumArray(left),
            ValidatedConstRecordSumChildMaterialization::SumArray(right),
        ) => {
            field_occurrence_matches(
                &left.field,
                left.field_identity,
                &right.field,
                right.field_identity,
            ) && sum_array_element_selections_match(&left.elements, &right.elements)
        }
        (
            ValidatedConstRecordSumChildMaterialization::Record(left),
            ValidatedConstRecordSumChildMaterialization::Record(right),
        ) => {
            field_occurrence_matches(
                left.outer_field(),
                left.outer_member_identity(),
                right.outer_field(),
                right.outer_member_identity(),
            ) && left
                .inner
                .non_authoritative_materialization_report_fingerprint()
                == right
                    .inner
                    .non_authoritative_materialization_report_fingerprint()
        }
        (
            ValidatedConstRecordSumChildMaterialization::RecordArray(left),
            ValidatedConstRecordSumChildMaterialization::RecordArray(right),
        ) => {
            field_occurrence_matches(
                &left.field,
                left.field_identity,
                &right.field,
                right.field_identity,
            ) && record_array_selections_match(&left.elements, &right.elements)
        }
        _ => false,
    }
}

impl RecordSumPathsReplay for ConventionalRecursiveRecordSumPathsLayoutReport {
    fn matches_for_replay(&self, other: &Self) -> bool {
        layout_plan_reports_match_for_replay(&self.outer_layout, &other.outer_layout)
            && self.children.len() == other.children.len()
            && self
                .children
                .iter()
                .zip(&other.children)
                .all(|(left, right)| child_rows_match_for_replay(left, right))
    }
}

impl RecordSumPathsReplay for ConventionalNestedRecordSumPathsLayoutReport {
    fn matches_for_replay(&self, other: &Self) -> bool {
        layout_plan_reports_match_for_replay(&self.outer_layout, &other.outer_layout)
            && self.paths.len() == other.paths.len()
            && self.paths.iter().zip(&other.paths).all(|(left, right)| {
                field_occurrence_matches(
                    &left.outer_field,
                    left.outer_member_identity,
                    &right.outer_field,
                    right.outer_member_identity,
                ) && layout_plan_reports_match_for_replay(&left.inner_layout, &right.inner_layout)
                    && left.child_sum_layouts.len() == right.child_sum_layouts.len()
                    && left
                        .child_sum_layouts
                        .iter()
                        .zip(&right.child_sum_layouts)
                        .all(|(left, right)| {
                            field_occurrence_matches(
                                &left.field,
                                left.member_identity,
                                &right.field,
                                right.member_identity,
                            ) && conventional_sum_layout_reports_match_for_replay(
                                &left.layout,
                                &right.layout,
                            )
                        })
            })
    }
}

pub(super) fn record_sum_paths_reports_match_for_replay<Paths: RecordSumPathsReplay>(
    left: &Paths,
    right: &Paths,
) -> bool {
    left.matches_for_replay(right)
}

fn hash_field_identity(hash: &mut u64, identity: Option<u64>, field: &str) {
    match identity {
        Some(identity) => {
            hash_byte(hash, 1);
            hash_u64(hash, identity);
        }
        None => {
            hash_byte(hash, 0);
            hash_text(hash, field);
        }
    }
}

/// Non-authoritative coordinate for one `[R; N]` child row's compact
/// custody: the literal count and stride, the shared element report's outer
/// layout coordinate, and every indexed element's recursive materialization
/// coordinate. The shared inner report itself is compared hash-free on
/// replay.
fn hash_record_array_child_custody(
    hash: &mut u64,
    element_count: u64,
    element_stride: u64,
    inner_outer_layout: &layout_plans::LayoutPlanReport,
    elements: &[ValidatedConstRecordArrayElementSelection],
) {
    hash_u64(hash, element_count);
    hash_u64(hash, element_stride);
    hash_u64(
        hash,
        normalized_layout_plan_report_fingerprint(inner_outer_layout),
    );
    hash_u64(hash, elements.len() as u64);
    for element in elements {
        hash_u64(hash, element.literal_index);
        hash_value(hash, &element.value);
        hash_u64(hash, element.bytes.len() as u64);
        hash_bytes(hash, &element.bytes);
        hash_u64(
            hash,
            element.non_authoritative_materialization_report_fingerprint,
        );
    }
}

/// Hash one supplied child row beside its retained custody: the row's field
/// identity and hop vocabulary, then the coordinate matching its kind.
fn hash_child_row_with_custody(
    hash: &mut u64,
    row: &ConventionalRecordSumChildLayoutReport,
    child: &ValidatedConstRecordSumChildMaterialization,
) {
    hash_field_identity(hash, row.member_identity, &row.field);
    match &row.hop {
        ConventionalRecordSumChildHop::Field => hash_byte(hash, 0),
        ConventionalRecordSumChildHop::Index {
            element_count,
            element_stride,
        } => {
            hash_byte(hash, 1);
            hash_u64(hash, *element_count);
            hash_u64(hash, *element_stride);
        }
    }
    match child {
        ValidatedConstRecordSumChildMaterialization::Sum(custody) => {
            hash_byte(hash, 0);
            hash_u64(
                hash,
                custody
                    .nested_sum
                    .non_authoritative_layout_report_fingerprint(),
            );
            match custody.nested_sum.selected_case_identity() {
                Some(identity) => {
                    hash_byte(hash, 1);
                    hash_u64(hash, identity);
                }
                None => hash_byte(hash, 0),
            }
            hash_u64(hash, u64::from(custody.nested_sum.selected_case_ordinal()));
        }
        ValidatedConstRecordSumChildMaterialization::SumArray(custody) => {
            hash_byte(hash, 1);
            let ConventionalRecordSumChildInterior::Sum(element_layout) = &row.interior else {
                unreachable!("sum-array custody pairs with an index hop beside a sum interior")
            };
            hash_u64(
                hash,
                normalized_conventional_sum_layout_report_fingerprint(element_layout),
            );
            hash_u64(hash, custody.elements.len() as u64);
            for element in &custody.elements {
                hash_u64(hash, element.literal_index);
                hash_u64(hash, element.non_authoritative_schema_report_fingerprint);
                hash_value(hash, &element.value);
                hash_u64(hash, element.non_authoritative_layout_report_fingerprint);
                match element.selected_case_identity {
                    Some(identity) => {
                        hash_byte(hash, 1);
                        hash_u64(hash, identity);
                    }
                    None => hash_byte(hash, 0),
                }
                hash_u64(hash, u64::from(element.selected_case_ordinal));
                hash_u64(hash, element.bytes.len() as u64);
                hash_bytes(hash, &element.bytes);
                hash_u64(
                    hash,
                    element.non_authoritative_materialization_report_fingerprint,
                );
            }
        }
        ValidatedConstRecordSumChildMaterialization::Record(occurrence) => {
            hash_byte(hash, 2);
            let ConventionalRecordSumChildInterior::Record(inner) = &row.interior else {
                unreachable!("record custody pairs with a field hop beside a record interior")
            };
            hash_u64(
                hash,
                normalized_layout_plan_report_fingerprint(&inner.outer_layout),
            );
            hash_u64(
                hash,
                occurrence
                    .inner
                    .non_authoritative_materialization_report_fingerprint(),
            );
        }
        ValidatedConstRecordSumChildMaterialization::RecordArray(custody) => {
            hash_byte(hash, 3);
            let (
                ConventionalRecordSumChildHop::Index {
                    element_count,
                    element_stride,
                },
                ConventionalRecordSumChildInterior::Record(inner),
            ) = (&row.hop, &row.interior)
            else {
                unreachable!(
                    "record-array custody pairs with an index hop beside a record interior"
                )
            };
            hash_record_array_child_custody(
                hash,
                *element_count,
                *element_stride,
                &inner.outer_layout,
                &custody.elements,
            );
        }
    }
}

/// Non-authoritative report coordinate for one recursive record level's
/// complete child custody — every classified child row on the single
/// `children` channel beside its retained custody.
pub(super) fn recursive_level_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalRecursiveRecordSumPathsLayoutReport,
    children: &[ValidatedConstRecordSumChildMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-recursive-level-children.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    hash_u64(&mut hash, path_layout.children.len() as u64);
    for (row, child) in path_layout.children.iter().zip(children) {
        hash_child_row_with_custody(&mut hash, row, child);
    }
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}
pub(super) fn nested_record_sum_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalNestedRecordSumPathLayoutReport,
    inner: &ValidatedConstRecordWithSumMaterialization,
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-record-with-nested-sum-record.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    match path_layout.outer_member_identity {
        Some(identity) => {
            hash_byte(&mut hash, 1);
            hash_u64(&mut hash, identity);
        }
        None => {
            hash_byte(&mut hash, 0);
            hash_text(&mut hash, &path_layout.outer_field);
        }
    }
    hash_u64(
        &mut hash,
        normalized_layout_plan_report_fingerprint(&path_layout.inner_layout),
    );
    hash_u64(
        &mut hash,
        inner.non_authoritative_materialization_report_fingerprint(),
    );
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}

pub(super) fn nested_record_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalNestedRecordSumPathsLayoutReport,
    inner_records: &[ValidatedConstNestedSumRecordOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-record-with-nested-sum-records.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    hash_u64(&mut hash, inner_records.len() as u64);
    for (path, occurrence) in path_layout.paths.iter().zip(inner_records) {
        match path.outer_member_identity {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => {
                hash_byte(&mut hash, 0);
                hash_text(&mut hash, &path.outer_field);
            }
        }
        hash_u64(
            &mut hash,
            normalized_layout_plan_report_fingerprint(&path.inner_layout),
        );
        hash_u64(
            &mut hash,
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint(),
        );
    }
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}
