//! Exact plural depth-sixteen constant-value custody and replay.

use super::*;
use psi_layout_plans::ConventionalDepthSixteenRecordSumPathsLayoutReport;

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-sixteen path set.
///
/// The nested carrier retains its complete plural depth-fifteen custody. This
/// type deliberately does not implement `Clone`.
pub type ValidatedConstDepthSixteenNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-fifteen carrier. This type deliberately
/// does not implement `Clone`.
pub type ValidatedConstRecordWithDepthSixteenNestedSumsMaterialization =
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalDepthSixteenRecordSumPathsLayoutReport,
        ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization,
    >;

impl ValidatedConstRecordWithDepthSixteenNestedSumsMaterialization {
    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-fifteen carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthSixteenRecordSumPathsLayoutReport,
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

    pub(super) fn replay_against_with_reachability(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthSixteenRecordSumPathsLayoutReport,
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
            "depth-sixteen",
            b"omega.const-materializable-plural-depth-sixteen-record-sum-paths.v1",
            derive_depth_sixteen_nested_sums_bytes_with_reachability,
            ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization::replay_against_with_reachability,
            ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization::schema_name,
            ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization::value,
            ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
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
            "depth-sixteen",
            Self::replay_against,
        )
    }
}

pub fn validate_const_materializable_record_with_depth_sixteen_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthSixteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthSixteenNestedSumsMaterialization, MaterializationDiagnostic>
{
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_sixteen_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

pub(super) fn validate_const_materializable_record_with_depth_sixteen_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthSixteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthSixteenNestedSumsMaterialization, MaterializationDiagnostic>
{
    validate_recursive_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        b"omega.const-materializable-plural-depth-sixteen-record-sum-paths.v1",
        derive_depth_sixteen_nested_sums_bytes_with_reachability,
        ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
    )
}

type DerivedDepthSixteenNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization,
>;

fn derive_depth_sixteen_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthSixteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthSixteenNestedSumsMaterialization, MaterializationDiagnostic> {
    derive_recursive_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        "depth-sixteen",
        validate_const_materializable_record_with_depth_fifteen_nested_sums_with_reachability,
        ValidatedConstRecordWithDepthFifteenNestedSumsMaterialization::bytes,
    )
}
