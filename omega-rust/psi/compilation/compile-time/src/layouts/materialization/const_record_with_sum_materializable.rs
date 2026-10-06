//! Value-sensitive materialization of one record with direct conventional sum fields.
//!
//! This file validates const-materializable records with conventional
//! sums, sum arrays and sum array sets. `validated_materializations.rs`
//! carries the validated materializations, `derived_bytes.rs` derives
//! their bytes, `outer_layouts.rs` validates outer record owners and
//! layouts, `replay_matching.rs` matches retained rows on replay,
//! `report_fingerprints.rs` hashes occurrences into report-only
//! fingerprints and `tests.rs` holds the materialization tests.

mod derived_bytes;
mod outer_layouts;
mod replay_matching;
mod report_fingerprints;
mod validated_materializations;

pub(crate) use derived_bytes::{EncodedOuterField, prepare_sum_array_field};
pub(crate) use outer_layouts::{
    exact_named_data, field_occurrence_matches, flatten_literal_array_elements,
    validate_outer_layout, validate_outer_record_owner,
};
pub use validated_materializations::{
    ValidatedConstRecordSumArrayElementMaterialization,
    ValidatedConstRecordSumArrayElementSelection, ValidatedConstRecordSumArrayFieldMaterialization,
    ValidatedConstRecordSumFieldMaterialization, ValidatedConstRecordWithSumArrayMaterialization,
    ValidatedConstRecordWithSumArraysMaterialization, ValidatedConstRecordWithSumMaterialization,
};

use layout_plans::{
    ByteOrder, ConventionalSumFieldLayoutReport, LayoutPlanReport, MaterializationDiagnostic,
    normalized_layout_plan_report_fingerprint,
};
use typed_trees::TypedTrees;

use super::BuildTimeValue;
use crate::layouts::materialization::const_record_with_sum_materializable::derived_bytes::derive_record_with_sum_bytes;
use crate::layouts::materialization::const_record_with_sum_materializable::report_fingerprints::non_authoritative_record_with_sum_materialization_report_fingerprint;

/// Exact materialization custody for one direct runtime-relevant conventional
/// pure-sum field of a closed record.
/// Validate every direct conventional pure-sum field inside one closed
/// non-generic `[copy]` record. The outer record uses its exact validated
/// `LayoutPlanReport`; each nested sum uses the compiler-owned conventional
/// runtime layout and cannot acquire programmable tag/case placement. The
/// supplied rows must be the complete authored-order direct-sum field set.
pub fn validate_const_materializable_record_with_conventional_sums(
    typed: &TypedTrees,
    schema_name: &str,
    layout: &LayoutPlanReport,
    nested_sum_layouts: &[ConventionalSumFieldLayoutReport],
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithSumMaterialization, MaterializationDiagnostic> {
    let derived = derive_record_with_sum_bytes(
        typed,
        schema_name,
        layout,
        nested_sum_layouts,
        value,
        byte_order,
    )?;
    let layout_report_fingerprint = normalized_layout_plan_report_fingerprint(layout);
    let materialization_report_fingerprint =
        non_authoritative_record_with_sum_materialization_report_fingerprint(
            schema_name,
            derived.schema_report_fingerprint,
            layout_report_fingerprint,
            &derived.nested_sums,
            byte_order,
            value,
            &derived.bytes,
        );
    Ok(ValidatedConstRecordWithSumMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        layout: layout.clone(),
        non_authoritative_layout_report_fingerprint: layout_report_fingerprint,
        nested_sums: derived.nested_sums,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_report_fingerprint,
    })
}
