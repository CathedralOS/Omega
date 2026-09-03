//! Exact plural depth-eighteen constant-value custody and replay.

use super::depth_seventeen::validate_const_materializable_record_with_depth_seventeen_nested_sums_with_reachability;
use super::*;
use psi_layout_plans::ConventionalDepthEighteenRecordSumPathsLayoutReport;

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-eighteen path set.
///
/// The nested carrier retains its complete plural depth-seventeen custody. This
/// type deliberately does not implement `Clone`.
pub type ValidatedConstDepthEighteenNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthSeventeenNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Sixteenth -> Fifteenth -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-seventeen carrier. This type deliberately
/// does not implement `Clone`.
pub type ValidatedConstRecordWithDepthEighteenNestedSumsMaterialization =
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalDepthEighteenRecordSumPathsLayoutReport,
        ValidatedConstRecordWithDepthSeventeenNestedSumsMaterialization,
    >;

impl ValidatedConstRecordWithDepthEighteenNestedSumsMaterialization {
    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-seventeen carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthEighteenRecordSumPathsLayoutReport,
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
        path_layout: &ConventionalDepthEighteenRecordSumPathsLayoutReport,
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
            "depth-eighteen",
            b"omega.const-materializable-plural-depth-eighteen-record-sum-paths.v1",
            derive_depth_eighteen_nested_sums_bytes_with_reachability,
            ValidatedConstRecordWithDepthSeventeenNestedSumsMaterialization::replay_against_with_reachability,
            ValidatedConstRecordWithDepthSeventeenNestedSumsMaterialization::schema_name,
            ValidatedConstRecordWithDepthSeventeenNestedSumsMaterialization::value,
            ValidatedConstRecordWithDepthSeventeenNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
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
            "depth-eighteen",
            Self::replay_against,
        )
    }
}

pub fn validate_const_materializable_record_with_depth_eighteen_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthEighteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstRecordWithDepthEighteenNestedSumsMaterialization, MaterializationDiagnostic>
{
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_eighteen_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

pub(super) fn validate_const_materializable_record_with_depth_eighteen_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthEighteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<ValidatedConstRecordWithDepthEighteenNestedSumsMaterialization, MaterializationDiagnostic>
{
    validate_recursive_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        b"omega.const-materializable-plural-depth-eighteen-record-sum-paths.v1",
        derive_depth_eighteen_nested_sums_bytes_with_reachability,
        ValidatedConstRecordWithDepthSeventeenNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
    )
}

type DerivedDepthEighteenNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthSeventeenNestedSumsMaterialization,
>;

fn derive_depth_eighteen_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthEighteenRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthEighteenNestedSumsMaterialization, MaterializationDiagnostic> {
    derive_recursive_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        "depth-eighteen",
        validate_const_materializable_record_with_depth_seventeen_nested_sums_with_reachability,
        ValidatedConstRecordWithDepthSeventeenNestedSumsMaterialization::bytes,
    )
}
