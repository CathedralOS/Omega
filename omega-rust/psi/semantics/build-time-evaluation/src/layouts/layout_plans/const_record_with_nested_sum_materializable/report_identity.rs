//! Hash-free replay equality and non-authoritative report fingerprints.

use super::{
    BuildTimeValue, ByteOrder, ConventionalNestedRecordSumPathLayoutReport,
    ConventionalNestedRecordSumPathsLayoutReport, ConventionalRecordSumPathsLayoutReport,
    ConventionalRecursiveRecordSumPathsLayoutReport, ConventionalSumArrayFieldLayoutReport,
    ConventionalSumFieldLayoutReport, ValidatedConstNestedSumRecordOccurrenceMaterialization,
    ValidatedConstRecordSumArrayFieldMaterialization, ValidatedConstRecordSumFieldMaterialization,
    ValidatedConstRecordWithSumMaterialization,
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization,
    conventional_sum_layout_reports_match_for_replay, field_occurrence_matches, hash_byte,
    hash_bytes, hash_compact_sum_array_occurrence, hash_text, hash_u64, hash_value,
    layout_plan_reports_match_for_replay, normalized_layout_plan_report_fingerprint,
    sum_array_layout_sets_match_for_replay,
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

fn child_sum_rows_match_for_replay(
    left: &[ConventionalSumFieldLayoutReport],
    right: &[ConventionalSumFieldLayoutReport],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            field_occurrence_matches(
                &left.field,
                left.member_identity,
                &right.field,
                right.member_identity,
            ) && conventional_sum_layout_reports_match_for_replay(&left.layout, &right.layout)
        })
}

impl RecordSumPathsReplay for ConventionalRecursiveRecordSumPathsLayoutReport {
    fn matches_for_replay(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Leaf {
                    outer_layout: left_layout,
                    child_sum_layouts: left_sums,
                    child_sum_array_layouts: left_arrays,
                },
                Self::Leaf {
                    outer_layout: right_layout,
                    child_sum_layouts: right_sums,
                    child_sum_array_layouts: right_arrays,
                },
            ) => {
                layout_plan_reports_match_for_replay(left_layout, right_layout)
                    && child_sum_rows_match_for_replay(left_sums, right_sums)
                    && sum_array_layout_sets_match_for_replay(left_arrays, right_arrays)
            }
            (Self::Branch(left), Self::Branch(right)) => left.matches_for_replay(right),
            _ => false,
        }
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

impl RecordSumPathsReplay for ConventionalRecordSumPathsLayoutReport {
    fn matches_for_replay(&self, other: &Self) -> bool {
        layout_plan_reports_match_for_replay(&self.outer_layout, &other.outer_layout)
            && child_sum_rows_match_for_replay(&self.child_sum_layouts, &other.child_sum_layouts)
            && sum_array_layout_sets_match_for_replay(
                &self.child_sum_array_layouts,
                &other.child_sum_array_layouts,
            )
            && self.paths.len() == other.paths.len()
            && self.paths.iter().zip(&other.paths).all(|(left, right)| {
                field_occurrence_matches(
                    &left.outer_field,
                    left.outer_member_identity,
                    &right.outer_field,
                    right.outer_member_identity,
                ) && left.inner.matches_for_replay(&right.inner)
            })
    }
}

pub(super) fn record_sum_paths_reports_match_for_replay<Paths: RecordSumPathsReplay>(
    left: &Paths,
    right: &Paths,
) -> bool {
    left.matches_for_replay(right)
}

fn hash_direct_sum_occurrences(
    hash: &mut u64,
    nested_sums: &[ValidatedConstRecordSumFieldMaterialization],
) {
    hash_u64(hash, nested_sums.len() as u64);
    for nested in nested_sums {
        match nested.field_identity {
            Some(identity) => {
                hash_byte(hash, 1);
                hash_u64(hash, identity);
            }
            None => {
                hash_byte(hash, 0);
                hash_text(hash, &nested.field);
            }
        }
        hash_u64(
            hash,
            nested
                .nested_sum
                .non_authoritative_layout_report_fingerprint(),
        );
        match nested.nested_sum.selected_case_identity() {
            Some(identity) => {
                hash_byte(hash, 1);
                hash_u64(hash, identity);
            }
            None => hash_byte(hash, 0),
        }
        hash_u64(hash, u64::from(nested.nested_sum.selected_case_ordinal()));
    }
}

fn hash_direct_sum_array_occurrences(
    hash: &mut u64,
    array_layouts: &[ConventionalSumArrayFieldLayoutReport],
    nested_sum_arrays: &[ValidatedConstRecordSumArrayFieldMaterialization],
) {
    hash_u64(hash, nested_sum_arrays.len() as u64);
    for (array_layout, array) in array_layouts.iter().zip(nested_sum_arrays) {
        hash_compact_sum_array_occurrence(hash, array_layout, &array.elements);
    }
}

/// Non-authoritative report coordinate for one recursive record level's
/// complete direct-sum children — direct sums and direct sum arrays alike.
pub(super) fn record_level_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    child_sum_array_layouts: &[ConventionalSumArrayFieldLayoutReport],
    nested_sums: &[ValidatedConstRecordSumFieldMaterialization],
    nested_sum_arrays: &[ValidatedConstRecordSumArrayFieldMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-record-level-sum-children.v1",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    hash_direct_sum_occurrences(&mut hash, nested_sums);
    hash_direct_sum_array_occurrences(&mut hash, child_sum_array_layouts, nested_sum_arrays);
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

pub(super) fn record_sum_paths_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstRecursiveNestedSumOccurrenceMaterialization],
    nested_sums: &[ValidatedConstRecordSumFieldMaterialization],
    nested_sum_arrays: &[ValidatedConstRecordSumArrayFieldMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-recursive-record-sum-paths.v3",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    hash_direct_sum_occurrences(&mut hash, nested_sums);
    hash_direct_sum_array_occurrences(
        &mut hash,
        &path_layout.child_sum_array_layouts,
        nested_sum_arrays,
    );
    hash_u64(&mut hash, occurrences.len() as u64);
    for (path, occurrence) in path_layout.paths.iter().zip(occurrences) {
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
            normalized_layout_plan_report_fingerprint(path.inner.outer_layout()),
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
