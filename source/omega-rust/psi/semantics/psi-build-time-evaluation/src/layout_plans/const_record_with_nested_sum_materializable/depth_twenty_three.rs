//! Exact plural depth-twenty-three constant-value custody and replay.

use super::depth_twenty_two::validate_const_materializable_record_with_depth_twenty_two_nested_sums_with_reachability;
use super::*;
use psi_layout_plans::ConventionalDepthTwentyThreeRecordSumPathsLayoutReport;

/// Exact custody for one authored outer-field occurrence in the complete
/// plural depth-twenty-three path set.
///
/// The nested carrier retains its complete plural depth-twenty-two custody. This
/// type deliberately does not implement `Clone`.
pub type ValidatedConstDepthTwentyThreeNestedSumOccurrenceMaterialization =
    ValidatedConstRecursiveNestedSumOccurrenceMaterialization<
        ValidatedConstRecordWithDepthTwentyTwoNestedSumsMaterialization,
    >;

/// Exact custody for the complete authored-order set of qualifying
/// `Outer -> TwentyFirst -> Twentieth -> Nineteenth -> Eighteenth -> Seventeenth -> Sixteenth -> Fifteenth -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct sums` paths.
///
/// The outer layout and final image are retained once. Each outer occurrence
/// owns exactly one unchanged plural depth-twenty-two carrier. This type deliberately
/// does not implement `Clone`.
pub type ValidatedConstRecordWithDepthTwentyThreeNestedSumsMaterialization =
    ValidatedConstRecursiveNestedSumsMaterialization<
        ConventionalDepthTwentyThreeRecordSumPathsLayoutReport,
        ValidatedConstRecordWithDepthTwentyTwoNestedSumsMaterialization,
    >;

impl ValidatedConstRecordWithDepthTwentyThreeNestedSumsMaterialization {
    /// Re-resolve the complete authored-order path set and independently replay
    /// every retained depth-twenty-two carrier before accepting the staged image.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        path_layout: &ConventionalDepthTwentyThreeRecordSumPathsLayoutReport,
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
        path_layout: &ConventionalDepthTwentyThreeRecordSumPathsLayoutReport,
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
            "depth-twenty-three",
            b"omega.const-materializable-plural-depth-twenty-three-record-sum-paths.v1",
            derive_depth_twenty_three_nested_sums_bytes_with_reachability,
            ValidatedConstRecordWithDepthTwentyTwoNestedSumsMaterialization::replay_against_with_reachability,
            ValidatedConstRecordWithDepthTwentyTwoNestedSumsMaterialization::schema_name,
            ValidatedConstRecordWithDepthTwentyTwoNestedSumsMaterialization::value,
            ValidatedConstRecordWithDepthTwentyTwoNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
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
            "depth-twenty-three",
            Self::replay_against,
        )
    }
}

pub fn validate_const_materializable_record_with_depth_twenty_three_nested_sums(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwentyThreeRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<
    ValidatedConstRecordWithDepthTwentyThreeNestedSumsMaterialization,
    MaterializationDiagnostic,
> {
    let mut reachability = SumReachability::new(typed);
    validate_const_materializable_record_with_depth_twenty_three_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        &mut reachability,
    )
}

pub(super) fn validate_const_materializable_record_with_depth_twenty_three_nested_sums_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwentyThreeRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<
    ValidatedConstRecordWithDepthTwentyThreeNestedSumsMaterialization,
    MaterializationDiagnostic,
> {
    validate_recursive_nested_sums_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        b"omega.const-materializable-plural-depth-twenty-three-record-sum-paths.v1",
        derive_depth_twenty_three_nested_sums_bytes_with_reachability,
        ValidatedConstRecordWithDepthTwentyTwoNestedSumsMaterialization::non_authoritative_materialization_report_fingerprint,
    )
}

type DerivedDepthTwentyThreeNestedSumsMaterialization = DerivedRecursiveNestedSumsMaterialization<
    ValidatedConstRecordWithDepthTwentyTwoNestedSumsMaterialization,
>;

fn derive_depth_twenty_three_nested_sums_bytes_with_reachability(
    typed: &TypedTrees,
    schema_name: &str,
    path_layout: &ConventionalDepthTwentyThreeRecordSumPathsLayoutReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
    reachability: &mut SumReachability<'_>,
) -> Result<DerivedDepthTwentyThreeNestedSumsMaterialization, MaterializationDiagnostic> {
    derive_recursive_nested_sums_bytes_with_reachability(
        typed,
        schema_name,
        path_layout,
        value,
        byte_order,
        reachability,
        "depth-twenty-three",
        validate_const_materializable_record_with_depth_twenty_two_nested_sums_with_reachability,
        ValidatedConstRecordWithDepthTwentyTwoNestedSumsMaterialization::bytes,
    )
}
