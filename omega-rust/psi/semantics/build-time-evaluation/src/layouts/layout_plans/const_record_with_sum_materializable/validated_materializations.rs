//! Validated const record materializations with sums, sum arrays and sum
//! array sets.

use crate::layouts::layout_plans::const_record_with_sum_materializable::derived_bytes::{
    derive_record_with_sum_array_bytes, derive_record_with_sum_arrays_bytes,
    derive_record_with_sum_bytes,
};
use crate::layouts::layout_plans::const_record_with_sum_materializable::outer_layouts::{
    nested_sum_fields_match, validate_supplied_nested_rows_against_retained,
};
use crate::layouts::layout_plans::const_record_with_sum_materializable::replay_matching::{
    sum_array_elements_match, sum_array_fields_match, sum_array_layout_sets_match_for_replay,
    sum_array_layouts_match_for_replay,
};
use crate::layouts::layout_plans::const_record_with_sum_materializable::report_fingerprints::{
    non_authoritative_record_with_sum_array_materialization_fingerprint,
    non_authoritative_record_with_sum_arrays_materialization_fingerprint,
    non_authoritative_record_with_sum_materialization_report_fingerprint,
};
use crate::layouts::layout_plans::{BuildTimeValue, ValidatedConstSumMaterialization};
use layout_plans::{
    ByteOrder, ConventionalSumArrayFieldLayoutReport, ConventionalSumFieldLayoutReport,
    ConventionalSumLayoutReport, LayoutPlanReport, MaterializationDiagnostic,
    layout_plan_reports_match_for_replay, normalized_layout_plan_report_fingerprint,
};
use typed_trees::TypedTrees;

#[derive(Debug)]
pub struct ValidatedConstRecordSumFieldMaterialization {
    pub(crate) field: String,
    pub(crate) field_identity: Option<u64>,
    pub(crate) nested_sum: ValidatedConstSumMaterialization,
}

/// Legacy singular value-sensitive custody for one literal element.
#[derive(Debug)]
pub struct ValidatedConstRecordSumArrayElementMaterialization {
    pub(crate) literal_index: u64,
    pub(crate) nested_sum: ValidatedConstSumMaterialization,
}

impl ValidatedConstRecordSumArrayElementMaterialization {
    pub const fn literal_index(&self) -> u64 {
        self.literal_index
    }

    pub const fn nested_sum(&self) -> &ValidatedConstSumMaterialization {
        &self.nested_sum
    }
}

/// Compact value-sensitive custody for one literal element of one occurrence
/// in the plural direct fixed-array-of-conventional-sums set. The complete
/// all-case layout is retained once in the occurrence's compact target row.
#[derive(Debug)]
pub struct ValidatedConstRecordSumArrayElementSelection {
    pub(crate) literal_index: u64,
    pub(crate) non_authoritative_schema_report_fingerprint: u64,
    pub(crate) value: BuildTimeValue,
    pub(crate) non_authoritative_layout_report_fingerprint: u64,
    pub(crate) selected_case_identity: Option<u64>,
    pub(crate) selected_case_ordinal: u32,
    pub(crate) bytes: Vec<u8>,
    pub(crate) non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordSumArrayElementSelection {
    pub const fn literal_index(&self) -> u64 {
        self.literal_index
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn non_authoritative_schema_report_fingerprint(&self) -> u64 {
        self.non_authoritative_schema_report_fingerprint
    }

    pub const fn non_authoritative_layout_report_fingerprint(&self) -> u64 {
        self.non_authoritative_layout_report_fingerprint
    }

    pub const fn selected_case_identity(&self) -> Option<u64> {
        self.selected_case_identity
    }

    pub const fn selected_case_ordinal(&self) -> u32 {
        self.selected_case_ordinal
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }
}

/// Exact materialization custody for one closed record containing exactly one
/// direct, nonzero literal fixed array of conventional pure sums.
#[derive(Debug)]
pub struct ValidatedConstRecordWithSumArrayMaterialization {
    pub(crate) schema_name: String,
    pub(crate) non_authoritative_schema_report_fingerprint: u64,
    pub(crate) value: BuildTimeValue,
    pub(crate) layout: LayoutPlanReport,
    pub(crate) non_authoritative_layout_report_fingerprint: u64,
    pub(crate) array_layout: ConventionalSumArrayFieldLayoutReport,
    pub(crate) elements: Vec<ValidatedConstRecordSumArrayElementMaterialization>,
    pub(crate) byte_order: ByteOrder,
    pub(crate) bytes: Vec<u8>,
    pub(crate) non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithSumArrayMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn layout(&self) -> &LayoutPlanReport {
        &self.layout
    }

    pub const fn array_layout(&self) -> &ConventionalSumArrayFieldLayoutReport {
        &self.array_layout
    }

    pub fn elements(&self) -> &[ValidatedConstRecordSumArrayElementMaterialization] {
        &self.elements
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Reconstruct the exact outer layout, compact array layout, every indexed
    /// selected sum, and the staged whole-record image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        layout: &LayoutPlanReport,
        array_layout: &ConventionalSumArrayFieldLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable sum-array record invocation drifted from retained custody"
                    .into(),
            ));
        }
        let layout_fingerprint = normalized_layout_plan_report_fingerprint(layout);
        if layout_fingerprint != self.non_authoritative_layout_report_fingerprint
            || !layout_plan_reports_match_for_replay(layout, &self.layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable sum-array outer layout drifted from retained custody".into(),
            ));
        }
        if !sum_array_layouts_match_for_replay(array_layout, &self.array_layout) {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable compact sum-array layout drifted from retained custody".into(),
            ));
        }
        let replayed = derive_record_with_sum_array_bytes(
            typed,
            schema_name,
            layout,
            array_layout,
            value,
            byte_order,
        )?;
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || !sum_array_elements_match(&replayed.elements, &self.elements)
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable indexed sum-array custody drifted from exact replay".into(),
            ));
        }
        let fingerprint = non_authoritative_record_with_sum_array_materialization_fingerprint(
            schema_name,
            replayed.schema_report_fingerprint,
            layout_fingerprint,
            array_layout,
            &replayed.elements,
            byte_order,
            value,
            &replayed.bytes,
        );
        if fingerprint != self.non_authoritative_materialization_report_fingerprint {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable sum-array report fingerprint drifted from exact replay".into(),
            ));
        }
        Ok(())
    }

    /// Replay before atomically copying the complete outer record image.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        self.replay_against(
            typed,
            &self.schema_name,
            &self.layout,
            &self.array_layout,
            &self.value,
            self.byte_order,
        )?;
        if destination.len() < self.bytes.len() {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable sum-array record copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

/// Value-sensitive custody for one direct fixed-array-of-sums field in the
/// complete authored-order occurrence set.
#[derive(Debug)]
pub struct ValidatedConstRecordSumArrayFieldMaterialization {
    pub(crate) field: String,
    pub(crate) field_identity: Option<u64>,
    pub(crate) elements: Vec<ValidatedConstRecordSumArrayElementSelection>,
}

impl ValidatedConstRecordSumArrayFieldMaterialization {
    pub fn field(&self) -> &str {
        &self.field
    }

    pub const fn field_identity(&self) -> Option<u64> {
        self.field_identity
    }

    pub fn elements(&self) -> &[ValidatedConstRecordSumArrayElementSelection] {
        &self.elements
    }
}

/// Exact custody for the complete nonempty authored-order set of direct
/// nonzero literal fixed arrays of conventional pure sums.
#[derive(Debug)]
pub struct ValidatedConstRecordWithSumArraysMaterialization {
    pub(crate) schema_name: String,
    pub(crate) non_authoritative_schema_report_fingerprint: u64,
    pub(crate) value: BuildTimeValue,
    pub(crate) layout: LayoutPlanReport,
    pub(crate) non_authoritative_layout_report_fingerprint: u64,
    pub(crate) array_layouts: Vec<ConventionalSumArrayFieldLayoutReport>,
    pub(crate) arrays: Vec<ValidatedConstRecordSumArrayFieldMaterialization>,
    pub(crate) byte_order: ByteOrder,
    pub(crate) bytes: Vec<u8>,
    pub(crate) non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithSumArraysMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn layout(&self) -> &LayoutPlanReport {
        &self.layout
    }

    pub fn array_layouts(&self) -> &[ConventionalSumArrayFieldLayoutReport] {
        &self.array_layouts
    }

    pub fn arrays(&self) -> &[ValidatedConstRecordSumArrayFieldMaterialization] {
        &self.arrays
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        layout: &LayoutPlanReport,
        array_layouts: &[ConventionalSumArrayFieldLayoutReport],
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable sum-array fields invocation drifted from retained custody"
                    .into(),
            ));
        }
        let layout_fingerprint = normalized_layout_plan_report_fingerprint(layout);
        if layout_fingerprint != self.non_authoritative_layout_report_fingerprint
            || !layout_plan_reports_match_for_replay(layout, &self.layout)
            || !sum_array_layout_sets_match_for_replay(array_layouts, &self.array_layouts)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable sum-array field layout set drifted from retained custody"
                    .into(),
            ));
        }
        let replayed = derive_record_with_sum_arrays_bytes(
            typed,
            schema_name,
            layout,
            array_layouts,
            value,
            byte_order,
        )?;
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || !sum_array_fields_match(&replayed.arrays, &self.arrays)
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable sum-array field custody drifted from exact replay".into(),
            ));
        }
        let fingerprint = non_authoritative_record_with_sum_arrays_materialization_fingerprint(
            schema_name,
            replayed.schema_report_fingerprint,
            layout_fingerprint,
            array_layouts,
            &replayed.arrays,
            byte_order,
            value,
            &replayed.bytes,
        );
        if fingerprint != self.non_authoritative_materialization_report_fingerprint {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable sum-array fields report fingerprint drifted after replay"
                    .into(),
            ));
        }
        Ok(())
    }

    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        self.replay_against(
            typed,
            &self.schema_name,
            &self.layout,
            &self.array_layouts,
            &self.value,
            self.byte_order,
        )?;
        if destination.len() < self.bytes.len() {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable sum-array fields copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

impl ValidatedConstRecordSumFieldMaterialization {
    pub fn field(&self) -> &str {
        &self.field
    }

    pub const fn field_identity(&self) -> Option<u64> {
        self.field_identity
    }

    /// Complete retained conventional layout, selected case, value, byte, and
    /// report custody for this direct sum field.
    pub const fn nested_sum(&self) -> &ValidatedConstSumMaterialization {
        &self.nested_sum
    }
}

/// Exact materialization custody for one closed record containing one or more
/// direct, runtime-relevant conventional case-bearing fields — pure sums or
/// mixed common-field/case shapes.
///
/// This deliberately distinct carrier keeps arrays of sums, recursively nested
/// sums, and target-dependent sum geometry outside the first
/// nested-sum rung. It does not implement `Clone`: replay reconstructs every
/// outer and nested fact from the caller's current typed program.
#[derive(Debug)]
pub struct ValidatedConstRecordWithSumMaterialization {
    pub(crate) schema_name: String,
    pub(crate) non_authoritative_schema_report_fingerprint: u64,
    pub(crate) value: BuildTimeValue,
    pub(crate) layout: LayoutPlanReport,
    pub(crate) non_authoritative_layout_report_fingerprint: u64,
    pub(crate) nested_sums: Vec<ValidatedConstRecordSumFieldMaterialization>,
    pub(crate) byte_order: ByteOrder,
    pub(crate) bytes: Vec<u8>,
    pub(crate) non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithSumMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn non_authoritative_schema_report_fingerprint(&self) -> u64 {
        self.non_authoritative_schema_report_fingerprint
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn layout(&self) -> &LayoutPlanReport {
        &self.layout
    }

    pub const fn non_authoritative_layout_report_fingerprint(&self) -> u64 {
        self.non_authoritative_layout_report_fingerprint
    }

    /// Complete authored-order custody for every direct sum field.
    pub fn nested_sums(&self) -> &[ValidatedConstRecordSumFieldMaterialization] {
        &self.nested_sums
    }

    /// Compatibility accessor for the singular API. Generalized consumers
    /// should use [`Self::nested_sums`].
    pub fn nested_sum_field(&self) -> &str {
        self.nested_sums[0].field()
    }

    /// Compatibility accessor for the singular API.
    pub fn nested_sum_field_identity(&self) -> Option<u64> {
        self.nested_sums[0].field_identity()
    }

    /// Compatibility accessor for the singular API.
    pub fn nested_sum(&self) -> &ValidatedConstSumMaterialization {
        self.nested_sums[0].nested_sum()
    }

    pub const fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Independently replay the outer layout and the complete caller-supplied
    /// conventional nested-sum layout. Compact report fingerprints are checked
    /// only after exact hash-free layout comparisons.
    pub fn replay_against_sum_fields(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        layout: &LayoutPlanReport,
        nested_sum_layouts: &[ConventionalSumFieldLayoutReport],
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable nested-sum record schema `{schema_name}` does not match retained schema `{}`",
                self.schema_name
            )));
        }
        if value != &self.value {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-sum record value drifted from retained custody".into(),
            ));
        }
        if byte_order != self.byte_order {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-sum record target byte order drifted from retained custody"
                    .into(),
            ));
        }
        let layout_report_fingerprint = normalized_layout_plan_report_fingerprint(layout);
        if layout_report_fingerprint != self.non_authoritative_layout_report_fingerprint
            || !layout_plan_reports_match_for_replay(layout, &self.layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-sum outer layout drifted from retained custody".into(),
            ));
        }
        validate_supplied_nested_rows_against_retained(
            typed,
            nested_sum_layouts,
            &self.nested_sums,
            byte_order,
        )?;

        let replayed = derive_record_with_sum_bytes(
            typed,
            schema_name,
            layout,
            nested_sum_layouts,
            value,
            byte_order,
        )?;
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || !nested_sum_fields_match(&replayed.nested_sums, &self.nested_sums)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-sum field identity drifted from retained custody"
                    .into(),
            ));
        }
        if replayed.bytes != self.bytes {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-sum record bytes drifted from exact zero-initialized replay"
                    .into(),
            ));
        }
        let materialization_report_fingerprint =
            non_authoritative_record_with_sum_materialization_report_fingerprint(
                schema_name,
                replayed.schema_report_fingerprint,
                layout_report_fingerprint,
                &replayed.nested_sums,
                byte_order,
                value,
                &replayed.bytes,
            );
        if materialization_report_fingerprint
            != self.non_authoritative_materialization_report_fingerprint
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-sum record report fingerprint drifted from exact replay"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Replay the original singular-row surface without weakening plural
    /// custody. Records with more than one direct sum reject this wrapper.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        layout: &LayoutPlanReport,
        nested_sum_layout: &ConventionalSumLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        let [nested] = self.nested_sums.as_slice() else {
            return Err(MaterializationDiagnostic(
                "singular nested-sum replay cannot discard multiple retained field rows".into(),
            ));
        };
        self.replay_against_sum_fields(
            typed,
            schema_name,
            layout,
            &[ConventionalSumFieldLayoutReport {
                field: nested.field.clone(),
                member_identity: nested.field_identity,
                layout: nested_sum_layout.clone(),
            }],
            value,
            byte_order,
        )
    }

    /// Replay both retained layouts before atomically copying the exact outer
    /// bytes. Rejection and a short destination leave `destination` unchanged.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        self.replay_against_sum_fields(
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
            &self.value,
            self.byte_order,
        )?;
        if destination.len() < self.bytes.len() {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable nested-sum record copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}
