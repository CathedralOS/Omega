//! Value-sensitive materialization of record fields containing records with
//! direct conventional pure-sum fields. A recursive record level may also
//! hold its own direct sum fields, direct sum arrays, and direct record
//! arrays beside its deeper record paths; every child kind retains
//! independent custody under the same bounded traversal.

use layout_plans::{
    AggregateFieldSchema, AggregateFieldValue, ByteOrder,
    ConventionalNestedRecordSumPathLayoutReport, ConventionalNestedRecordSumPathsLayoutReport,
    ConventionalRecordSumChildHop, ConventionalRecordSumChildInterior,
    ConventionalRecordSumChildLayoutReport, ConventionalRecursiveRecordSumPathsLayoutReport,
    ConventionalSumArrayFieldLayoutReport, MaterializationDiagnostic,
    conventional_sum_layout_reports_match_for_replay, layout_plan_reports_match_for_replay,
    materialize_aggregate_layout_into, normalized_conventional_sum_layout_report_fingerprint,
    normalized_layout_plan_report_fingerprint,
};
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember, DataShapeKind};
use typed_trees::types::TypeReferenceNode;

use super::const_materializable::{
    hash_byte, hash_bytes, hash_text, hash_u64, hash_value, unique_data_by_name, validate_value,
    value_kind,
};
use super::const_record_with_sum_materializable::{
    EncodedOuterField, exact_named_data, field_occurrence_matches, flatten_literal_array_elements,
    prepare_sum_array_field, validate_outer_layout, validate_outer_record_owner,
};
use super::{
    BuildTimeValue, RepeatedFieldInfo, encode_typed_owned_value, exact_struct_fields,
    normalized_schema_report_fingerprint, reflected_field_layout,
    validate_const_materializable_conventional_sum,
    validate_const_materializable_record_with_conventional_sums,
};
use crate::layouts::layout_plans::{
    ValidatedConstRecordSumArrayElementSelection, ValidatedConstRecordSumArrayFieldMaterialization,
    ValidatedConstRecordSumFieldMaterialization, ValidatedConstRecordWithSumMaterialization,
};

mod derivation;
mod record_level;
mod recursive;
mod report_identity;
mod sum_reachability;

use derivation::*;
pub use record_level::ValidatedConstRecordSumChildMaterialization;
pub use recursive::{
    ValidatedConstRecordWithRecursiveNestedSumsMaterialization,
    validate_const_materializable_record_with_recursive_nested_sums,
};
use report_identity::*;
pub(super) use sum_reachability::SumReachability;
use sum_reachability::{record_sum_profile, reject_sum_array_type};

/// Exact custody for one outer-field occurrence at any supported recursive
/// record-path depth.
///
/// Each occurrence retains its own child custody without flattening the path.
#[derive(Debug)]
pub struct ValidatedConstRecursiveNestedSumOccurrenceMaterialization {
    outer_field: String,
    outer_member_identity: Option<u64>,
    inner: ValidatedConstRecordWithRecursiveNestedSumsMaterialization,
}

impl ValidatedConstRecursiveNestedSumOccurrenceMaterialization {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithRecursiveNestedSumsMaterialization {
        &self.inner
    }
}

/// Compact value-sensitive custody for one literal element of one direct
/// `[R; N]` record-array occurrence. The element record's complete
/// recursive report is retained once in the occurrence's compact row, so
/// each element keeps only its literal index, exact value, staged bytes,
/// and recursive materialization coordinate.
#[derive(Debug)]
pub struct ValidatedConstRecordArrayElementSelection {
    pub(crate) literal_index: u64,
    pub(crate) value: BuildTimeValue,
    pub(crate) bytes: Vec<u8>,
    pub(crate) non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordArrayElementSelection {
    pub const fn literal_index(&self) -> u64 {
        self.literal_index
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }
}

/// Value-sensitive custody for one direct fixed-array-of-records field in
/// the complete authored-order occurrence set — each retained element
/// selection keeps its own literal index and recursive materialization
/// coordinate beside the field's shared compact row.
#[derive(Debug)]
pub struct ValidatedConstRecordArrayFieldMaterialization {
    pub(crate) field: String,
    pub(crate) field_identity: Option<u64>,
    pub(crate) elements: Vec<ValidatedConstRecordArrayElementSelection>,
}

impl ValidatedConstRecordArrayFieldMaterialization {
    pub fn field(&self) -> &str {
        &self.field
    }

    pub const fn field_identity(&self) -> Option<u64> {
        self.field_identity
    }

    pub fn elements(&self) -> &[ValidatedConstRecordArrayElementSelection] {
        &self.elements
    }
}

/// One recursive level's derived child custody beside the level's complete
/// staged bytes — every classified child row in authored order.
struct DerivedRecordLevelChildrenMaterialization {
    schema_report_fingerprint: u64,
    children: Vec<ValidatedConstRecordSumChildMaterialization>,
    bytes: Vec<u8>,
}

/// Exact custody for one bounded outer-record -> inner-record -> direct-sums
/// materialization path.
///
/// The inner carrier is retained whole rather than flattening selected sums
/// into the outer record. This type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithNestedSumRecordMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalNestedRecordSumPathLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    inner: ValidatedConstRecordWithSumMaterialization,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithNestedSumRecordMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalNestedRecordSumPathLayoutReport {
        &self.path_layout
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithSumMaterialization {
        &self.inner
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Re-resolve the exact outer field and independently reconstruct both
    /// record layers and every selected child sum.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalNestedRecordSumPathLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record path invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !nested_path_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record path layout drifted from retained custody"
                    .into(),
            ));
        }

        let replayed =
            derive_nested_record_sum_bytes(typed, schema_name, path_layout, value, byte_order)?;
        self.inner.replay_against_sum_fields(
            typed,
            replayed.inner.schema_name(),
            &path_layout.inner_layout,
            &path_layout.child_sum_layouts,
            replayed.inner.value(),
            byte_order,
        )?;
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
            || replayed
                .inner
                .non_authoritative_materialization_report_fingerprint()
                != self
                    .inner
                    .non_authoritative_materialization_report_fingerprint()
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record path custody drifted from exact replay".into(),
            ));
        }
        let fingerprint = nested_record_sum_materialization_report_fingerprint(
            schema_name,
            replayed.schema_report_fingerprint,
            outer_fingerprint,
            path_layout,
            &replayed.inner,
            byte_order,
            value,
            &replayed.bytes,
        );
        if fingerprint != self.non_authoritative_materialization_report_fingerprint {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record path report fingerprint drifted from exact replay"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Replay every retained fact before one atomic copy of the outer image.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        self.replay_against(
            typed,
            &self.schema_name,
            &self.path_layout,
            &self.value,
            self.byte_order,
        )?;
        if destination.len() < self.bytes.len() {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable nested-record path copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

/// Validate the singular one-level record path and retain the inner direct-sum
/// carrier as one atomic outer field value.
pub fn validate_const_materializable_record_with_nested_sum_record(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalNestedRecordSumPathLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithNestedSumRecordMaterialization, MaterializationDiagnostic> {
    let derived =
        derive_nested_record_sum_bytes(typed, schema_name, path_layout, value, byte_order)?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = nested_record_sum_materialization_report_fingerprint(
        schema_name,
        derived.schema_report_fingerprint,
        outer_fingerprint,
        path_layout,
        &derived.inner,
        byte_order,
        value,
        &derived.bytes,
    );
    Ok(ValidatedConstRecordWithNestedSumRecordMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        path_layout: path_layout.clone(),
        non_authoritative_outer_layout_report_fingerprint: outer_fingerprint,
        inner: derived.inner,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_fingerprint,
    })
}

/// Exact custody for the complete authored-order set of qualifying direct
/// inner-record occurrences. Each occurrence retains one independent existing
/// direct-sum record carrier; the outer layout is retained only once.
#[derive(Debug)]
pub struct ValidatedConstRecordWithNestedSumRecordsMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalNestedRecordSumPathsLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    inner_records: Vec<ValidatedConstNestedSumRecordOccurrenceMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithNestedSumRecordsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalNestedRecordSumPathsLayoutReport {
        &self.path_layout
    }

    pub fn inner_records(&self) -> &[ValidatedConstNestedSumRecordOccurrenceMaterialization] {
        &self.inner_records
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
        path_layout: &ConventionalNestedRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        let mut reachability = SumReachability::new(typed);
        self.replay_against_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            &mut reachability,
        )
    }

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalNestedRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record paths invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !record_sum_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record paths layout drifted from retained custody"
                    .into(),
            ));
        }
        let replayed = derive_nested_record_sums_bytes_with_reachability(
            typed,
            schema_name,
            NestedPathsView::Plural(path_layout),
            value,
            byte_order,
            reachability,
        )?;
        if replayed.inner_records.len() != self.inner_records.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record occurrence custody changed cardinality".into(),
            ));
        }
        for (((retained, replayed), path), retained_path) in self
            .inner_records
            .iter()
            .zip(&replayed.inner_records)
            .zip(&path_layout.paths)
            .zip(&self.path_layout.paths)
        {
            if !field_occurrence_matches(
                retained.outer_field(),
                retained.outer_member_identity(),
                replayed.outer_field(),
                replayed.outer_member_identity(),
            ) || !field_occurrence_matches(
                &path.outer_field,
                path.outer_member_identity,
                &retained_path.outer_field,
                retained_path.outer_member_identity,
            ) {
                return Err(MaterializationDiagnostic(
                    "ConstMaterializable nested-record occurrence identity drifted from retained custody"
                        .into(),
                ));
            }
            retained.inner.replay_against_sum_fields(
                typed,
                replayed.inner.schema_name(),
                &path.inner_layout,
                &path.child_sum_layouts,
                replayed.inner.value(),
                byte_order,
            )?;
            if retained
                .inner
                .non_authoritative_materialization_report_fingerprint()
                != replayed
                    .inner
                    .non_authoritative_materialization_report_fingerprint()
            {
                return Err(MaterializationDiagnostic(
                    "ConstMaterializable nested-record inner report coordinate drifted after exact replay"
                        .into(),
                ));
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record paths bytes drifted from exact replay".into(),
            ));
        }
        let fingerprint = nested_record_sums_materialization_report_fingerprint(
            schema_name,
            replayed.schema_report_fingerprint,
            outer_fingerprint,
            path_layout,
            &replayed.inner_records,
            byte_order,
            value,
            &replayed.bytes,
        );
        if fingerprint != self.non_authoritative_materialization_report_fingerprint {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable nested-record paths report fingerprint drifted after exact replay"
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
            &self.path_layout,
            &self.value,
            self.byte_order,
        )?;
        if destination.len() < self.bytes.len() {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable nested-record paths copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_nested_sum_records(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalNestedRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithNestedSumRecordsMaterialization, MaterializationDiagnostic> {
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_nested_sum_records_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_nested_sum_records_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalNestedRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithNestedSumRecordsMaterialization, MaterializationDiagnostic> {
    let derived = derive_nested_record_sums_bytes_with_reachability(
        typed,
        schema_name,
        NestedPathsView::Plural(path_layout),
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = nested_record_sums_materialization_report_fingerprint(
        schema_name,
        derived.schema_report_fingerprint,
        outer_fingerprint,
        path_layout,
        &derived.inner_records,
        byte_order,
        value,
        &derived.bytes,
    );
    Ok(ValidatedConstRecordWithNestedSumRecordsMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        path_layout: path_layout.clone(),
        non_authoritative_outer_layout_report_fingerprint: outer_fingerprint,
        inner_records: derived.inner_records,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_fingerprint,
    })
}

struct DerivedNestedRecordSumMaterialization {
    schema_report_fingerprint: u64,
    inner: ValidatedConstRecordWithSumMaterialization,
    bytes: Vec<u8>,
}

struct DerivedNestedRecordSumsMaterialization {
    schema_report_fingerprint: u64,
    inner_records: Vec<ValidatedConstNestedSumRecordOccurrenceMaterialization>,
    bytes: Vec<u8>,
}

#[derive(Debug)]
pub struct ValidatedConstNestedSumRecordOccurrenceMaterialization {
    outer_field: String,
    outer_member_identity: Option<u64>,
    inner: ValidatedConstRecordWithSumMaterialization,
}

impl ValidatedConstNestedSumRecordOccurrenceMaterialization {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithSumMaterialization {
        &self.inner
    }
}

#[derive(Clone, Copy)]
struct NestedPathOccurrenceView<'a> {
    outer_field: &'a str,
    outer_member_identity: Option<u64>,
    inner_layout: &'a layout_plans::LayoutPlanReport,
    child_sum_layouts: &'a [layout_plans::ConventionalSumFieldLayoutReport],
}

#[derive(Clone, Copy)]
enum NestedPathsView<'a> {
    Singular(&'a ConventionalNestedRecordSumPathLayoutReport),
    Plural(&'a ConventionalNestedRecordSumPathsLayoutReport),
}

impl<'a> NestedPathsView<'a> {
    fn outer_layout(self) -> &'a layout_plans::LayoutPlanReport {
        match self {
            Self::Singular(report) => &report.outer_layout,
            Self::Plural(report) => &report.outer_layout,
        }
    }

    fn len(self) -> usize {
        match self {
            Self::Singular(_) => 1,
            Self::Plural(report) => report.paths.len(),
        }
    }

    fn get(self, index: usize) -> Option<NestedPathOccurrenceView<'a>> {
        match self {
            Self::Singular(report) if index == 0 => Some(NestedPathOccurrenceView {
                outer_field: &report.outer_field,
                outer_member_identity: report.outer_member_identity,
                inner_layout: &report.inner_layout,
                child_sum_layouts: &report.child_sum_layouts,
            }),
            Self::Singular(_) => None,
            Self::Plural(report) => report
                .paths
                .get(index)
                .map(|path| NestedPathOccurrenceView {
                    outer_field: &path.outer_field,
                    outer_member_identity: path.outer_member_identity,
                    inner_layout: &path.inner_layout,
                    child_sum_layouts: &path.child_sum_layouts,
                }),
        }
    }
}
