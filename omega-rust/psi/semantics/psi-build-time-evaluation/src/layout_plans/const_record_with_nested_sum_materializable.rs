//! Value-sensitive materialization of record fields containing records with
//! direct conventional pure-sum fields.

use psi_layout_plans::{
    AggregateFieldSchema, AggregateFieldValue, ByteOrder,
    ConventionalDepthEightRecordSumPathsLayoutReport,
    ConventionalDepthElevenRecordSumPathsLayoutReport,
    ConventionalDepthFifteenRecordSumPathsLayoutReport,
    ConventionalDepthFiveRecordSumPathsLayoutReport,
    ConventionalDepthFourRecordSumPathsLayoutReport,
    ConventionalDepthFourteenRecordSumPathsLayoutReport,
    ConventionalDepthNineRecordSumPathsLayoutReport,
    ConventionalDepthSevenRecordSumPathsLayoutReport,
    ConventionalDepthSixRecordSumPathsLayoutReport, ConventionalDepthTenRecordSumPathsLayoutReport,
    ConventionalDepthThirteenRecordSumPathsLayoutReport,
    ConventionalDepthThreeRecordSumPathLayoutReport,
    ConventionalDepthThreeRecordSumPathsLayoutReport,
    ConventionalDepthTwelveRecordSumPathsLayoutReport,
    ConventionalDepthTwoRecordSumPathLayoutReport, ConventionalDepthTwoRecordSumPathsLayoutReport,
    ConventionalNestedRecordSumPathLayoutReport, ConventionalNestedRecordSumPathsLayoutReport,
    ConventionalRecordSumPathsLayoutReport, MaterializationDiagnostic,
    conventional_sum_layout_reports_match_for_replay, layout_plan_reports_match_for_replay,
    materialize_aggregate_layout_into, normalized_layout_plan_report_fingerprint,
};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataDefinition, DataMember, DataShapeKind};
use psi_typed_trees::types::TypeReferenceNode;

use super::const_materializable::{
    hash_byte, hash_bytes, hash_text, hash_u64, hash_value, unique_data_by_name, validate_value,
    value_kind,
};
use super::const_record_with_sum_materializable::{
    EncodedOuterField, exact_named_data, field_occurrence_matches, validate_outer_layout,
    validate_outer_record_owner,
};
use super::{
    BuildTimeValue, encode_typed_owned_value, exact_struct_fields,
    normalized_schema_report_fingerprint, reflected_field_layout,
    validate_const_materializable_record_with_conventional_sums,
};
use crate::layout_plans::ValidatedConstRecordWithSumMaterialization;

mod derivation;
mod fixed_depths;
mod report_identity;
mod sum_reachability;

use derivation::*;
pub use fixed_depths::*;
use report_identity::*;
pub(super) use sum_reachability::SumReachability;
use sum_reachability::{record_sum_profile, reject_sum_array_type};

/// Exact custody for one outer-field occurrence at any supported recursive
/// record-path depth.
///
/// The concrete `Inner` type preserves the preceding depth in the Rust type;
/// fixed-depth public names below remain aliases with no dynamic widening.
#[derive(Debug)]
pub struct ValidatedConstRecursiveNestedSumOccurrenceMaterialization<Inner> {
    outer_field: String,
    outer_member_identity: Option<u64>,
    inner: Inner,
}

impl<Inner> ValidatedConstRecursiveNestedSumOccurrenceMaterialization<Inner> {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn inner(&self) -> &Inner {
        &self.inner
    }
}

struct DerivedRecursiveNestedSumsMaterialization<Inner> {
    schema_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstRecursiveNestedSumOccurrenceMaterialization<Inner>>,
    bytes: Vec<u8>,
}

/// Retained custody shared by every exact recursive record-path depth.
///
/// Concrete public aliases bind both the exact path-report depth and the exact
/// preceding materialization depth; this carrier does not erase either axis.
#[derive(Debug)]
pub struct ValidatedConstRecursiveNestedSumsMaterialization<PathLayout, Inner> {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: PathLayout,
    non_authoritative_outer_layout_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstRecursiveNestedSumOccurrenceMaterialization<Inner>>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl<PathLayout, Inner> ValidatedConstRecursiveNestedSumsMaterialization<PathLayout, Inner> {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &PathLayout {
        &self.path_layout
    }

    pub fn occurrences(
        &self,
    ) -> &[ValidatedConstRecursiveNestedSumOccurrenceMaterialization<Inner>] {
        &self.occurrences
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }
}

fn validate_recursive_nested_sums_with_reachability<InnerPaths, Inner, Derive>(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalRecordSumPathsLayoutReport<InnerPaths>,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
    fingerprint_domain: &[u8],
    mut derive: Derive,
    inner_report_fingerprint: fn(&Inner) -> u64,
) -> Result<
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalRecordSumPathsLayoutReport<InnerPaths>,
        Inner,
    >,
    MaterializationDiagnostic,
>
where
    InnerPaths: Clone + RecordSumPathsInnerLayout,
    Derive: FnMut(
        &TypedTrees,
        &str,
        &ConventionalRecordSumPathsLayoutReport<InnerPaths>,
        &BuildTimeValue,
        ByteOrder,
        &mut SumReachability<'_>,
    ) -> Result<
        DerivedRecursiveNestedSumsMaterialization<Inner>,
        MaterializationDiagnostic,
    >,
{
    let derived = derive(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = record_sum_paths_materialization_report_fingerprint(
        fingerprint_domain,
        schema_name,
        derived.schema_report_fingerprint,
        outer_fingerprint,
        path_layout,
        &derived.occurrences,
        byte_order,
        value,
        &derived.bytes,
        |occurrence| inner_report_fingerprint(&occurrence.inner),
    );
    Ok(ValidatedConstRecursiveNestedSumsMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        path_layout: path_layout.clone(),
        non_authoritative_outer_layout_report_fingerprint: outer_fingerprint,
        occurrences: derived.occurrences,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_fingerprint,
    })
}

fn replay_recursive_nested_sums_with_reachability<InnerPaths, Inner, Derive, ReplayInner>(
    retained: &ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalRecordSumPathsLayoutReport<InnerPaths>,
        Inner,
    >,
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalRecordSumPathsLayoutReport<InnerPaths>,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
    depth_label: &str,
    fingerprint_domain: &[u8],
    mut derive: Derive,
    mut replay_inner: ReplayInner,
    inner_schema_name: for<'a> fn(&'a Inner) -> &'a str,
    inner_value: for<'a> fn(&'a Inner) -> &'a BuildTimeValue,
    inner_report_fingerprint: fn(&Inner) -> u64,
) -> Result<(), MaterializationDiagnostic>
where
    InnerPaths: RecordSumPathsInnerLayout + RecordSumPathsReplay,
    Derive: FnMut(
        &TypedTrees,
        &str,
        &ConventionalRecordSumPathsLayoutReport<InnerPaths>,
        &BuildTimeValue,
        ByteOrder,
        &mut SumReachability<'_>,
    ) -> Result<
        DerivedRecursiveNestedSumsMaterialization<Inner>,
        MaterializationDiagnostic,
    >,
    ReplayInner: FnMut(
        &Inner,
        &TypedTrees,
        &str,
        &InnerPaths,
        &BuildTimeValue,
        ByteOrder,
        &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic>,
{
    if schema_name != retained.schema_name
        || value != &retained.value
        || byte_order != retained.byte_order
    {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural {depth_label} invocation drifted from retained custody"
        )));
    }
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    if outer_fingerprint != retained.non_authoritative_outer_layout_report_fingerprint
        || !record_sum_paths_reports_match_for_replay(path_layout, &retained.path_layout)
    {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural {depth_label} layout drifted from retained custody"
        )));
    }

    let replayed = derive(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    if replayed.occurrences.len() != retained.occurrences.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural {depth_label} custody changed cardinality"
        )));
    }
    for (((retained_occurrence, replayed_occurrence), path), retained_path) in retained
        .occurrences
        .iter()
        .zip(&replayed.occurrences)
        .zip(&path_layout.paths)
        .zip(&retained.path_layout.paths)
    {
        if !field_occurrence_matches(
            retained_occurrence.outer_field(),
            retained_occurrence.outer_member_identity(),
            replayed_occurrence.outer_field(),
            replayed_occurrence.outer_member_identity(),
        ) || !field_occurrence_matches(
            &path.outer_field,
            path.outer_member_identity,
            &retained_path.outer_field,
            retained_path.outer_member_identity,
        ) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural {depth_label} occurrence identity drifted from retained custody"
            )));
        }
        replay_inner(
            &retained_occurrence.inner,
            typed,
            inner_schema_name(&replayed_occurrence.inner),
            &path.inner,
            inner_value(&replayed_occurrence.inner),
            byte_order,
            reachability,
        )?;
        if inner_report_fingerprint(&retained_occurrence.inner)
            != inner_report_fingerprint(&replayed_occurrence.inner)
        {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable plural {depth_label} inner custody drifted after exact replay"
            )));
        }
    }
    if replayed.schema_report_fingerprint != retained.non_authoritative_schema_report_fingerprint
        || replayed.bytes != retained.bytes
    {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural {depth_label} bytes drifted after exact replay"
        )));
    }
    let fingerprint = record_sum_paths_materialization_report_fingerprint(
        fingerprint_domain,
        schema_name,
        replayed.schema_report_fingerprint,
        outer_fingerprint,
        path_layout,
        &replayed.occurrences,
        byte_order,
        value,
        &replayed.bytes,
        |occurrence| inner_report_fingerprint(&occurrence.inner),
    );
    if fingerprint != retained.non_authoritative_materialization_report_fingerprint {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural {depth_label} fingerprint drifted after exact replay"
        )));
    }
    Ok(())
}

fn replay_recursive_nested_sums<PathLayout, Inner, Replay>(
    retained: &ValidatedConstRecursiveNestedSumsMaterialization<PathLayout, Inner>,
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &PathLayout,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    mut replay: Replay,
) -> Result<(), MaterializationDiagnostic>
where
    Replay: FnMut(
        &ValidatedConstRecursiveNestedSumsMaterialization<PathLayout, Inner>,
        &TypedTrees,
        &str,
        &PathLayout,
        &BuildTimeValue,
        ByteOrder,
        &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic>,
{
    let mut reachability = SumReachability::new(typed);
    replay(
        retained,
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn apply_recursive_nested_sums<PathLayout, Inner, Replay>(
    retained: &ValidatedConstRecursiveNestedSumsMaterialization<PathLayout, Inner>,
    typed: &TypedTrees,
    destination: &mut [u8],
    depth_label: &str,
    mut replay: Replay,
) -> Result<(), MaterializationDiagnostic>
where
    Replay: FnMut(
        &ValidatedConstRecursiveNestedSumsMaterialization<PathLayout, Inner>,
        &TypedTrees,
        &str,
        &PathLayout,
        &BuildTimeValue,
        ByteOrder,
    ) -> Result<(), MaterializationDiagnostic>,
{
    replay(
        retained,
        typed,
        &retained.schema_name,
        &retained.path_layout,
        &retained.value,
        retained.byte_order,
    )?;
    if destination.len() < retained.bytes.len() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable plural {depth_label} copy needs {} bytes, destination has {}",
            retained.bytes.len(),
            destination.len()
        )));
    }
    destination[..retained.bytes.len()].copy_from_slice(&retained.bytes);
    Ok(())
}

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-thirteen path set.
///
/// The nested carrier retains its complete plural depth-twelve custody. This
/// type deliberately does not implement `Clone`.
pub type ValidatedConstDepthThirteenNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-twelve carrier. This type deliberately
/// does not implement `Clone`.
pub type ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization =
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalDepthThirteenRecordSumPathsLayoutReport,
        ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization,
    >;

impl ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization {
    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-twelve carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthThirteenRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            Self::replay_against_with_reachability,
        )
    }

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthThirteenRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums_with_reachability(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
            "depth-thirteen",
            b"omega.const-materializable-plural-depth-thirteen-record-sum-paths.v1",
            derive_depth_thirteen_nested_sums_bytes_with_reachability,
            ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization::replay_against_with_reachability,
            ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization::schema_name,
            ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization::value,
            ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
        )
    }

    /// Replay complete retained custody before one atomic outer-image copy.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        apply_recursive_nested_sums(
            self,
            typed,
            destination,
            "depth-thirteen",
            Self::replay_against,
        )
    }
}

pub fn validate_const_materializable_record_with_depth_thirteen_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthThirteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization, MaterializationDiagnostic>
{
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_thirteen_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_thirteen_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthThirteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization, MaterializationDiagnostic>
{
    validate_recursive_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        b"omega.const-materializable-plural-depth-thirteen-record-sum-paths.v1",
        derive_depth_thirteen_nested_sums_bytes_with_reachability,
        ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
    )
}

type DerivedDepthThirteenNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization,
>;

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-fourteen path set.
///
/// The nested carrier retains its complete plural depth-thirteen custody. This
/// type deliberately does not implement `Clone`.
pub type ValidatedConstDepthFourteenNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-thirteen carrier. This type deliberately
/// does not implement `Clone`.
pub type ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization =
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalDepthFourteenRecordSumPathsLayoutReport,
        ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization,
    >;

impl ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization {
    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-thirteen carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthFourteenRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            Self::replay_against_with_reachability,
        )
    }

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthFourteenRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums_with_reachability(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
            "depth-fourteen",
            b"omega.const-materializable-plural-depth-fourteen-record-sum-paths.v1",
            derive_depth_fourteen_nested_sums_bytes_with_reachability,
            ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization::replay_against_with_reachability,
            ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization::schema_name,
            ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization::value,
            ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
        )
    }

    /// Replay complete retained custody before one atomic outer-image copy.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        apply_recursive_nested_sums(
            self,
            typed,
            destination,
            "depth-fourteen",
            Self::replay_against,
        )
    }
}

pub fn validate_const_materializable_record_with_depth_fourteen_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFourteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization, MaterializationDiagnostic>
{
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_fourteen_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_fourteen_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFourteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization, MaterializationDiagnostic>
{
    validate_recursive_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        b"omega.const-materializable-plural-depth-fourteen-record-sum-paths.v1",
        derive_depth_fourteen_nested_sums_bytes_with_reachability,
        ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
    )
}

type DerivedDepthFourteenNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthThirteenNestedSumsMaterialization,
>;

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-fifteen path set.
///
/// The nested carrier retains its complete plural depth-fourteen custody. This
/// type deliberately does not implement `Clone`.
pub type ValidatedConstDepthFifteenNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-fourteen carrier. This type deliberately
/// does not implement `Clone`.
pub type ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization =
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalDepthFifteenRecordSumPathsLayoutReport,
        ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization,
    >;

impl ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization {
    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-fourteen carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthFifteenRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            Self::replay_against_with_reachability,
        )
    }

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthFifteenRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums_with_reachability(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
            "depth-fifteen",
            b"omega.const-materializable-plural-depth-fifteen-record-sum-paths.v1",
            derive_depth_fifteen_nested_sums_bytes_with_reachability,
            ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization::replay_against_with_reachability,
            ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization::schema_name,
            ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization::value,
            ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
        )
    }

    /// Replay complete retained custody before one atomic outer-image copy.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        apply_recursive_nested_sums(
            self,
            typed,
            destination,
            "depth-fifteen",
            Self::replay_against,
        )
    }
}

pub fn validate_const_materializable_record_with_depth_fifteen_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFifteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization, MaterializationDiagnostic>
{
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_fifteen_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_fifteen_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFifteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization, MaterializationDiagnostic>
{
    validate_recursive_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        b"omega.const-materializable-plural-depth-fifteen-record-sum-paths.v1",
        derive_depth_fifteen_nested_sums_bytes_with_reachability,
        ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
    )
}

type DerivedDepthFifteenNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthFourteenNestedSumsMaterialization,
>;

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-twelve path set.
///
/// The nested carrier retains its complete plural depth-eleven custody. This
/// type deliberately does not implement `Clone`.
pub type ValidatedConstDepthTwelveNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthElevenNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-eleven carrier. This type deliberately
/// does not implement `Clone`.
pub type ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization =
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalDepthTwelveRecordSumPathsLayoutReport,
        ValidatedConstRecordWithDepthElevenNestedSumsMaterialization,
    >;

impl ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization {
    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-eleven carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthTwelveRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            Self::replay_against_with_reachability,
        )
    }

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthTwelveRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums_with_reachability(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
            "depth-twelve",
            b"omega.const-materializable-plural-depth-twelve-record-sum-paths.v1",
            derive_depth_twelve_nested_sums_bytes_with_reachability,
            ValidatedConstRecordWithDepthElevenNestedSumsMaterialization::replay_against_with_reachability,
            ValidatedConstRecordWithDepthElevenNestedSumsMaterialization::schema_name,
            ValidatedConstRecordWithDepthElevenNestedSumsMaterialization::value,
            ValidatedConstRecordWithDepthElevenNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
        )
    }

    /// Replay complete retained custody before one atomic outer-image copy.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        apply_recursive_nested_sums(
            self,
            typed,
            destination,
            "depth-twelve",
            Self::replay_against,
        )
    }
}

pub fn validate_const_materializable_record_with_depth_twelve_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwelveRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization, MaterializationDiagnostic>
{
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_twelve_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_twelve_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwelveRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthTwelveNestedSumsMaterialization, MaterializationDiagnostic>
{
    validate_recursive_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        b"omega.const-materializable-plural-depth-twelve-record-sum-paths.v1",
        derive_depth_twelve_nested_sums_bytes_with_reachability,
        ValidatedConstRecordWithDepthElevenNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
    )
}

type DerivedDepthTwelveNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthElevenNestedSumsMaterialization,
>;
/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-eleven path set.
///
/// The nested carrier retains its complete plural depth-ten custody. This
/// type deliberately does not implement `Clone`.
pub type ValidatedConstDepthElevenNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthTenNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-ten carrier. This type deliberately
/// does not implement `Clone`.
pub type ValidatedConstRecordWithDepthElevenNestedSumsMaterialization =
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalDepthElevenRecordSumPathsLayoutReport,
        ValidatedConstRecordWithDepthTenNestedSumsMaterialization,
    >;

impl ValidatedConstRecordWithDepthElevenNestedSumsMaterialization {
    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-ten carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthElevenRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            Self::replay_against_with_reachability,
        )
    }

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthElevenRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums_with_reachability(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
            "depth-eleven",
            b"omega.const-materializable-plural-depth-eleven-record-sum-paths.v1",
            derive_depth_eleven_nested_sums_bytes_with_reachability,
            ValidatedConstRecordWithDepthTenNestedSumsMaterialization::replay_against_with_reachability,
            ValidatedConstRecordWithDepthTenNestedSumsMaterialization::schema_name,
            ValidatedConstRecordWithDepthTenNestedSumsMaterialization::value,
            ValidatedConstRecordWithDepthTenNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
        )
    }

    /// Replay complete retained custody before one atomic outer-image copy.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        apply_recursive_nested_sums(
            self,
            typed,
            destination,
            "depth-eleven",
            Self::replay_against,
        )
    }
}

pub fn validate_const_materializable_record_with_depth_eleven_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthElevenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthElevenNestedSumsMaterialization, MaterializationDiagnostic>
{
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_eleven_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_eleven_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthElevenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthElevenNestedSumsMaterialization, MaterializationDiagnostic>
{
    validate_recursive_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        b"omega.const-materializable-plural-depth-eleven-record-sum-paths.v1",
        derive_depth_eleven_nested_sums_bytes_with_reachability,
        ValidatedConstRecordWithDepthTenNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
    )
}

type DerivedDepthElevenNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthTenNestedSumsMaterialization,
>;

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-ten path set.
///
/// The nested carrier retains its complete plural depth-nine custody. This
/// type deliberately does not implement `Clone`.
pub type ValidatedConstDepthTenNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthNineNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-nine carrier. This type deliberately
/// does not implement `Clone`.
pub type ValidatedConstRecordWithDepthTenNestedSumsMaterialization =
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalDepthTenRecordSumPathsLayoutReport,
        ValidatedConstRecordWithDepthNineNestedSumsMaterialization,
    >;

impl ValidatedConstRecordWithDepthTenNestedSumsMaterialization {
    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-nine carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthTenRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            Self::replay_against_with_reachability,
        )
    }

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthTenRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums_with_reachability(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
            "depth-ten",
            b"omega.const-materializable-plural-depth-ten-record-sum-paths.v1",
            derive_depth_ten_nested_sums_bytes_with_reachability,
            ValidatedConstRecordWithDepthNineNestedSumsMaterialization::replay_against_with_reachability,
            ValidatedConstRecordWithDepthNineNestedSumsMaterialization::schema_name,
            ValidatedConstRecordWithDepthNineNestedSumsMaterialization::value,
            ValidatedConstRecordWithDepthNineNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
        )
    }

    /// Replay complete retained custody before one atomic outer-image copy.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        apply_recursive_nested_sums(self, typed, destination, "depth-ten", Self::replay_against)
    }
}

pub fn validate_const_materializable_record_with_depth_ten_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthTenNestedSumsMaterialization, MaterializationDiagnostic> {
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_ten_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_ten_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthTenNestedSumsMaterialization, MaterializationDiagnostic> {
    validate_recursive_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        b"omega.const-materializable-plural-depth-ten-record-sum-paths.v1",
        derive_depth_ten_nested_sums_bytes_with_reachability,
        ValidatedConstRecordWithDepthNineNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
    )
}

type DerivedDepthTenNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthNineNestedSumsMaterialization,
>;

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-nine path set.
///
/// The nested carrier retains its complete plural depth-eight custody. This
/// type deliberately does not implement `Clone`.
pub type ValidatedConstDepthNineNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthEightNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-eight carrier. This type deliberately
/// does not implement `Clone`.
pub type ValidatedConstRecordWithDepthNineNestedSumsMaterialization =
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalDepthNineRecordSumPathsLayoutReport,
        ValidatedConstRecordWithDepthEightNestedSumsMaterialization,
    >;

impl ValidatedConstRecordWithDepthNineNestedSumsMaterialization {
    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-eight carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthNineRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            Self::replay_against_with_reachability,
        )
    }

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthNineRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums_with_reachability(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
            "depth-nine",
            b"omega.const-materializable-plural-depth-nine-record-sum-paths.v1",
            derive_depth_nine_nested_sums_bytes_with_reachability,
            ValidatedConstRecordWithDepthEightNestedSumsMaterialization::replay_against_with_reachability,
            ValidatedConstRecordWithDepthEightNestedSumsMaterialization::schema_name,
            ValidatedConstRecordWithDepthEightNestedSumsMaterialization::value,
            ValidatedConstRecordWithDepthEightNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
        )
    }

    /// Replay complete retained custody before one atomic outer-image copy.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        apply_recursive_nested_sums(self, typed, destination, "depth-nine", Self::replay_against)
    }
}

pub fn validate_const_materializable_record_with_depth_nine_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthNineRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthNineNestedSumsMaterialization, MaterializationDiagnostic> {
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_nine_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_nine_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthNineRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthNineNestedSumsMaterialization, MaterializationDiagnostic> {
    validate_recursive_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        b"omega.const-materializable-plural-depth-nine-record-sum-paths.v1",
        derive_depth_nine_nested_sums_bytes_with_reachability,
        ValidatedConstRecordWithDepthEightNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
    )
}

type DerivedDepthNineNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthEightNestedSumsMaterialization,
>;

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-eight path set.
///
/// The nested carrier retains its complete plural depth-seven custody. This
/// type deliberately does not implement `Clone`.
pub type ValidatedConstDepthEightNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthSevenNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-seven carrier. This type deliberately
/// does not implement `Clone`.
pub type ValidatedConstRecordWithDepthEightNestedSumsMaterialization =
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalDepthEightRecordSumPathsLayoutReport,
        ValidatedConstRecordWithDepthSevenNestedSumsMaterialization,
    >;

impl ValidatedConstRecordWithDepthEightNestedSumsMaterialization {
    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-seven carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthEightRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            Self::replay_against_with_reachability,
        )
    }

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthEightRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums_with_reachability(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
            "depth-eight",
            b"omega.const-materializable-plural-depth-eight-record-sum-paths.v1",
            derive_depth_eight_nested_sums_bytes_with_reachability,
            ValidatedConstRecordWithDepthSevenNestedSumsMaterialization::replay_against_with_reachability,
            ValidatedConstRecordWithDepthSevenNestedSumsMaterialization::schema_name,
            ValidatedConstRecordWithDepthSevenNestedSumsMaterialization::value,
            ValidatedConstRecordWithDepthSevenNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
        )
    }

    /// Replay complete retained custody before one atomic outer-image copy.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        apply_recursive_nested_sums(
            self,
            typed,
            destination,
            "depth-eight",
            Self::replay_against,
        )
    }
}

pub fn validate_const_materializable_record_with_depth_eight_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthEightRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthEightNestedSumsMaterialization, MaterializationDiagnostic>
{
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_eight_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_eight_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthEightRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthEightNestedSumsMaterialization, MaterializationDiagnostic>
{
    validate_recursive_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        b"omega.const-materializable-plural-depth-eight-record-sum-paths.v1",
        derive_depth_eight_nested_sums_bytes_with_reachability,
        ValidatedConstRecordWithDepthSevenNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
    )
}

type DerivedDepthEightNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthSevenNestedSumsMaterialization,
>;

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-seven path set.
///
/// The nested carrier retains its complete plural depth-six custody. This
/// type deliberately does not implement `Clone`.
pub type ValidatedConstDepthSevenNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthSixNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-six carrier. This type deliberately
/// does not implement `Clone`.
pub type ValidatedConstRecordWithDepthSevenNestedSumsMaterialization =
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalDepthSevenRecordSumPathsLayoutReport,
        ValidatedConstRecordWithDepthSixNestedSumsMaterialization,
    >;

impl ValidatedConstRecordWithDepthSevenNestedSumsMaterialization {
    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-six carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthSevenRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            Self::replay_against_with_reachability,
        )
    }

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthSevenRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums_with_reachability(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
            "depth-seven",
            b"omega.const-materializable-plural-depth-seven-record-sum-paths.v1",
            derive_depth_seven_nested_sums_bytes_with_reachability,
            ValidatedConstRecordWithDepthSixNestedSumsMaterialization::replay_against_with_reachability,
            ValidatedConstRecordWithDepthSixNestedSumsMaterialization::schema_name,
            ValidatedConstRecordWithDepthSixNestedSumsMaterialization::value,
            ValidatedConstRecordWithDepthSixNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
        )
    }

    /// Replay complete retained custody before one atomic outer-image copy.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        apply_recursive_nested_sums(
            self,
            typed,
            destination,
            "depth-seven",
            Self::replay_against,
        )
    }
}

pub fn validate_const_materializable_record_with_depth_seven_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthSevenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthSevenNestedSumsMaterialization, MaterializationDiagnostic>
{
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_seven_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_seven_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthSevenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthSevenNestedSumsMaterialization, MaterializationDiagnostic>
{
    validate_recursive_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        b"omega.const-materializable-plural-depth-seven-record-sum-paths.v1",
        derive_depth_seven_nested_sums_bytes_with_reachability,
        ValidatedConstRecordWithDepthSixNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
    )
}

type DerivedDepthSevenNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthSixNestedSumsMaterialization,
>;

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-six path set.
///
/// The nested carrier retains its complete plural depth-five custody. This
/// type deliberately does not implement `Clone`.
pub type ValidatedConstDepthSixNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthFiveNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-five carrier. This type deliberately
/// does not implement `Clone`.
pub type ValidatedConstRecordWithDepthSixNestedSumsMaterialization =
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalDepthSixRecordSumPathsLayoutReport,
        ValidatedConstRecordWithDepthFiveNestedSumsMaterialization,
    >;

impl ValidatedConstRecordWithDepthSixNestedSumsMaterialization {
    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-five carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthSixRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            Self::replay_against_with_reachability,
        )
    }

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthSixRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums_with_reachability(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
            "depth-six",
            b"omega.const-materializable-plural-depth-six-record-sum-paths.v1",
            derive_depth_six_nested_sums_bytes_with_reachability,
            ValidatedConstRecordWithDepthFiveNestedSumsMaterialization::replay_against_with_reachability,
            ValidatedConstRecordWithDepthFiveNestedSumsMaterialization::schema_name,
            ValidatedConstRecordWithDepthFiveNestedSumsMaterialization::value,
            ValidatedConstRecordWithDepthFiveNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
        )
    }

    /// Replay complete retained custody before one atomic outer-image copy.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        apply_recursive_nested_sums(self, typed, destination, "depth-six", Self::replay_against)
    }
}

pub fn validate_const_materializable_record_with_depth_six_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthSixRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthSixNestedSumsMaterialization, MaterializationDiagnostic> {
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_six_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_six_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthSixRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthSixNestedSumsMaterialization, MaterializationDiagnostic> {
    validate_recursive_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        b"omega.const-materializable-plural-depth-six-record-sum-paths.v1",
        derive_depth_six_nested_sums_bytes_with_reachability,
        ValidatedConstRecordWithDepthFiveNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
    )
}

type DerivedDepthSixNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthFiveNestedSumsMaterialization,
>;

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-five path set.
///
/// The nested carrier retains its complete plural depth-four custody. This
/// type deliberately does not implement `Clone`.
pub type ValidatedConstDepthFiveNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthFourNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-four carrier. This type deliberately
/// does not implement `Clone`.
pub type ValidatedConstRecordWithDepthFiveNestedSumsMaterialization =
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalDepthFiveRecordSumPathsLayoutReport,
        ValidatedConstRecordWithDepthFourNestedSumsMaterialization,
    >;

impl ValidatedConstRecordWithDepthFiveNestedSumsMaterialization {
    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-four carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthFiveRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            Self::replay_against_with_reachability,
        )
    }

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthFiveRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums_with_reachability(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
            "depth-five",
            b"omega.const-materializable-plural-depth-five-record-sum-paths.v1",
            derive_depth_five_nested_sums_bytes_with_reachability,
            ValidatedConstRecordWithDepthFourNestedSumsMaterialization::replay_against_with_reachability,
            ValidatedConstRecordWithDepthFourNestedSumsMaterialization::schema_name,
            ValidatedConstRecordWithDepthFourNestedSumsMaterialization::value,
            ValidatedConstRecordWithDepthFourNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
        )
    }

    /// Replay complete retained custody before one atomic outer-image copy.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        apply_recursive_nested_sums(self, typed, destination, "depth-five", Self::replay_against)
    }
}

pub fn validate_const_materializable_record_with_depth_five_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFiveRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthFiveNestedSumsMaterialization, MaterializationDiagnostic> {
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_five_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_five_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFiveRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthFiveNestedSumsMaterialization, MaterializationDiagnostic> {
    validate_recursive_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        b"omega.const-materializable-plural-depth-five-record-sum-paths.v1",
        derive_depth_five_nested_sums_bytes_with_reachability,
        ValidatedConstRecordWithDepthFourNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
    )
}

type DerivedDepthFiveNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthFourNestedSumsMaterialization,
>;

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-four path set.
///
/// The nested carrier retains its complete plural depth-three custody. This
/// type deliberately does not implement `Clone`.
pub type ValidatedConstDepthFourNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthThreeNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-three carrier. This type
/// deliberately does not implement `Clone`.
pub type ValidatedConstRecordWithDepthFourNestedSumsMaterialization =
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalDepthFourRecordSumPathsLayoutReport,
        ValidatedConstRecordWithDepthThreeNestedSumsMaterialization,
    >;

impl ValidatedConstRecordWithDepthFourNestedSumsMaterialization {
    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-three carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthFourRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            Self::replay_against_with_reachability,
        )
    }

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthFourRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums_with_reachability(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
            "depth-four",
            b"omega.const-materializable-plural-depth-four-record-sum-paths.v1",
            derive_depth_four_nested_sums_bytes_with_reachability,
            ValidatedConstRecordWithDepthThreeNestedSumsMaterialization::replay_against_with_reachability,
            ValidatedConstRecordWithDepthThreeNestedSumsMaterialization::schema_name,
            ValidatedConstRecordWithDepthThreeNestedSumsMaterialization::value,
            ValidatedConstRecordWithDepthThreeNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
        )
    }

    /// Replay complete retained custody before one atomic outer-image copy.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        apply_recursive_nested_sums(self, typed, destination, "depth-four", Self::replay_against)
    }
}

pub fn validate_const_materializable_record_with_depth_four_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFourRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthFourNestedSumsMaterialization, MaterializationDiagnostic> {
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_four_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_four_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthFourRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthFourNestedSumsMaterialization, MaterializationDiagnostic> {
    validate_recursive_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        b"omega.const-materializable-plural-depth-four-record-sum-paths.v1",
        derive_depth_four_nested_sums_bytes_with_reachability,
        ValidatedConstRecordWithDepthThreeNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
    )
}

type DerivedDepthFourNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthThreeNestedSumsMaterialization,
>;

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-three path set.
///
/// The nested carrier retains its own complete authored-order depth-two path
/// set. This type deliberately does not implement `Clone`.
pub type ValidatedConstDepthThreeNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthTwoNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one existing plural depth-two carrier. This type deliberately
/// does not implement `Clone`.
pub type ValidatedConstRecordWithDepthThreeNestedSumsMaterialization =
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalDepthThreeRecordSumPathsLayoutReport,
        ValidatedConstRecordWithDepthTwoNestedSumsMaterialization,
    >;

impl ValidatedConstRecordWithDepthThreeNestedSumsMaterialization {
    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-two carrier before accepting the staged outer image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthThreeRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            Self::replay_against_with_reachability,
        )
    }

    fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthThreeRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        replay_recursive_nested_sums_with_reachability(
            self,
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
            "depth-three",
            b"omega.const-materializable-plural-depth-three-record-sum-paths.v1",
            derive_depth_three_nested_sums_bytes_with_reachability,
            ValidatedConstRecordWithDepthTwoNestedSumsMaterialization::replay_against_with_reachability,
            ValidatedConstRecordWithDepthTwoNestedSumsMaterialization::schema_name,
            ValidatedConstRecordWithDepthTwoNestedSumsMaterialization::value,
            ValidatedConstRecordWithDepthTwoNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
        )
    }

    /// Replay the complete retained custody before one atomic outer-image copy.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        apply_recursive_nested_sums(
            self,
            typed,
            destination,
            "depth-three",
            Self::replay_against,
        )
    }
}

pub fn validate_const_materializable_record_with_depth_three_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthThreeRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthThreeNestedSumsMaterialization, MaterializationDiagnostic>
{
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_three_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_three_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthThreeRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthThreeNestedSumsMaterialization, MaterializationDiagnostic>
{
    validate_recursive_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        b"omega.const-materializable-plural-depth-three-record-sum-paths.v1",
        derive_depth_three_nested_sums_bytes_with_reachability,
        ValidatedConstRecordWithDepthTwoNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
    )
}

type DerivedDepthThreeNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthTwoNestedSumsMaterialization,
>;

/// Exact custody for one fixed-depth
/// `Outer -> First -> Middle -> Leaf -> direct sums` chain.
///
/// The complete existing depth-two carrier stays nested rather than flattening
/// any child layout or selected sum into the new outer record. This type
/// deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthThreeNestedSumMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthThreeRecordSumPathLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    inner: ValidatedConstRecordWithDepthTwoNestedSumMaterialization,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthThreeNestedSumMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthThreeRecordSumPathLayoutReport {
        &self.path_layout
    }

    pub const fn inner(&self) -> &ValidatedConstRecordWithDepthTwoNestedSumMaterialization {
        &self.inner
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
        path_layout: &ConventionalDepthThreeRecordSumPathLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable depth-three record path invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !depth_three_path_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable depth-three record path layout drifted from retained custody"
                    .into(),
            ));
        }
        let replayed = derive_depth_three_nested_sum_bytes(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
        )?;
        self.inner.replay_against(
            typed,
            replayed.inner.schema_name(),
            &path_layout.depth_two_path,
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
                "ConstMaterializable depth-three record path custody drifted from exact replay"
                    .into(),
            ));
        }
        let fingerprint = depth_three_nested_sum_materialization_report_fingerprint(
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
                "ConstMaterializable depth-three record path fingerprint drifted after exact replay"
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
                "ConstMaterializable depth-three record copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_three_nested_sum(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthThreeRecordSumPathLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthThreeNestedSumMaterialization, MaterializationDiagnostic> {
    let derived =
        derive_depth_three_nested_sum_bytes(typed, schema_name, path_layout, value, byte_order)?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = depth_three_nested_sum_materialization_report_fingerprint(
        schema_name,
        derived.schema_report_fingerprint,
        outer_fingerprint,
        path_layout,
        &derived.inner,
        byte_order,
        value,
        &derived.bytes,
    );
    Ok(ValidatedConstRecordWithDepthThreeNestedSumMaterialization {
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

struct DerivedDepthThreeNestedSumMaterialization {
    schema_report_fingerprint: u64,
    inner: ValidatedConstRecordWithDepthTwoNestedSumMaterialization,
    bytes: Vec<u8>,
}

/// Exact custody for one fixed-depth
/// `Outer -> Middle -> Leaf -> direct sums` chain.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthTwoNestedSumMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthTwoRecordSumPathLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    middle: ValidatedConstRecordWithNestedSumRecordMaterialization,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthTwoNestedSumMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthTwoRecordSumPathLayoutReport {
        &self.path_layout
    }

    pub const fn middle(&self) -> &ValidatedConstRecordWithNestedSumRecordMaterialization {
        &self.middle
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
        path_layout: &ConventionalDepthTwoRecordSumPathLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable depth-two record path invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !depth_two_path_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable depth-two record path layout drifted from retained custody"
                    .into(),
            ));
        }
        let replayed =
            derive_depth_two_nested_sum_bytes(typed, schema_name, path_layout, value, byte_order)?;
        self.middle.replay_against(
            typed,
            replayed.middle.schema_name(),
            &path_layout.middle_path,
            replayed.middle.value(),
            byte_order,
        )?;
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
            || replayed
                .middle
                .non_authoritative_materialization_report_fingerprint()
                != self
                    .middle
                    .non_authoritative_materialization_report_fingerprint()
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable depth-two record path custody drifted from exact replay"
                    .into(),
            ));
        }
        let fingerprint = depth_two_nested_sum_materialization_report_fingerprint(
            schema_name,
            replayed.schema_report_fingerprint,
            outer_fingerprint,
            path_layout,
            &replayed.middle,
            byte_order,
            value,
            &replayed.bytes,
        );
        if fingerprint != self.non_authoritative_materialization_report_fingerprint {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable depth-two record path fingerprint drifted after exact replay"
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
                "ConstMaterializable depth-two record copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_two_nested_sum(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwoRecordSumPathLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthTwoNestedSumMaterialization, MaterializationDiagnostic> {
    let derived =
        derive_depth_two_nested_sum_bytes(typed, schema_name, path_layout, value, byte_order)?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = depth_two_nested_sum_materialization_report_fingerprint(
        schema_name,
        derived.schema_report_fingerprint,
        outer_fingerprint,
        path_layout,
        &derived.middle,
        byte_order,
        value,
        &derived.bytes,
    );
    Ok(ValidatedConstRecordWithDepthTwoNestedSumMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        path_layout: path_layout.clone(),
        non_authoritative_outer_layout_report_fingerprint: outer_fingerprint,
        middle: derived.middle,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_fingerprint,
    })
}

struct DerivedDepthTwoNestedSumMaterialization {
    schema_report_fingerprint: u64,
    middle: ValidatedConstRecordWithNestedSumRecordMaterialization,
    bytes: Vec<u8>,
}

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-two path set.
///
/// The middle carrier retains its own complete authored-order leaf-record
/// occurrence set. This type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstDepthTwoNestedSumOccurrenceMaterialization {
    outer_field: String,
    outer_member_identity: Option<u64>,
    middle: ValidatedConstRecordWithNestedSumRecordsMaterialization,
}

impl ValidatedConstDepthTwoNestedSumOccurrenceMaterialization {
    pub fn outer_field(&self) -> &str {
        &self.outer_field
    }

    pub const fn outer_member_identity(&self) -> Option<u64> {
        self.outer_member_identity
    }

    pub const fn middle(&self) -> &ValidatedConstRecordWithNestedSumRecordsMaterialization {
        &self.middle
    }
}

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one existing plural one-level carrier, preserving repeated
/// occurrences without forming a cross-product of layouts or selected values.
/// This type deliberately does not implement `Clone`.
#[derive(Debug)]
pub struct ValidatedConstRecordWithDepthTwoNestedSumsMaterialization {
    schema_name: String,
    non_authoritative_schema_report_fingerprint: u64,
    value: BuildTimeValue,
    path_layout: ConventionalDepthTwoRecordSumPathsLayoutReport,
    non_authoritative_outer_layout_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthTwoNestedSumOccurrenceMaterialization>,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstRecordWithDepthTwoNestedSumsMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub const fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub const fn path_layout(&self) -> &ConventionalDepthTwoRecordSumPathsLayoutReport {
        &self.path_layout
    }

    pub fn occurrences(&self) -> &[ValidatedConstDepthTwoNestedSumOccurrenceMaterialization] {
        &self.occurrences
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained middle carrier before accepting the staged outer image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthTwoRecordSumPathsLayoutReport,
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
        path_layout: &ConventionalDepthTwoRecordSumPathsLayoutReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
        reachability: &mut SumReachability<'_>,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name || value != &self.value || byte_order != self.byte_order
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-two invocation drifted from retained custody"
                    .into(),
            ));
        }
        let outer_fingerprint =
            normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
        if outer_fingerprint != self.non_authoritative_outer_layout_report_fingerprint
            || !record_sum_paths_reports_match_for_replay(path_layout, &self.path_layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-two layout drifted from retained custody".into(),
            ));
        }

        let replayed = derive_depth_two_nested_sums_bytes_with_reachability(
            typed,
            schema_name,
            path_layout,
            value,
            byte_order,
            reachability,
        )?;
        if replayed.occurrences.len() != self.occurrences.len() {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-two custody changed cardinality".into(),
            ));
        }
        for (((retained, replayed), path), retained_path) in self
            .occurrences
            .iter()
            .zip(&replayed.occurrences)
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
                    "ConstMaterializable plural depth-two occurrence identity drifted from retained custody"
                        .into(),
                ));
            }
            retained.middle.replay_against_with_reachability(
                typed,
                replayed.middle.schema_name(),
                &path.inner,
                replayed.middle.value(),
                byte_order,
                reachability,
            )?;
            if retained
                .middle
                .non_authoritative_materialization_report_fingerprint()
                != replayed
                    .middle
                    .non_authoritative_materialization_report_fingerprint()
            {
                return Err(MaterializationDiagnostic(
                    "ConstMaterializable plural depth-two middle custody drifted after exact replay"
                        .into(),
                ));
            }
        }
        if replayed.schema_report_fingerprint != self.non_authoritative_schema_report_fingerprint
            || replayed.bytes != self.bytes
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-two bytes drifted after exact replay".into(),
            ));
        }
        let fingerprint = depth_two_nested_sums_materialization_report_fingerprint(
            schema_name,
            replayed.schema_report_fingerprint,
            outer_fingerprint,
            path_layout,
            &replayed.occurrences,
            byte_order,
            value,
            &replayed.bytes,
        );
        if fingerprint != self.non_authoritative_materialization_report_fingerprint {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable plural depth-two fingerprint drifted after exact replay"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Replay the complete retained custody before one atomic outer-image copy.
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
                "ConstMaterializable plural depth-two copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

pub fn validate_const_materializable_record_with_depth_two_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwoRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthTwoNestedSumsMaterialization, MaterializationDiagnostic> {
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_two_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

fn validate_const_materializable_record_with_depth_two_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwoRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthTwoNestedSumsMaterialization, MaterializationDiagnostic> {
    let derived = derive_depth_two_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
    )?;
    let outer_fingerprint = normalized_layout_plan_report_fingerprint(&path_layout.outer_layout);
    let materialization_fingerprint = depth_two_nested_sums_materialization_report_fingerprint(
        schema_name,
        derived.schema_report_fingerprint,
        outer_fingerprint,
        path_layout,
        &derived.occurrences,
        byte_order,
        value,
        &derived.bytes,
    );
    Ok(ValidatedConstRecordWithDepthTwoNestedSumsMaterialization {
        schema_name: schema_name.to_owned(),
        non_authoritative_schema_report_fingerprint: derived.schema_report_fingerprint,
        value: value.clone(),
        path_layout: path_layout.clone(),
        non_authoritative_outer_layout_report_fingerprint: outer_fingerprint,
        occurrences: derived.occurrences,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_fingerprint,
    })
}

struct DerivedDepthTwoNestedSumsMaterialization {
    schema_report_fingerprint: u64,
    occurrences: Vec<ValidatedConstDepthTwoNestedSumOccurrenceMaterialization>,
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
    inner_layout: &'a psi_layout_plans::LayoutPlanReport,
    child_sum_layouts: &'a [psi_layout_plans::ConventionalSumFieldLayoutReport],
}

#[derive(Clone, Copy)]
enum NestedPathsView<'a> {
    Singular(&'a ConventionalNestedRecordSumPathLayoutReport),
    Plural(&'a ConventionalNestedRecordSumPathsLayoutReport),
}

impl<'a> NestedPathsView<'a> {
    fn outer_layout(self) -> &'a psi_layout_plans::LayoutPlanReport {
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
