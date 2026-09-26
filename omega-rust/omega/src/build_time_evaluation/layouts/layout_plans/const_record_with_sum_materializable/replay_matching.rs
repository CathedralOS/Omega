//! Matching sum array layouts, elements, selections and fields on replay.

use crate::build_time_evaluation::layouts::layout_plans::const_record_with_sum_materializable::outer_layouts::field_occurrence_matches;
use crate::build_time_evaluation::layouts::layout_plans::const_record_with_sum_materializable::{
    ValidatedConstRecordSumArrayElementMaterialization,
    ValidatedConstRecordSumArrayElementSelection, ValidatedConstRecordSumArrayFieldMaterialization,
};
use terminal_psi::layout_plans::{
    ConventionalSumArrayFieldLayoutReport, conventional_sum_layout_reports_match_for_replay,
    normalized_conventional_sum_layout_report_fingerprint,
};

pub(crate) fn sum_array_layouts_match_for_replay(
    left: &ConventionalSumArrayFieldLayoutReport,
    right: &ConventionalSumArrayFieldLayoutReport,
) -> bool {
    field_occurrence_matches(
        &left.field,
        left.member_identity,
        &right.field,
        right.member_identity,
    ) && left.element_count == right.element_count
        && left.element_stride == right.element_stride
        && normalized_conventional_sum_layout_report_fingerprint(&left.element_layout)
            == normalized_conventional_sum_layout_report_fingerprint(&right.element_layout)
        && conventional_sum_layout_reports_match_for_replay(
            &left.element_layout,
            &right.element_layout,
        )
}

pub(crate) fn sum_array_elements_match(
    left: &[ValidatedConstRecordSumArrayElementMaterialization],
    right: &[ValidatedConstRecordSumArrayElementMaterialization],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.literal_index == right.literal_index
                && left.nested_sum.schema_name() == right.nested_sum.schema_name()
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
        })
}

fn sum_array_selections_match(
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

pub(crate) fn sum_array_layout_sets_match_for_replay(
    left: &[ConventionalSumArrayFieldLayoutReport],
    right: &[ConventionalSumArrayFieldLayoutReport],
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| sum_array_layouts_match_for_replay(left, right))
}

pub(crate) fn sum_array_fields_match(
    left: &[ValidatedConstRecordSumArrayFieldMaterialization],
    right: &[ValidatedConstRecordSumArrayFieldMaterialization],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            field_occurrence_matches(
                &left.field,
                left.field_identity,
                &right.field,
                right.field_identity,
            ) && sum_array_selections_match(&left.elements, &right.elements)
        })
}
