//! One recursive record level's retained direct-child custody.
//!
//! A record level that ends the record recursion still carries both direct
//! child kinds: its direct conventional pure-sum fields and its direct fixed
//! arrays of conventional pure sums. The level's custody mirrors the branch
//! custody — the exact supplied report rows are retained beside the validated
//! per-field selections so replay compares hash-free layout facts before any
//! fingerprint coordinate.

use super::{
    BuildTimeValue, ByteOrder, MaterializationDiagnostic, SumReachability, TypedTrees,
    ValidatedConstRecordSumArrayFieldMaterialization, ValidatedConstRecordSumFieldMaterialization,
    derive_record_level_children_bytes, layout_plan_reports_match_for_replay,
    nested_sum_fields_match, normalized_layout_plan_report_fingerprint,
    record_level_materialization_report_fingerprint, sum_array_fields_match,
    sum_array_layout_sets_match_for_replay, validate_supplied_nested_rows_against_retained,
};
use layout_plans::{
    ConventionalSumArrayFieldLayoutReport, ConventionalSumFieldLayoutReport, LayoutPlanReport,
};

/// Complete value-sensitive custody for one recursive record level that ends
/// the record recursion: every direct conventional pure-sum field and every
/// direct fixed array of pure sums beside the level's flat outer plan.
///
/// `array_layouts` retains the supplied compact rows because each retained
/// element selection is deliberately compact — the complete all-case element
/// layout lives once per occurrence here, so replay can compare supplied rows
/// hash-free before re-deriving any selected case. This type does not
/// implement `Clone`: replay reconstructs every outer and nested fact from
/// the caller's current typed program.
#[derive(Debug)]
pub struct ValidatedConstRecordLevelSumChildrenMaterialization {
    pub(crate) schema_name: String,
    pub(crate) non_authoritative_schema_report_fingerprint: u64,
    pub(crate) value: BuildTimeValue,
    pub(crate) layout: LayoutPlanReport,
    pub(crate) non_authoritative_layout_report_fingerprint: u64,
    pub(crate) nested_sums: Vec<ValidatedConstRecordSumFieldMaterialization>,
    pub(crate) array_layouts: Vec<ConventionalSumArrayFieldLayoutReport>,
    pub(crate) nested_sum_arrays: Vec<ValidatedConstRecordSumArrayFieldMaterialization>,
    pub(crate) byte_order: ByteOrder,
    pub(crate) bytes: Vec<u8>,
    pub(crate) non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordLevelSumChildrenMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn layout(&self) -> &LayoutPlanReport {
        &self.layout
    }

    /// Direct conventional sum custody at this level, in authored field order.
    pub fn nested_sums(&self) -> &[ValidatedConstRecordSumFieldMaterialization] {
        &self.nested_sums
    }

    /// Direct fixed-array-of-sums custody at this level, in authored field
    /// order — each retained element selection keeps its own literal index.
    pub fn nested_sum_arrays(&self) -> &[ValidatedConstRecordSumArrayFieldMaterialization] {
        &self.nested_sum_arrays
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Independently rederive the level's typed value, both supplied direct
    /// child row sets, and the complete staged bytes.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        layout: &LayoutPlanReport,
        child_sum_layouts: &[ConventionalSumFieldLayoutReport],
        child_sum_array_layouts: &[ConventionalSumArrayFieldLayoutReport],
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        let mut reachability = SumReachability::new(typed);
        self.replay_against_with_reachability(
            typed,
            schema_name,
            layout,
            child_sum_layouts,
            child_sum_array_layouts,
            value,
            byte_order,
            &mut reachability,
        )
    }

    pub(super) fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        layout: &LayoutPlanReport,
        child_sum_layouts: &[ConventionalSumFieldLayoutReport],
        child_sum_array_layouts: &[ConventionalSumArrayFieldLayoutReport],
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable record-level invocation drifted from retained custody".into(),
            ));
        }
        let layout_fingerprint = normalized_layout_plan_report_fingerprint(layout);
        if layout_fingerprint != self.non_authoritative_layout_report_fingerprint
            || !layout_plan_reports_match_for_replay(layout, &self.layout)
            || !sum_array_layout_sets_match_for_replay(child_sum_array_layouts, &self.array_layouts)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable record-level layout set drifted from retained custody".into(),
            ));
        }
        validate_supplied_nested_rows_against_retained(
            typed,
            child_sum_layouts,
            &self.nested_sums,
            byte_order,
        )?;

        let replayed = derive_record_level_children_bytes(
            typed,
            schema_name,
            layout,
            child_sum_layouts,
            child_sum_array_layouts,
            value,
            byte_order,
            reachability,
        )?;
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || !nested_sum_fields_match(&replayed.nested_sums, &self.nested_sums)
            || !sum_array_fields_match(&replayed.nested_sum_arrays, &self.nested_sum_arrays)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable record-level direct-child custody drifted from exact replay"
                    .into(),
            ));
        }
        if replayed.bytes != self.bytes {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable record-level bytes drifted from exact replay".into(),
            ));
        }
        let fingerprint = record_level_materialization_report_fingerprint(
            schema_name,
            replayed.schema_report_fingerprint,
            layout_fingerprint,
            child_sum_array_layouts,
            &replayed.nested_sums,
            &replayed.nested_sum_arrays,
            byte_order,
            value,
            &replayed.bytes,
        );
        if fingerprint != self.non_authoritative_materialization_report_fingerprint {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable record-level report fingerprint drifted from exact replay"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Replay both retained child channels before one atomic copy of the
    /// level's complete image.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        self.replay_against(
            typed,
            &self.schema_name,
            &self.layout,
            &self
                .nested_sums
                .iter()
                .map(|row| ConventionalSumFieldLayoutReport {
                    field: row.field.clone(),
                    member_identity: row.field_identity,
                    layout: row.nested_sum.layout().clone(),
                })
                .collect::<Vec<_>>(),
            &self.array_layouts,
            &self.value,
            self.byte_order,
        )?;
        if destination.len() < self.bytes.len() {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable record-level copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

/// Validate one record level's complete direct-sum children and retain the
/// exact supplied rows beside each field's value-sensitive custody.
pub(super) fn validate_record_level_sum_children_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    outer_layout: &LayoutPlanReport,
    child_sum_layouts: &[ConventionalSumFieldLayoutReport],
    child_sum_array_layouts: &[ConventionalSumArrayFieldLayoutReport],
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordLevelSumChildrenMaterialization, MaterializationDiagnostic> {
    let derived = derive_record_level_children_bytes(
        typed,
        schema_name,
        outer_layout,
        child_sum_layouts,
        child_sum_array_layouts,
        value,
        byte_order,
        reachability,
    )?;
    let layout_fingerprint = normalized_layout_plan_report_fingerprint(outer_layout);
    let materialization_fingerprint = record_level_materialization_report_fingerprint(
        schema_name,
        derived.schema_report_fingerprint,
        layout_fingerprint,
        child_sum_array_layouts,
        &derived.nested_sums,
        &derived.nested_sum_arrays,
        byte_order,
        value,
        &derived.bytes,
    );
    Ok(ValidatedConstRecordLevelSumChildrenMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        layout: outer_layout.clone(),
        non_authoritative_layout_report_fingerprint: layout_fingerprint,
        nested_sums: derived.nested_sums,
        array_layouts: child_sum_array_layouts.to_vec(),
        nested_sum_arrays: derived.nested_sum_arrays,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_fingerprint,
    })
}
