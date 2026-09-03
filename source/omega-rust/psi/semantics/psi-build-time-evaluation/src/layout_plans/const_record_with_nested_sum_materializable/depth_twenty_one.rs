//! Exact plural depth-twenty-one constant-value custody and replay.

use super::depth_twenty::validate_const_materializable_record_with_depth_twenty_nested_sums_with_reachability;
use super::*;
use psi_layout_plans::ConventionalDepthTwentyOneRecordSumPathsLayoutReport;

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-twenty-one path set.
///
/// The nested carrier retains its complete plural depth-twenty custody. This
/// type deliberately does not implement `Clone`.
pub type ValidatedConstDepthTwentyOneNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthTwentyNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Nineteenth -> Eighteenth -> Seventeenth -> Sixteenth -> Fifteenth -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-twenty carrier. This type deliberately
/// does not implement `Clone`.
pub type ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization =
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
        ValidatedConstRecordWithDepthTwentyNestedSumsMaterialization,
    >;

impl ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization {
    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-twenty carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
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
        path_layout: &ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
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
            "depth-twenty-one",
            b"omega.const-materializable-plural-depth-twenty-one-record-sum-paths.v1",
            derive_depth_twenty_one_nested_sums_bytes_with_reachability,
            ValidatedConstRecordWithDepthTwentyNestedSumsMaterialization::replay_against_with_reachability,
            ValidatedConstRecordWithDepthTwentyNestedSumsMaterialization::schema_name,
            ValidatedConstRecordWithDepthTwentyNestedSumsMaterialization::value,
            ValidatedConstRecordWithDepthTwentyNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
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
            "depth-twenty-one",
            Self::replay_against,
        )
    }
}

pub fn validate_const_materializable_record_with_depth_twenty_one_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<
    ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization,
    MaterializationDiagnostic,
> {
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_twenty_one_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

pub(super) fn validate_const_materializable_record_with_depth_twenty_one_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<
    ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization,
    MaterializationDiagnostic,
> {
    validate_recursive_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        b"omega.const-materializable-plural-depth-twenty-one-record-sum-paths.v1",
        derive_depth_twenty_one_nested_sums_bytes_with_reachability,
        ValidatedConstRecordWithDepthTwentyNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
    )
}

type DerivedDepthTwentyOneNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthTwentyNestedSumsMaterialization,
>;

fn derive_depth_twenty_one_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwentyOneRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthTwentyOneNestedSumsMaterialization, MaterializationDiagnostic> {
    derive_recursive_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        "depth-twenty-one",
        validate_const_materializable_record_with_depth_twenty_nested_sums_with_reachability,
        ValidatedConstRecordWithDepthTwentyNestedSumsMaterialization::bytes,
    )
}
