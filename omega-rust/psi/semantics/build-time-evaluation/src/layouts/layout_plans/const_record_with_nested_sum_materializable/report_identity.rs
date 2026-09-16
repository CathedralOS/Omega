//! Hash-free replay equality and non-authoritative report fingerprints.

use super::{
    BuildTimeValue, ByteOrder, ConventionalNestedRecordSumPathLayoutReport,
    ConventionalNestedRecordSumPathsLayoutReport, ConventionalRecordSumPathsLayoutReport,
    ConventionalRecursiveRecordSumPathsLayoutReport,
    ValidatedConstNestedSumRecordOccurrenceMaterialization,
    ValidatedConstRecordSumFieldMaterialization, ValidatedConstRecordWithSumMaterialization,
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization,
    conventional_sum_layout_reports_match_for_replay, field_occurrence_matches, hash_byte,
    hash_bytes, hash_text, hash_u64, hash_value, layout_plan_reports_match_for_replay,
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

impl RecordSumPathsReplay for ConventionalRecursiveRecordSumPathsLayoutReport {
    fn matches_for_replay(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Leaf {
                    outer_layout: left_layout,
                    child_sum_layouts: left,
                },
                Self::Leaf {
                    outer_layout: right_layout,
                    child_sum_layouts: right,
                },
            ) => {
                layout_plan_reports_match_for_replay(left_layout, right_layout)
                    && left.len() == right.len()
                    && left.iter().zip(right).all(|(left, right)| {
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
            && self.child_sum_layouts.len() == other.child_sum_layouts.len()
            && self
                .child_sum_layouts
                .iter()
                .zip(&other.child_sum_layouts)
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

pub(super) fn record_sum_paths_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstRecursiveNestedSumOccurrenceMaterialization],
    nested_sums: &[ValidatedConstRecordSumFieldMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-recursive-record-sum-paths.v2",
    );
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
    hash_u64(&mut hash, nested_sums.len() as u64);
    for nested in nested_sums {
        match nested.field_identity {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => {
                hash_byte(&mut hash, 0);
                hash_text(&mut hash, &nested.field);
            }
        }
        hash_u64(
            &mut hash,
            nested
                .nested_sum
                .non_authoritative_layout_report_fingerprint(),
        );
        match nested.nested_sum.selected_case_identity() {
            Some(identity) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, identity);
            }
            None => hash_byte(&mut hash, 0),
        }
        hash_u64(
            &mut hash,
            u64::from(nested.nested_sum.selected_case_ordinal()),
        );
    }
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
