//! Hash-free replay equality and non-authoritative report fingerprints.

use super::*;

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

pub(super) fn depth_two_path_reports_match_for_replay(
    left: &ConventionalDepthTwoRecordSumPathLayoutReport,
    right: &ConventionalDepthTwoRecordSumPathLayoutReport,
) -> bool {
    layout_plan_reports_match_for_replay(&left.outer_layout, &right.outer_layout)
        && field_occurrence_matches(
            &left.outer_field,
            left.outer_member_identity,
            &right.outer_field,
            right.outer_member_identity,
        )
        && nested_path_reports_match_for_replay(&left.middle_path, &right.middle_path)
}

pub(super) fn depth_three_path_reports_match_for_replay(
    left: &ConventionalDepthThreeRecordSumPathLayoutReport,
    right: &ConventionalDepthThreeRecordSumPathLayoutReport,
) -> bool {
    layout_plan_reports_match_for_replay(&left.outer_layout, &right.outer_layout)
        && field_occurrence_matches(
            &left.outer_field,
            left.outer_member_identity,
            &right.outer_field,
            right.outer_member_identity,
        )
        && depth_two_path_reports_match_for_replay(&left.depth_two_path, &right.depth_two_path)
}

pub(super) trait RecordSumPathsReplay {
    fn matches_for_replay(&self, other: &Self) -> bool;
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

impl<InnerPaths: RecordSumPathsReplay> RecordSumPathsReplay
    for ConventionalRecordSumPathsLayoutReport<InnerPaths>
{
    fn matches_for_replay(&self, other: &Self) -> bool {
        layout_plan_reports_match_for_replay(&self.outer_layout, &other.outer_layout)
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

pub(super) trait RecordSumPathsInnerLayout {
    fn outer_layout(&self) -> &psi_layout_plans::LayoutPlanReport;
}

impl RecordSumPathsInnerLayout for ConventionalNestedRecordSumPathsLayoutReport {
    fn outer_layout(&self) -> &psi_layout_plans::LayoutPlanReport {
        &self.outer_layout
    }
}

impl<InnerPaths> RecordSumPathsInnerLayout for ConventionalRecordSumPathsLayoutReport<InnerPaths> {
    fn outer_layout(&self) -> &psi_layout_plans::LayoutPlanReport {
        &self.outer_layout
    }
}

pub(super) fn record_sum_paths_materialization_report_fingerprint<
    InnerPaths: RecordSumPathsInnerLayout,
    Occurrence,
>(
    domain: &[u8],
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalRecordSumPathsLayoutReport<InnerPaths>,
    occurrences: &[Occurrence],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
    inner_fingerprint: impl Fn(&Occurrence) -> u64,
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(&mut hash, domain);
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_report_fingerprint);
    hash_u64(&mut hash, outer_layout_report_fingerprint);
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
        hash_u64(&mut hash, inner_fingerprint(occurrence));
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

pub(super) fn depth_twelve_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthTwelveRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthTwelveNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    record_sum_paths_materialization_report_fingerprint(
        b"omega.const-materializable-plural-depth-twelve-record-sum-paths.v1",
        schema_name,
        schema_report_fingerprint,
        outer_layout_report_fingerprint,
        path_layout,
        occurrences,
        byte_order,
        value,
        bytes,
        |occurrence| {
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint()
        },
    )
}

pub(super) fn depth_thirteen_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthThirteenRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthThirteenNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    record_sum_paths_materialization_report_fingerprint(
        b"omega.const-materializable-plural-depth-thirteen-record-sum-paths.v1",
        schema_name,
        schema_report_fingerprint,
        outer_layout_report_fingerprint,
        path_layout,
        occurrences,
        byte_order,
        value,
        bytes,
        |occurrence| {
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint()
        },
    )
}

pub(super) fn depth_fourteen_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthFourteenRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthFourteenNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    record_sum_paths_materialization_report_fingerprint(
        b"omega.const-materializable-plural-depth-fourteen-record-sum-paths.v1",
        schema_name,
        schema_report_fingerprint,
        outer_layout_report_fingerprint,
        path_layout,
        occurrences,
        byte_order,
        value,
        bytes,
        |occurrence| {
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint()
        },
    )
}
pub(super) fn depth_fifteen_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthFifteenRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthFifteenNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    record_sum_paths_materialization_report_fingerprint(
        b"omega.const-materializable-plural-depth-fifteen-record-sum-paths.v1",
        schema_name,
        schema_report_fingerprint,
        outer_layout_report_fingerprint,
        path_layout,
        occurrences,
        byte_order,
        value,
        bytes,
        |occurrence| {
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint()
        },
    )
}

pub(super) fn depth_eleven_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthElevenRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthElevenNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    record_sum_paths_materialization_report_fingerprint(
        b"omega.const-materializable-plural-depth-eleven-record-sum-paths.v1",
        schema_name,
        schema_report_fingerprint,
        outer_layout_report_fingerprint,
        path_layout,
        occurrences,
        byte_order,
        value,
        bytes,
        |occurrence| {
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint()
        },
    )
}

pub(super) fn depth_ten_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthTenRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthTenNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    record_sum_paths_materialization_report_fingerprint(
        b"omega.const-materializable-plural-depth-ten-record-sum-paths.v1",
        schema_name,
        schema_report_fingerprint,
        outer_layout_report_fingerprint,
        path_layout,
        occurrences,
        byte_order,
        value,
        bytes,
        |occurrence| {
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint()
        },
    )
}

pub(super) fn depth_nine_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthNineRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthNineNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    record_sum_paths_materialization_report_fingerprint(
        b"omega.const-materializable-plural-depth-nine-record-sum-paths.v1",
        schema_name,
        schema_report_fingerprint,
        outer_layout_report_fingerprint,
        path_layout,
        occurrences,
        byte_order,
        value,
        bytes,
        |occurrence| {
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint()
        },
    )
}

pub(super) fn depth_eight_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthEightRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthEightNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    record_sum_paths_materialization_report_fingerprint(
        b"omega.const-materializable-plural-depth-eight-record-sum-paths.v1",
        schema_name,
        schema_report_fingerprint,
        outer_layout_report_fingerprint,
        path_layout,
        occurrences,
        byte_order,
        value,
        bytes,
        |occurrence| {
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint()
        },
    )
}

pub(super) fn depth_seven_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthSevenRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthSevenNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    record_sum_paths_materialization_report_fingerprint(
        b"omega.const-materializable-plural-depth-seven-record-sum-paths.v1",
        schema_name,
        schema_report_fingerprint,
        outer_layout_report_fingerprint,
        path_layout,
        occurrences,
        byte_order,
        value,
        bytes,
        |occurrence| {
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint()
        },
    )
}

pub(super) fn depth_six_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthSixRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthSixNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    record_sum_paths_materialization_report_fingerprint(
        b"omega.const-materializable-plural-depth-six-record-sum-paths.v1",
        schema_name,
        schema_report_fingerprint,
        outer_layout_report_fingerprint,
        path_layout,
        occurrences,
        byte_order,
        value,
        bytes,
        |occurrence| {
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint()
        },
    )
}

pub(super) fn depth_five_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthFiveRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthFiveNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    record_sum_paths_materialization_report_fingerprint(
        b"omega.const-materializable-plural-depth-five-record-sum-paths.v1",
        schema_name,
        schema_report_fingerprint,
        outer_layout_report_fingerprint,
        path_layout,
        occurrences,
        byte_order,
        value,
        bytes,
        |occurrence| {
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint()
        },
    )
}

pub(super) fn depth_four_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthFourRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthFourNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    record_sum_paths_materialization_report_fingerprint(
        b"omega.const-materializable-plural-depth-four-record-sum-paths.v1",
        schema_name,
        schema_report_fingerprint,
        outer_layout_report_fingerprint,
        path_layout,
        occurrences,
        byte_order,
        value,
        bytes,
        |occurrence| {
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint()
        },
    )
}

pub(super) fn depth_three_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthThreeRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthThreeNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    record_sum_paths_materialization_report_fingerprint(
        b"omega.const-materializable-plural-depth-three-record-sum-paths.v1",
        schema_name,
        schema_report_fingerprint,
        outer_layout_report_fingerprint,
        path_layout,
        occurrences,
        byte_order,
        value,
        bytes,
        |occurrence| {
            occurrence
                .inner
                .non_authoritative_materialization_report_fingerprint()
        },
    )
}

pub(super) fn depth_three_nested_sum_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthThreeRecordSumPathLayoutReport,
    inner: &ValidatedConstRecordWithDepthTwoNestedSumMaterialization,
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-depth-three-record-sum-path.v1",
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
        normalized_layout_plan_report_fingerprint(&path_layout.depth_two_path.outer_layout),
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

pub(super) fn depth_two_nested_sum_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthTwoRecordSumPathLayoutReport,
    middle: &ValidatedConstRecordWithNestedSumRecordMaterialization,
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(
        &mut hash,
        b"omega.const-materializable-depth-two-record-sum-path.v1",
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
        normalized_layout_plan_report_fingerprint(&path_layout.middle_path.outer_layout),
    );
    hash_u64(
        &mut hash,
        middle.non_authoritative_materialization_report_fingerprint(),
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

pub(super) fn depth_two_nested_sums_materialization_report_fingerprint(
    schema_name: &str,
    schema_report_fingerprint: u64,
    outer_layout_report_fingerprint: u64,
    path_layout: &ConventionalDepthTwoRecordSumPathsLayoutReport,
    occurrences: &[ValidatedConstDepthTwoNestedSumOccurrenceMaterialization],
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    record_sum_paths_materialization_report_fingerprint(
        b"omega.const-materializable-plural-depth-two-record-sum-paths.v1",
        schema_name,
        schema_report_fingerprint,
        outer_layout_report_fingerprint,
        path_layout,
        occurrences,
        byte_order,
        value,
        bytes,
        |occurrence| {
            occurrence
                .middle
                .non_authoritative_materialization_report_fingerprint()
        },
    )
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
