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
#[cfg(test)]
mod tests;
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
    ByteOrder, ConventionalSumArrayFieldLayoutReport, ConventionalSumFieldLayoutReport,
    ConventionalSumLayoutReport, LayoutPlanReport, MaterializationDiagnostic,
    normalized_layout_plan_report_fingerprint,
};
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember, DataShapeKind};

use super::BuildTimeValue;
use super::const_materializable::unique_data_by_name;
use crate::layouts::layout_plans::const_record_with_sum_materializable::derived_bytes::{
    derive_record_with_sum_array_bytes, derive_record_with_sum_arrays_bytes,
    derive_record_with_sum_bytes,
};
use crate::layouts::layout_plans::const_record_with_sum_materializable::report_fingerprints::{
    non_authoritative_record_with_sum_array_materialization_fingerprint,
    non_authoritative_record_with_sum_arrays_materialization_fingerprint,
    non_authoritative_record_with_sum_materialization_report_fingerprint,
};

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

/// Preserve the original singular projection surface. It remains fail-closed
/// when the record now requires more than one direct sum-field row.
pub fn validate_const_materializable_record_with_conventional_sum(
    typed: &TypedTrees,
    schema_name: &str,
    layout: &LayoutPlanReport,
    nested_sum_layout: &ConventionalSumLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithSumMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_outer_record_owner(typed, data)?;
    let direct_sums = typed
        .data_members(data)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) if !field.relevance.is_erased() => Some(field),
            DataMember::Field(_) | DataMember::Variant(_) => None,
        })
        .filter_map(
            |field| match exact_named_data(typed, field.type_reference) {
                Ok(Some(named))
                    if matches!(
                        DataDefinition::shape_kind_from_members(typed.data_members(named)),
                        DataShapeKind::Enum | DataShapeKind::Mixed
                    ) =>
                {
                    Some(Ok(field))
                }
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    if direct_sums.is_empty() {
        return validate_const_materializable_record_with_conventional_sums(
            typed,
            schema_name,
            layout,
            &[],
            value,
            byte_order,
        );
    }
    let [field] = direct_sums.as_slice() else {
        return Err(MaterializationDiagnostic(format!(
            "singular nested-sum validation requires exactly one direct runtime-relevant pure-sum field; `{schema_name}` has {}",
            direct_sums.len()
        )));
    };
    validate_const_materializable_record_with_conventional_sums(
        typed,
        schema_name,
        layout,
        &[ConventionalSumFieldLayoutReport {
            field: field.name.to_string(),
            member_identity: field.identity,
            layout: nested_sum_layout.clone(),
        }],
        value,
        byte_order,
    )
}

/// Validate the compact layout of the sole direct fixed-array-of-sums field,
/// retain each selected element independently, and stage one complete outer
/// record image.
pub fn validate_const_materializable_record_with_conventional_sum_array(
    typed: &TypedTrees,
    schema_name: &str,
    layout: &LayoutPlanReport,
    array_layout: &ConventionalSumArrayFieldLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithSumArrayMaterialization, MaterializationDiagnostic> {
    let derived = derive_record_with_sum_array_bytes(
        typed,
        schema_name,
        layout,
        array_layout,
        value,
        byte_order,
    )?;
    let layout_report_fingerprint = normalized_layout_plan_report_fingerprint(layout);
    let materialization_report_fingerprint =
        non_authoritative_record_with_sum_array_materialization_fingerprint(
            schema_name,
            derived.schema_report_fingerprint,
            layout_report_fingerprint,
            array_layout,
            &derived.elements,
            byte_order,
            value,
            &derived.bytes,
        );
    Ok(ValidatedConstRecordWithSumArrayMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        layout: layout.clone(),
        non_authoritative_layout_report_fingerprint: layout_report_fingerprint,
        array_layout: array_layout.clone(),
        elements: derived.elements,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_report_fingerprint,
    })
}

pub fn validate_const_materializable_record_with_conventional_sum_arrays(
    typed: &TypedTrees,
    schema_name: &str,
    layout: &LayoutPlanReport,
    array_layouts: &[ConventionalSumArrayFieldLayoutReport],
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithSumArraysMaterialization, MaterializationDiagnostic> {
    let derived = derive_record_with_sum_arrays_bytes(
        typed,
        schema_name,
        layout,
        array_layouts,
        value,
        byte_order,
    )?;
    let layout_report_fingerprint = normalized_layout_plan_report_fingerprint(layout);
    let materialization_report_fingerprint =
        non_authoritative_record_with_sum_arrays_materialization_fingerprint(
            schema_name,
            derived.schema_report_fingerprint,
            layout_report_fingerprint,
            array_layouts,
            &derived.arrays,
            byte_order,
            value,
            &derived.bytes,
        );
    let mut retained_array_layouts = Vec::new();
    retained_array_layouts
        .try_reserve_exact(array_layouts.len())
        .map_err(|_| {
            MaterializationDiagnostic(
                "ConstMaterializable sum-array retained layout set exceeds compiler resources"
                    .into(),
            )
        })?;
    retained_array_layouts.extend(array_layouts.iter().cloned());
    Ok(ValidatedConstRecordWithSumArraysMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        layout: layout.clone(),
        non_authoritative_layout_report_fingerprint: layout_report_fingerprint,
        array_layouts: retained_array_layouts,
        arrays: derived.arrays,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_report_fingerprint,
    })
}
