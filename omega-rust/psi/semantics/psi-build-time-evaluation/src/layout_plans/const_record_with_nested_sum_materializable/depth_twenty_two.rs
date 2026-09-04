//! Exact plural depth-twenty-two constant-value custody and replay.

use super::depth_twenty_one::validate_const_materializable_record_with_depth_twenty_one_nested_sums_with_reachability;
use super::*;
use psi_layout_plans::ConventionalDepthTwentyTwoRecordSumPathsLayoutReport;

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-twenty-two path set.
///
/// The nested carrier retains its complete plural depth-twenty-one custody. This
/// type deliberately does not implement `Clone`.
pub type ValidatedConstDepthTwentyTwoNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> Twentieth -> Nineteenth -> Eighteenth -> Seventeenth -> Sixteenth -> Fifteenth -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-twenty-one carrier. This type deliberately
/// does not implement `Clone`.
pub type ValidatedConstRecordWithDepthTwentyTwoNestedSumsMaterialization =
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalDepthTwentyTwoRecordSumPathsLayoutReport,
        ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization,
    >;

impl ValidatedConstRecordWithDepthTwentyTwoNestedSumsMaterialization {
    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-twenty-one carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthTwentyTwoRecordSumPathsLayoutReport,
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
        path_layout: &ConventionalDepthTwentyTwoRecordSumPathsLayoutReport,
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
            "depth-twenty-two",
            b"omega.const-materializable-plural-depth-twenty-two-record-sum-paths.v1",
            derive_depth_twenty_two_nested_sums_bytes_with_reachability,
            ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization::replay_against_with_reachability,
            ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization::schema_name,
            ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization::value,
            ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
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
            "depth-twenty-two",
            Self::replay_against,
        )
    }
}

pub fn validate_const_materializable_record_with_depth_twenty_two_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwentyTwoRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<
    ValidatedConstRecordWithDepthTwentyTwoNestedSumsMaterialization,
    MaterializationDiagnostic,
> {
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_twenty_two_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

pub(super) fn validate_const_materializable_record_with_depth_twenty_two_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwentyTwoRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<
    ValidatedConstRecordWithDepthTwentyTwoNestedSumsMaterialization,
    MaterializationDiagnostic,
> {
    validate_recursive_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        b"omega.const-materializable-plural-depth-twenty-two-record-sum-paths.v1",
        derive_depth_twenty_two_nested_sums_bytes_with_reachability,
        ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
    )
}

type DerivedDepthTwentyTwoNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization,
>;

fn derive_depth_twenty_two_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwentyTwoRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthTwentyTwoNestedSumsMaterialization, MaterializationDiagnostic> {
    derive_recursive_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        "depth-twenty-two",
        validate_const_materializable_record_with_depth_twenty_one_nested_sums_with_reachability,
        ValidatedConstRecordWithDepthTwentyOneNestedSumsMaterialization::bytes,
    )
}
